//! Datos locales de la app: rutas por sistema, ajustes TOML y la biblioteca SQLite.

pub mod library;
pub mod paths;
pub mod settings;

pub use library::{Assignment, Library, OpenReport, ScanReport, WallpaperRow, ALL_OUTPUTS};
pub use paths::Paths;
pub use settings::{Language, LogLevel, Settings};
