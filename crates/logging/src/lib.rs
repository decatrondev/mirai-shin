//! Logs a archivo con rotación diaria en `<datos>/logs/`, conservando los últimos días.
//! `RUST_LOG` pisa el nivel de los ajustes para depurar.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Días de log que se conservan.
pub const KEEP_FILES: usize = 7;

/// Crea el escritor de archivo con rotación. Separado de `init` para poder probarlo
/// sin instalar el suscriptor global.
pub fn file_appender(dir: &Path, prefix: &str) -> anyhow::Result<RollingFileAppender> {
    std::fs::create_dir_all(dir)?;
    Ok(RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(prefix)
        .filename_suffix("log")
        .max_log_files(KEEP_FILES)
        .build(dir)?)
}

/// Instala el registro global. Hay que guardar el `WorkerGuard` mientras corre el programa:
/// al soltarlo se vacía lo pendiente al archivo.
pub fn init(dir: &Path, prefix: &str, level: &str, console: bool) -> anyhow::Result<WorkerGuard> {
    let (writer, guard) = tracing_appender::non_blocking(file_appender(dir, prefix)?);
    let filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let file_layer = fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_filter(filter());
    let console_layer = console.then(|| {
        fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(filter())
    });
    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .try_init()?;
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    #[test]
    fn escribe_en_un_archivo_con_fecha() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = super::file_appender(dir.path(), "daemon").unwrap();
        w.write_all(b"hola\n").unwrap();
        w.flush().unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(
            names[0].starts_with("daemon.") && names[0].ends_with(".log"),
            "{names:?}"
        );
    }
}
