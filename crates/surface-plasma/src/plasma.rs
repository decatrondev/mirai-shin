//! `PlasmaSurface`: cumple el contrato `Surface` pasándole los cuadros a Plasma.

use std::collections::{HashMap, HashSet};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use miraishin_core::fit::FitMode;
use miraishin_core::surface::{check_frame, EventSink, Frame, OutputInfo, Surface, SurfaceError};
use miraishin_surface_wayland::OutputWatcher;
use zbus::blocking::Connection;

use crate::{package, script};

const STATE_FILE: &str = "state.json";

pub struct PlasmaSurface {
    watcher: OutputWatcher,
    dbus: Connection,
    frames_dir: PathBuf,
    /// Tipo de fondo que tenía cada monitor antes del nuestro (se guarda en disco: hay que
    /// poder devolverlo aunque el daemon se haya reiniciado).
    previous: HashMap<String, String>,
    /// Archivo del cuadro que muestra cada monitor (se borra al cambiar o quitar).
    current: HashMap<String, PathBuf>,
    /// Monitores donde se pidió un video (solo a esos se les pregunta su estado).
    videos: HashSet<String>,
    counter: u64,
}

impl PlasmaSurface {
    /// Instala o actualiza el complemento, comprueba que Plasma responde y lo ve, y empieza
    /// a seguir los monitores. Los cuadros se escriben en `frames_dir`, que Plasma debe
    /// poder leer (la carpeta de datos de la app lo es, también dentro de Flatpak).
    pub fn connect(events: EventSink, frames_dir: PathBuf) -> Result<Self, String> {
        let dbus = Connection::session().map_err(|_| "kde_plasma_unavailable".to_string())?;
        let dir = package::user_wallpapers_dir().ok_or("kde_plugin_install_failed")?;
        match package::install(&dir) {
            Ok(true) => tracing::info!("plasma add-on installed or updated"),
            Ok(false) => {}
            Err(_) => return Err("kde_plugin_install_failed".into()),
        }
        let known = eval(&dbus, &script::probe(package::plugin_name()))?;
        if known.trim() != "known" {
            // Plasma todavía no ve el complemento recién instalado.
            return Err("kde_plugin_needs_restart".into());
        }
        std::fs::create_dir_all(&frames_dir).map_err(|_| "kde_frames_dir_failed".to_string())?;
        let previous = std::fs::read_to_string(frames_dir.join(STATE_FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let watcher = OutputWatcher::connect(events).map_err(|e| e.as_str().to_string())?;
        let mut surface = Self {
            watcher,
            dbus,
            frames_dir,
            previous,
            current: HashMap::new(),
            videos: HashSet::new(),
            counter: 0,
        };
        surface.forget_old_frames();
        Ok(surface)
    }

    fn output(&self, name: &str) -> Result<OutputInfo, SurfaceError> {
        self.watcher
            .outputs()
            .into_iter()
            .find(|o| o.name == name)
            .ok_or(SurfaceError::UnknownOutput)
    }

    fn save_state(&self) {
        if let Ok(text) = serde_json::to_string(&self.previous) {
            let _ = std::fs::write(self.frames_dir.join(STATE_FILE), text);
        }
    }

    /// Al arrancar, los PNG de una sesión anterior ya no se usan (se escriben nuevos).
    /// El que Plasma esté mostrando se borra recién cuando llegue el nuevo cuadro.
    fn forget_old_frames(&mut self) {
        let Ok(entries) = std::fs::read_dir(&self.frames_dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "png") {
                let output = frame_output(&p);
                if let Some(o) = output {
                    // Se recuerda para borrarlo después de poner el siguiente.
                    if let Some(old) = self.current.insert(o, p.clone()) {
                        let _ = std::fs::remove_file(old);
                    }
                }
            }
        }
    }

    fn write_png(&mut self, output: &str, frame: &Frame) -> Result<PathBuf, SurfaceError> {
        self.counter += 1;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = self
            .frames_dir
            .join(format!("{}~{stamp}-{}.png", safe(output), self.counter));
        let tmp = path.with_extension("tmp");
        let file = std::fs::File::create(&tmp).map_err(|_| SurfaceError::Disconnected)?;
        let enc = PngEncoder::new_with_quality(
            BufWriter::new(file),
            CompressionType::Fast,
            FilterType::Adaptive,
        );
        enc.write_image(
            &frame.rgba,
            frame.size.width,
            frame.size.height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| SurfaceError::Disconnected)?;
        std::fs::rename(&tmp, &path).map_err(|_| SurfaceError::Disconnected)?;
        Ok(path)
    }
}

impl Surface for PlasmaSurface {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.watcher.outputs()
    }

    fn show(&mut self, output: &str, frame: Frame) -> Result<(), SurfaceError> {
        check_frame(&self.output(output)?, &frame)?;
        let path = self.write_png(output, &frame)?;
        drop(frame);
        let entries = script::image_entries(&script::file_url(&path));
        match self.put(output, &entries) {
            Ok(()) => {
                self.videos.remove(output);
                if let Some(old) = self.current.insert(output.to_string(), path) {
                    let _ = std::fs::remove_file(old);
                }
                Ok(())
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(e)
            }
        }
    }

    /// El video lo reproduce Plasma en el complemento (prueba 2.0): aquí solo se le pasa
    /// el archivo y el encuadre.
    fn show_video(
        &mut self,
        output: &str,
        video: &Path,
        mode: FitMode,
    ) -> Result<(), SurfaceError> {
        self.output(output)?;
        let entries = script::video_entries(&script::file_url(video), mode.as_str());
        self.put(output, &entries)?;
        self.videos.insert(output.to_string());
        // El PNG anterior ya no hace falta: el complemento suelta la imagen al ver el video.
        if let Some(old) = self.current.remove(output) {
            let _ = std::fs::remove_file(old);
        }
        Ok(())
    }

    /// El complemento deja su estado en la configuración (`Status`); se lee por D-Bus solo
    /// cuando alguien lo pide (`outputs`), no a cada rato.
    fn video_problem(&self, output: &str) -> Option<String> {
        if !self.videos.contains(output) {
            return None;
        }
        let reply = eval(
            &self.dbus,
            &script::read_status(output, package::plugin_id()),
        )
        .ok()?;
        script::parse_status(&reply)
    }

    fn clear(&mut self, output: &str) -> Result<(), SurfaceError> {
        self.videos.remove(output);
        let restore = self
            .previous
            .get(output)
            .cloned()
            .unwrap_or_else(|| script::PLASMA_DEFAULT.to_string());
        let reply = eval(
            &self.dbus,
            &script::clear(output, &restore, package::plugin_id()),
        );
        if let Ok(Ok(_)) = reply.as_deref().map(script::parse) {
            if self.previous.remove(output).is_some() {
                self.save_state();
            }
        }
        if let Some(old) = self.current.remove(output) {
            let _ = std::fs::remove_file(old);
        }
        Ok(())
    }
}

impl PlasmaSurface {
    /// Pone nuestro complemento en la pantalla de `output` con esa configuración, y recuerda
    /// el tipo de fondo que había antes.
    fn put(&mut self, output: &str, entries: &[(&str, String)]) -> Result<(), SurfaceError> {
        let entries: Vec<(&str, &str)> = entries.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let reply = eval(
            &self.dbus,
            &script::show(output, &entries, package::plugin_id()),
        );
        match reply.as_deref().map(script::parse) {
            Ok(Ok(prev)) => {
                if !prev.is_empty() && prev != package::plugin_id() {
                    self.previous.insert(output.to_string(), prev);
                    self.save_state();
                }
                Ok(())
            }
            other => {
                tracing::warn!(output = %output, reply = ?other, "plasma did not take the wallpaper");
                Err(SurfaceError::UnknownOutput)
            }
        }
    }
}

/// Corre un guion en Plasma y devuelve lo que imprimió.
fn eval(dbus: &Connection, js: &str) -> Result<String, String> {
    let reply = dbus
        .call_method(
            Some("org.kde.plasmashell"),
            "/PlasmaShell",
            Some("org.kde.PlasmaShell"),
            "evaluateScript",
            &(js,),
        )
        .map_err(|_| "kde_plasma_unavailable".to_string())?;
    reply
        .body()
        .deserialize::<String>()
        .map_err(|_| "kde_script_failed".to_string())
}

/// Nombre de monitor apto para un archivo.
fn safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Monitor de un archivo de cuadro (`DP-1~123-4.png` → `DP-1`).
fn frame_output(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    name.split_once('~').map(|(o, _)| o.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_de_archivo() {
        assert_eq!(safe("DP-1"), "DP-1");
        assert_eq!(safe("../x y"), "___x_y");
        assert_eq!(
            frame_output(Path::new("/a/Virtual-1~17-3.png")),
            Some("Virtual-1".into())
        );
        assert_eq!(frame_output(Path::new("/a/state.json")), None);
    }
}
