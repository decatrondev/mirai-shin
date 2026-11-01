//! Dónde vive cada cosa. Linux: XDG (dentro de Flatpak apunta al sandbox de la app).
//! Windows: `%APPDATA%`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl Paths {
    /// Rutas reales del usuario. `None` si el sistema no da carpeta de datos (sin HOME).
    pub fn system() -> Option<Self> {
        let app = &miraishin_brand::get().app_id;
        Some(Self {
            data_dir: dirs::data_dir()?.join(app),
            config_dir: dirs::config_dir()?.join(app),
        })
    }

    /// Todo debajo de una carpeta: para pruebas.
    pub fn under(root: &Path) -> Self {
        Self {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn library_db(&self) -> PathBuf {
        self.data_dir.join("library.db")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Copias compatibles de los videos (fase 2.9). Se pueden borrar: se rehacen.
    pub fn video_copies_dir(&self) -> PathBuf {
        self.data_dir.join("video-copies")
    }

    pub fn default_wallpapers_dir(&self) -> PathBuf {
        self.data_dir.join("wallpapers")
    }
}

/// Ruta para logs y diagnóstico sin exponer la carpeta personal: `/home/ana/x` → `~/x`.
pub fn redact(path: &Path) -> String {
    redact_with_home(path, dirs::home_dir().as_deref())
}

fn redact_with_home(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oculta_la_carpeta_personal() {
        let home = Path::new("/home/ana");
        assert_eq!(
            redact_with_home(Path::new("/home/ana/fondos"), Some(home)),
            "~/fondos"
        );
        assert_eq!(redact_with_home(Path::new("/home/ana"), Some(home)), "~");
        assert_eq!(redact_with_home(Path::new("/srv/x"), Some(home)), "/srv/x");
    }
}
