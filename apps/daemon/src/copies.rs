//! Copias compatibles de los videos (fase 2.9): la parte con disco y procesos. Las reglas
//! están en `miraishin_core::copies`.
//!
//! Cada fondo tiene su carpeta en `<datos>/video-copies/<hash de la carpeta>/` con un
//! `folder.txt` (el nombre del fondo, para poder limpiar) y sus copias. La copia se escribe
//! como `.part` y se renombra al terminar, así una copia a medias nunca se reproduce.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use miraishin_core::copies::{self, CopyTarget, VideoInfo};

/// Dónde está la copia de un fondo para un destino.
pub fn copy_path(copies_dir: &Path, folder: &str, version: &str, target: &CopyTarget) -> PathBuf {
    copies_dir
        .join(copies::folder_dir_name(folder))
        .join(copies::copy_file_name(version, target))
}

/// ¿Este equipo no tiene GPU real? (Linux: fabricantes de `/sys/class/drm`.) En Windows,
/// por ahora, se supone que sí la tiene.
pub fn no_real_gpu() -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut vendors = Vec::new();
        if let Ok(dir) = std::fs::read_dir("/sys/class/drm") {
            for e in dir.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                // card0, card1… (no card0-HDMI-A-1, que son las salidas).
                if name.starts_with("card") && !name.contains('-') {
                    if let Some(v) = std::fs::read_to_string(e.path().join("device/vendor"))
                        .ok()
                        .and_then(|t| copies::parse_vendor(&t))
                    {
                        vendors.push(v);
                    }
                }
            }
        }
        copies::no_real_gpu(&vendors)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// El programa `ffmpeg`: el del Flatpak (`/app/bin`), o el del sistema si hay.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let flatpak = Path::new("/app/bin/ffmpeg");
    if flatpak.is_file() {
        return Some(flatpak.to_path_buf());
    }
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}

/// Lee códec, tamaño y cuadros del video con `ffmpeg -i` (solo la cabecera, como mucho 5 s).
/// `None` si no hay `ffmpeg` o no se entiende: entonces se sigue como si necesitara copia.
pub fn probe(ffmpeg: Option<&Path>, input: &Path) -> Option<VideoInfo> {
    let mut child = Command::new(ffmpeg?)
        .args(copies::probe_args(input))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let stderr = child.stderr.take()?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(stderr), &mut text);
        text
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                tracing::warn!(file = %input.display(), "reading the video header took too long");
                return None;
            }
        }
    }
    copies::parse_video_info(&reader.join().ok()?)
}

/// Crea la copia (bloquea: se llama desde un hilo propio). `progress` recibe el porcentaje.
/// Devuelve el código del problema si falla: `copy_unavailable` (no hay `ffmpeg`),
/// `video_damaged` (el archivo está roto: `ffmpeg` no lo puede abrir), `copy_failed` (no se
/// pudo convertir por otra razón, por ejemplo falta espacio).
pub fn make_copy(
    ffmpeg: Option<&Path>,
    input: &Path,
    output: &Path,
    target: &CopyTarget,
    mut progress: impl FnMut(u8),
) -> Result<(), &'static str> {
    let ffmpeg = ffmpeg.ok_or("copy_unavailable")?;
    let dir = output.parent().ok_or("copy_failed")?;
    std::fs::create_dir_all(dir).map_err(|_| "copy_failed")?;
    let part = output.with_extension("webm.part");
    let mut child = Command::new(ffmpeg)
        .args(copies::ffmpeg_args(input, &part, target))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "copy_unavailable")?;

    // La duración llega por la salida de errores; el avance, por la estándar.
    let stderr = child.stderr.take();
    let (dur_tx, dur_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut tail = Vec::new();
        if let Some(err) = stderr {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if let Some(d) = copies::parse_duration_us(&line) {
                    let _ = dur_tx.send(d);
                }
                tail.push(line);
                if tail.len() > 20 {
                    tail.remove(0);
                }
            }
        }
        tail
    });
    let mut total = 0;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if total == 0 {
                total = dur_rx.try_recv().unwrap_or(0);
            }
            if let Some(done) = copies::parse_progress_us(&line) {
                progress(copies::percent(done, total));
            }
        }
    }
    let status = child.wait().map_err(|_| "copy_failed")?;
    let tail = reader.join().unwrap_or_default();
    if !status.success() {
        tracing::warn!(status = ?status.code(), last = ?tail.last(), "video copy failed");
        let _ = std::fs::remove_file(&part);
        return Err(if copies::is_damaged_input(&tail) {
            "video_damaged"
        } else {
            "copy_failed"
        });
    }
    std::fs::rename(&part, output).map_err(|_| {
        let _ = std::fs::remove_file(&part);
        "copy_failed"
    })?;
    Ok(())
}

/// Anota a qué fondo pertenece su carpeta de copias y borra las copias viejas de ese fondo
/// (de otra versión del video o de otro destino), menos `keep`.
pub fn prepare_folder(copies_dir: &Path, folder: &str, keep: &[PathBuf]) {
    let dir = copies_dir.join(copies::folder_dir_name(folder));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("folder.txt"), folder);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            let is_copy = p.extension().is_some_and(|x| x == "webm" || x == "part");
            if is_copy && !keep.contains(&p) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

/// Borra las carpetas de copias de fondos que ya no existen. `exists` dice si un fondo
/// sigue en la biblioteca. Las copias a medias (`.part`) de una sesión anterior también.
pub fn cleanup(copies_dir: &Path, exists: impl Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(copies_dir) else {
        return;
    };
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let folder = std::fs::read_to_string(dir.join("folder.txt")).ok();
        if folder.as_deref().is_none_or(|f| !exists(f)) {
            let _ = std::fs::remove_dir_all(&dir);
            continue;
        }
        if let Ok(files) = std::fs::read_dir(&dir) {
            for f in files.flatten() {
                if f.path().extension().is_some_and(|x| x == "part") {
                    let _ = std::fs::remove_file(f.path());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limpieza() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        let keep = copy_path(root, "olas", "v2", &CopyTarget::SAVER);
        let old = copy_path(root, "olas", "v1", &CopyTarget::SAVER);
        std::fs::create_dir_all(keep.parent().unwrap()).unwrap();
        std::fs::write(&keep, b"x").unwrap();
        std::fs::write(&old, b"x").unwrap();
        prepare_folder(root, "olas", std::slice::from_ref(&keep));
        assert!(keep.is_file() && !old.exists());

        let gone = copy_path(root, "borrado", "v1", &CopyTarget::SAVER);
        prepare_folder(root, "borrado", &[]);
        std::fs::write(&gone, b"x").unwrap();
        std::fs::write(keep.with_extension("webm.part"), b"x").unwrap();
        cleanup(root, |f| f == "olas");
        assert!(keep.is_file());
        assert!(!keep.with_extension("webm.part").exists());
        assert!(!gone.parent().unwrap().exists());
    }

    #[test]
    fn sin_ffmpeg() {
        let t = tempfile::tempdir().unwrap();
        let r = make_copy(
            None,
            &t.path().join("a.mp4"),
            &t.path().join("c/a.webm"),
            &CopyTarget::SAVER,
            |_| {},
        );
        assert_eq!(r, Err("copy_unavailable"));
    }
}
