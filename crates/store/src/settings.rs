//! Ajustes en `settings.toml`. Cada campo tiene valor por defecto; un valor malo usa el
//! suyo y se avisa, sin caerse. Al guardar se conservan las claves que esta versión no
//! conoce (si una versión nueva agregó ajustes, una vieja no los borra).

use miraishin_core::copies::VideoSaver;
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    System,
    Es,
    En,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Es => "es",
            Self::En => "en",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "system" => Some(Self::System),
            "es" => Some(Self::Es),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    /// Idioma efectivo: el elegido, o el del sistema (`LC_ALL`/`LANG`) si es `system`.
    /// Cualquier idioma sin traducción cae a inglés.
    pub fn resolve(self, system_locale: Option<&str>) -> Self {
        match self {
            Self::System => match system_locale {
                Some(l) if l.to_ascii_lowercase().starts_with("es") => Self::Es,
                _ => Self::En,
            },
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settings {
    /// `None` = la carpeta por defecto (`<datos>/wallpapers`).
    pub wallpapers_dir: Option<PathBuf>,
    pub language: Language,
    pub log_level: LogLevel,
    /// Copias de ahorro de los videos (fase 2.9): `auto`, `on` u `off`.
    pub video_saver: VideoSaver,
}

/// Aviso al leer los ajustes: clave con valor no válido (se usó el valor por defecto),
/// o archivo ilegible entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWarning {
    UnreadableFile,
    InvalidValue(&'static str),
}

impl Settings {
    /// Lee el archivo. Si no existe, valores por defecto sin avisos.
    pub fn load(path: &Path) -> (Self, Vec<SettingsWarning>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), Vec::new()),
            Err(_) => (Self::default(), vec![SettingsWarning::UnreadableFile]),
        }
    }

    pub fn parse(text: &str) -> (Self, Vec<SettingsWarning>) {
        let Ok(doc) = text.parse::<DocumentMut>() else {
            return (Self::default(), vec![SettingsWarning::UnreadableFile]);
        };
        let mut warnings = Vec::new();
        let mut s = Self::default();

        if let Some(item) = doc.get("wallpapers_dir") {
            match item.as_str() {
                Some(p) if !p.trim().is_empty() => s.wallpapers_dir = Some(PathBuf::from(p)),
                _ => warnings.push(SettingsWarning::InvalidValue("wallpapers_dir")),
            }
        }
        if let Some(item) = doc.get("language") {
            match item.as_str().and_then(Language::parse) {
                Some(l) => s.language = l,
                None => warnings.push(SettingsWarning::InvalidValue("language")),
            }
        }
        if let Some(item) = doc.get("log_level") {
            match item.as_str().and_then(LogLevel::parse) {
                Some(l) => s.log_level = l,
                None => warnings.push(SettingsWarning::InvalidValue("log_level")),
            }
        }
        if let Some(item) = doc.get("video_saver") {
            match item.as_str().and_then(VideoSaver::parse) {
                Some(v) => s.video_saver = v,
                None => warnings.push(SettingsWarning::InvalidValue("video_saver")),
            }
        }
        (s, warnings)
    }

    /// Escribe los ajustes sobre el contenido actual del archivo (conserva claves
    /// desconocidas y comentarios). Escritura atómica: archivo temporal y renombrar.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let current = std::fs::read_to_string(path).unwrap_or_default();
        let text = self.merge_into(&current);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    fn merge_into(&self, current: &str) -> String {
        // Si el archivo actual está roto se empieza de cero (ya se avisó al leerlo).
        let mut doc = current.parse::<DocumentMut>().unwrap_or_default();
        match &self.wallpapers_dir {
            Some(p) => doc["wallpapers_dir"] = value(p.display().to_string()),
            None => {
                doc.remove("wallpapers_dir");
            }
        }
        doc["language"] = value(self.language.as_str());
        doc["log_level"] = value(self.log_level.as_str());
        doc["video_saver"] = value(self.video_saver.as_str());
        doc.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_archivo_usa_valores_por_defecto() {
        let dir = tempfile::tempdir().unwrap();
        let (s, w) = Settings::load(&dir.path().join("no-existe.toml"));
        assert_eq!(s, Settings::default());
        assert!(w.is_empty());
    }

    #[test]
    fn valor_malo_usa_su_defecto_y_avisa() {
        let (s, w) =
            Settings::parse("language = \"fr\"\nlog_level = \"debug\"\nwallpapers_dir = 3\n");
        assert_eq!(s.language, Language::System);
        assert_eq!(s.log_level, LogLevel::Debug);
        assert_eq!(s.wallpapers_dir, None);
        assert_eq!(
            w,
            vec![
                SettingsWarning::InvalidValue("wallpapers_dir"),
                SettingsWarning::InvalidValue("language")
            ]
        );
        let (s, w) = Settings::parse("video_saver = \"on\"\n");
        assert_eq!(s.video_saver, VideoSaver::On);
        assert!(w.is_empty());
        let (s, w) = Settings::parse("video_saver = \"siempre\"\n");
        assert_eq!(s.video_saver, VideoSaver::Auto);
        assert_eq!(w, vec![SettingsWarning::InvalidValue("video_saver")]);
        let (s, w) = Settings::parse("esto no es toml = = =");
        assert_eq!(s, Settings::default());
        assert_eq!(w, vec![SettingsWarning::UnreadableFile]);
    }

    #[test]
    fn guardar_conserva_claves_desconocidas_y_comentarios() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg/settings.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "# mío\nfps_limit = 30 # de una versión nueva\nlanguage = \"en\"\n",
        )
        .unwrap();

        let (mut s, _) = Settings::load(&path);
        assert_eq!(s.language, Language::En);
        s.language = Language::Es;
        s.wallpapers_dir = Some(PathBuf::from("/fondos"));
        s.save(&path).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mío"));
        assert!(text.contains("fps_limit = 30 # de una versión nueva"));
        let (again, w) = Settings::load(&path);
        assert!(w.is_empty());
        assert_eq!(again.language, Language::Es);
        assert_eq!(again.wallpapers_dir, Some(PathBuf::from("/fondos")));
    }

    #[test]
    fn idioma_del_sistema() {
        assert_eq!(Language::System.resolve(Some("es_PE.UTF-8")), Language::Es);
        assert_eq!(Language::System.resolve(Some("fr_FR.UTF-8")), Language::En);
        assert_eq!(Language::System.resolve(None), Language::En);
        assert_eq!(Language::Es.resolve(Some("en_US")), Language::Es);
    }
}
