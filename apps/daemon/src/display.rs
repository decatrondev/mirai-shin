//! Fondos en pantalla (fase 1.5): lo elegido en la biblioteca → un cuadro por monitor.
//!
//! Corre en su propio hilo, porque decodificar y escalar una imagen tarda (hasta ~1 s con
//! una foto grande) y no debe frenar las respuestas del IPC. Habla con el resto por canales.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use miraishin_core::fit::FitMode;
use miraishin_core::surface::{EventSink, Frame, OutputInfo, Surface, SurfaceEvent};
use miraishin_ipc::{events, Event, OutputItem, OutputsList};
use miraishin_render_image as render;
use miraishin_store::{Assignment, Library, ALL_OUTPUTS};
use serde_json::json;
use tokio::sync::{broadcast, oneshot};

/// Crea la superficie. Devuelve el nombre de la forma usada (`layer_shell`, …) o el código
/// de por qué no hay.
pub type SurfaceFactory =
    Box<dyn FnOnce(EventSink) -> Result<(Box<dyn Surface>, &'static str), String> + Send>;

/// Color de respaldo cuando lo elegido no se puede mostrar (grafito del sistema de diseño).
pub const FALLBACK: [u8; 4] = [0x1c, 0x1d, 0x21, 0xff];

/// Lo que se le puede pedir al hilo.
enum Msg {
    /// Volver a leer lo elegido y ponerlo (después de `set`, `clear` o `rescan`).
    Apply(Option<oneshot::Sender<()>>),
    Surface(SurfaceEvent),
    Outputs(oneshot::Sender<OutputsList>),
    Stop,
}

/// Manija para el resto del daemon.
#[derive(Clone)]
pub struct Display {
    tx: mpsc::Sender<Msg>,
    surface: String,
    available: bool,
}

impl Display {
    pub fn start(
        factory: SurfaceFactory,
        library: Arc<Mutex<Library>>,
        wallpapers_dir: PathBuf,
        events: broadcast::Sender<Event>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let sink_tx = tx.clone();
        let sink: EventSink = Box::new(move |e| {
            let _ = sink_tx.send(Msg::Surface(e));
        });
        let (surface, name, available) = match factory(sink) {
            Ok((s, name)) => (Some(s), name.to_string(), true),
            Err(code) => {
                tracing::warn!(code = %code, "no wallpaper surface on this desktop");
                (None, code, false)
            }
        };
        let worker = Worker {
            surface,
            surface_name: name.clone(),
            available,
            library,
            wallpapers_dir,
            events,
            shown: HashMap::new(),
            first: true,
        };
        let started = std::thread::Builder::new()
            .name("display".into())
            .spawn(move || worker.run(rx));
        if started.is_err() {
            tracing::error!("could not start the display thread");
        }
        let display = Self {
            tx,
            surface: name,
            available: available && started.is_ok(),
        };
        display.apply_nowait();
        display
    }

    pub fn surface(&self) -> (&str, bool) {
        (&self.surface, self.available)
    }

    /// Pone lo elegido y espera a que termine (para responder con el estado real).
    pub async fn apply(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Apply(Some(tx))).is_ok() {
            let _ = rx.await;
        }
    }

    pub fn apply_nowait(&self) {
        let _ = self.tx.send(Msg::Apply(None));
    }

    pub async fn outputs(&self) -> OutputsList {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Outputs(tx)).is_err() {
            return OutputsList::default();
        }
        rx.await.unwrap_or_default()
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

/// Qué muestra un monitor ahora. Si nada de esto cambia, no se vuelve a dibujar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    folder: String,
    mode: FitMode,
    /// `updated_at` del fondo: si el archivo cambió (rescan), se dibuja de nuevo.
    version: String,
    pixels: (u32, u32),
    problem: Option<String>,
}

struct Worker {
    surface: Option<Box<dyn Surface>>,
    surface_name: String,
    available: bool,
    library: Arc<Mutex<Library>>,
    wallpapers_dir: PathBuf,
    events: broadcast::Sender<Event>,
    shown: HashMap<String, Shown>,
    /// En la primera pasada se quita el fondo de los monitores sin nada elegido: en KDE el
    /// de una sesión anterior sigue puesto aunque el daemon se haya reiniciado.
    first: bool,
}

impl Worker {
    fn run(mut self, rx: mpsc::Receiver<Msg>) {
        while let Ok(msg) = rx.recv() {
            match msg {
                Msg::Apply(done) => {
                    self.apply();
                    if let Some(d) = done {
                        let _ = d.send(());
                    }
                }
                Msg::Surface(e) => {
                    if let SurfaceEvent::OutputRemoved { name } = &e {
                        self.shown.remove(name);
                        tracing::info!(output = %name, "monitor removed");
                    } else {
                        tracing::info!(event = ?e, "monitor changed");
                    }
                    self.apply();
                }
                Msg::Outputs(reply) => {
                    let _ = reply.send(self.list());
                }
                Msg::Stop => break,
            }
        }
    }

    fn assignments(library: &Mutex<Library>) -> Vec<Assignment> {
        let lib = library.lock().unwrap_or_else(|p| p.into_inner());
        lib.assignments().unwrap_or_else(|e| {
            tracing::error!(error = %e, "could not read assignments");
            Vec::new()
        })
    }

    fn apply(&mut self) {
        let assignments = Self::assignments(&self.library);
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let outputs = surface.outputs();
        let mut changed = false;
        // La misma imagen se decodifica una sola vez para todos los monitores.
        let mut decoded: HashMap<PathBuf, Result<render::RgbaImage, render::RenderError>> =
            HashMap::new();

        for output in &outputs {
            let Some((folder, mode)) = resolve(&assignments, &output.name) else {
                if self.shown.remove(&output.name).is_some() || self.first {
                    let _ = surface.clear(&output.name);
                    changed = true;
                }
                continue;
            };
            let px = output.pixel_size();
            let (version, source) = lookup(&self.library, &self.wallpapers_dir, &folder);
            let key = Shown {
                folder: folder.clone(),
                mode,
                version,
                pixels: (px.width, px.height),
                problem: None,
            };
            if self.shown.get(&output.name).is_some_and(|s| {
                Shown {
                    problem: None,
                    ..s.clone()
                } == key
            }) {
                continue;
            }
            let frame = source.and_then(|path| {
                let image = decoded
                    .entry(path.clone())
                    .or_insert_with(|| render::load_file(&path, render::DEFAULT_MAX_PIXELS));
                let image = image.as_ref().map_err(|e| e.as_str().to_string())?;
                let opts = render::Options {
                    mode,
                    ..render::Options::default()
                };
                render::compose(image, px, &opts).map_err(|e| e.as_str().to_string())
            });
            let (frame, problem) = match frame {
                Ok(f) => (f, None),
                Err(code) => {
                    tracing::warn!(output = %output.name, folder = %folder, code = %code,
                        "wallpaper could not be shown; using the fallback color");
                    match Frame::solid(px, FALLBACK) {
                        Ok(f) => (f, Some(code)),
                        Err(_) => continue,
                    }
                }
            };
            match surface.show(&output.name, frame) {
                Ok(()) => {
                    tracing::info!(output = %output.name, folder = %folder, mode = mode.as_str(),
                        problem = ?problem, "wallpaper shown");
                    self.shown
                        .insert(output.name.clone(), Shown { problem, ..key });
                    changed = true;
                }
                Err(e) => {
                    tracing::warn!(output = %output.name, code = e.as_str(), "surface refused the frame");
                }
            }
        }
        // Lo decodificado se suelta aquí: solo quedan los cuadros en la superficie.
        drop(decoded);
        self.first = false;
        let present: Vec<&str> = outputs.iter().map(|o| o.name.as_str()).collect();
        self.shown
            .retain(|name, _| present.contains(&name.as_str()));
        if changed {
            let _ = self
                .events
                .send(Event::new(events::OUTPUTS_CHANGED, json!(self.list())));
        }
    }

    fn list(&self) -> OutputsList {
        let outputs: Vec<OutputInfo> = self
            .surface
            .as_ref()
            .map(|s| s.outputs())
            .unwrap_or_default();
        let assignments = Self::assignments(&self.library);
        let all = assignments.iter().find(|a| a.output == ALL_OUTPUTS);
        OutputsList {
            items: outputs
                .iter()
                .map(|o| {
                    let px = o.pixel_size();
                    let shown = self.shown.get(&o.name);
                    OutputItem {
                        name: o.name.clone(),
                        description: o.description.clone(),
                        width: o.logical.width,
                        height: o.logical.height,
                        scale_percent: o.scale.percent(),
                        pixel_width: px.width,
                        pixel_height: px.height,
                        wallpaper: shown.map(|s| s.folder.clone()),
                        mode: shown.map(|s| s.mode.as_str().to_string()),
                        problem: shown.and_then(|s| s.problem.clone()),
                    }
                })
                .collect(),
            surface: self.surface_name.clone(),
            surface_available: self.available,
            all: all.and_then(|a| a.folder.clone()),
            all_mode: all.map(|a| a.mode.clone()),
        }
    }
}

/// Qué va en un monitor: lo suyo si tiene, si no lo de «todos». `None` = sin fondo.
pub fn resolve(assignments: &[Assignment], output: &str) -> Option<(String, FitMode)> {
    let own = assignments.iter().find(|a| a.output == output);
    let chosen = own.or_else(|| assignments.iter().find(|a| a.output == ALL_OUTPUTS))?;
    let folder = chosen.folder.clone()?;
    Some((folder, FitMode::parse(&chosen.mode).unwrap_or_default()))
}

/// Versión y archivo de imagen de un fondo, o el código de por qué no se puede mostrar.
fn lookup(
    library: &Arc<Mutex<Library>>,
    wallpapers_dir: &std::path::Path,
    folder: &str,
) -> (String, Result<PathBuf, String>) {
    let lib = library.lock().unwrap_or_else(|p| p.into_inner());
    let row = match lib.get(folder) {
        Ok(Some(r)) => r,
        Ok(None) => return (String::new(), Err("wallpaper_not_found".into())),
        Err(_) => return (String::new(), Err("internal".into())),
    };
    let version = row.updated_at.clone();
    if !row.valid {
        return (version, Err("wallpaper_invalid".into()));
    }
    if row.kind.as_deref() != Some("image") {
        return (version, Err("wallpaper_kind_unsupported".into()));
    }
    let Some(file) = row.file else {
        return (version, Err("wallpaper_invalid".into()));
    };
    (version, Ok(wallpapers_dir.join(folder).join(file)))
}

/// La superficie de este escritorio, si hay. Los códigos los explica `ctl` (fase 1.6).
/// `data_dir`: carpeta de datos de la app (en KDE ahí van los cuadros que lee Plasma).
pub fn auto_surface(data_dir: PathBuf) -> SurfaceFactory {
    Box::new(move |sink| {
        #[cfg(target_os = "linux")]
        {
            let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
            // En KDE, layer-shell tapa los íconos (prueba del 2026-10-30): ahí el fondo va por
            // el complemento de Plasma.
            if desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE")) {
                return miraishin_surface_plasma::PlasmaSurface::connect(
                    sink,
                    data_dir.join("plasma"),
                )
                .map(|s| (Box::new(s) as Box<dyn Surface>, "plasma"));
            }
            miraishin_surface_wayland::WaylandSurface::connect(sink)
                .map(|s| (Box::new(s) as Box<dyn Surface>, "layer_shell"))
                .map_err(|e| e.as_str().to_string())
        }
        #[cfg(not(target_os = "linux"))]
        {
            drop((sink, data_dir));
            Err("platform_pending".into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(output: &str, folder: Option<&str>, mode: &str) -> Assignment {
        Assignment {
            output: output.into(),
            folder: folder.map(Into::into),
            mode: mode.into(),
        }
    }

    #[test]
    fn lo_del_monitor_gana_a_todos() {
        let list = [
            a("*", Some("lluvia"), "fill"),
            a("DP-1", Some("mar"), "fit"),
        ];
        assert_eq!(resolve(&list, "DP-1"), Some(("mar".into(), FitMode::Fit)));
        assert_eq!(
            resolve(&list, "HDMI-A-1"),
            Some(("lluvia".into(), FitMode::Fill))
        );
    }

    #[test]
    fn sin_fondo() {
        assert_eq!(resolve(&[], "DP-1"), None);
        let list = [a("*", Some("lluvia"), "fill"), a("DP-1", None, "fill")];
        assert_eq!(resolve(&list, "DP-1"), None);
    }

    #[test]
    fn modo_desconocido_usa_rellenar() {
        let list = [a("*", Some("lluvia"), "zoom")];
        assert_eq!(resolve(&list, "X"), Some(("lluvia".into(), FitMode::Fill)));
    }
}
