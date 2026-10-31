//! Datos locales de la app: rutas por sistema, ajustes TOML y la biblioteca SQLite.

pub mod library;
pub mod paths;
pub mod settings;

pub use library::{Library, OpenReport, ScanReport, WallpaperRow};
pub use paths::Paths;
pub use settings::{Language, LogLevel, Settings};
