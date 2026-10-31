//! Formato de fondo v1: una carpeta con `manifest.json` y sus archivos.
//!
//! Reglas para que nada se rompa al crecer:
//! - Los campos desconocidos se ignoran (un fondo hecho con una versión nueva abre en una vieja).
//! - Un `schema_version` mayor al conocido se lee igual y se marca como «más nuevo».
//! - El validador devuelve todos los problemas juntos, cada uno con un código estable
//!   (la app lo traduce; aquí no hay texto para la persona).

use serde::Serialize;
use serde_json::{Map, Value};

/// Versión del formato que esta compilación conoce.
pub const SCHEMA_VERSION: u64 = 1;

pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mkv"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperKind {
    Image,
    Video,
}

impl WallpaperKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "image" => Some(Self::Image),
            "video" => Some(Self::Video),
            _ => None,
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Image => IMAGE_EXTENSIONS,
            Self::Video => VIDEO_EXTENSIONS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Manifest {
    pub schema_version: u64,
    pub id: String,
    pub kind: WallpaperKind,
    /// Ruta relativa dentro de la carpeta, siempre con `/`.
    pub file: String,
    pub title: String,
    pub preview: Option<String>,
    pub author: Option<String>,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Validated {
    pub manifest: Manifest,
    /// Hecho con una versión del formato más nueva que esta: puede traer cosas que no se usan.
    pub newer_schema: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    InvalidJson,
    NotAnObject,
    MissingField,
    WrongType,
    EmptyField,
    UnsupportedSchema,
    UnknownKind,
    UnsafePath,
    FileNotFound,
    ExtensionMismatch,
}

impl IssueCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::NotAnObject => "not_an_object",
            Self::MissingField => "missing_field",
            Self::WrongType => "wrong_type",
            Self::EmptyField => "empty_field",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::UnknownKind => "unknown_kind",
            Self::UnsafePath => "unsafe_path",
            Self::FileNotFound => "file_not_found",
            Self::ExtensionMismatch => "extension_mismatch",
        }
    }
}

/// Un problema del manifiesto: código estable y, si aplica, el campo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub code: IssueCode,
    pub field: Option<&'static str>,
}

impl Issue {
    fn new(code: IssueCode, field: Option<&'static str>) -> Self {
        Self { code, field }
    }

    /// Forma compacta para guardar o mostrar en logs: `missing_field:title`.
    pub fn to_code_string(&self) -> String {
        match self.field {
            Some(f) => format!("{}:{f}", self.code.as_str()),
            None => self.code.as_str().to_string(),
        }
    }
}

/// Valida el texto de un `manifest.json`.
///
/// `file_exists` responde si una ruta relativa (con `/`) existe dentro de la carpeta del fondo.
/// Así esta función no toca el disco y se puede probar sola.
pub fn validate(text: &str, file_exists: impl Fn(&str) -> bool) -> Result<Validated, Vec<Issue>> {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Err(vec![Issue::new(IssueCode::InvalidJson, None)]),
    };
    let Some(obj) = value.as_object() else {
        return Err(vec![Issue::new(IssueCode::NotAnObject, None)]);
    };

    let mut issues = Vec::new();

    let schema_version = match obj.get("schema_version") {
        None => {
            issues.push(Issue::new(IssueCode::MissingField, Some("schema_version")));
            None
        }
        Some(v) => match v.as_u64() {
            Some(n) if n >= 1 => Some(n),
            Some(_) => {
                issues.push(Issue::new(
                    IssueCode::UnsupportedSchema,
                    Some("schema_version"),
                ));
                None
            }
            None => {
                issues.push(Issue::new(IssueCode::WrongType, Some("schema_version")));
                None
            }
        },
    };

    let id = required_text(obj, "id", &mut issues);
    let title = required_text(obj, "title", &mut issues);

    let kind = required_text(obj, "type", &mut issues).and_then(|t| {
        let k = WallpaperKind::from_str(&t);
        if k.is_none() {
            issues.push(Issue::new(IssueCode::UnknownKind, Some("type")));
        }
        k
    });

    let file = required_text(obj, "file", &mut issues)
        .and_then(|f| checked_path(&f, "file", &file_exists, &mut issues));
    if let (Some(k), Some(f)) = (kind, &file) {
        if !has_extension(f, k.extensions()) {
            issues.push(Issue::new(IssueCode::ExtensionMismatch, Some("file")));
        }
    }

    let preview = optional_text(obj, "preview", &mut issues)
        .and_then(|p| checked_path(&p, "preview", &file_exists, &mut issues));
    if let Some(p) = &preview {
        if !has_extension(p, IMAGE_EXTENSIONS) {
            issues.push(Issue::new(IssueCode::ExtensionMismatch, Some("preview")));
        }
    }

    let author = optional_text(obj, "author", &mut issues);
    let source_url = optional_text(obj, "source_url", &mut issues);
    let tags = match obj.get("tags") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => {
            let tags: Option<Vec<String>> = items
                .iter()
                .map(|t| t.as_str().map(str::to_owned))
                .collect();
            tags.unwrap_or_else(|| {
                issues.push(Issue::new(IssueCode::WrongType, Some("tags")));
                Vec::new()
            })
        }
        Some(_) => {
            issues.push(Issue::new(IssueCode::WrongType, Some("tags")));
            Vec::new()
        }
    };

    if !issues.is_empty() {
        return Err(issues);
    }
    // Sin problemas, todos los obligatorios están.
    let (Some(schema_version), Some(id), Some(kind), Some(file), Some(title)) =
        (schema_version, id, kind, file, title)
    else {
        unreachable!("campo obligatorio ausente sin problema registrado");
    };
    Ok(Validated {
        newer_schema: schema_version > SCHEMA_VERSION,
        manifest: Manifest {
            schema_version,
            id,
            kind,
            file,
            title,
            preview,
            author,
            source_url,
            tags,
        },
    })
}

fn required_text(
    obj: &Map<String, Value>,
    field: &'static str,
    issues: &mut Vec<Issue>,
) -> Option<String> {
    match obj.get(field) {
        None | Some(Value::Null) => {
            issues.push(Issue::new(IssueCode::MissingField, Some(field)));
            None
        }
        Some(v) => text_value(v, field, issues),
    }
}

fn optional_text(
    obj: &Map<String, Value>,
    field: &'static str,
    issues: &mut Vec<Issue>,
) -> Option<String> {
    match obj.get(field) {
        None | Some(Value::Null) => None,
        Some(v) => text_value(v, field, issues),
    }
}

fn text_value(v: &Value, field: &'static str, issues: &mut Vec<Issue>) -> Option<String> {
    match v.as_str() {
        None => {
            issues.push(Issue::new(IssueCode::WrongType, Some(field)));
            None
        }
        Some(s) if s.trim().is_empty() => {
            issues.push(Issue::new(IssueCode::EmptyField, Some(field)));
            None
        }
        Some(s) => Some(s.trim().to_string()),
    }
}

/// Normaliza una ruta relativa y comprueba que no salga de la carpeta y que exista.
fn checked_path(
    raw: &str,
    field: &'static str,
    file_exists: &impl Fn(&str) -> bool,
    issues: &mut Vec<Issue>,
) -> Option<String> {
    let Some(path) = normalize_relative(raw) else {
        issues.push(Issue::new(IssueCode::UnsafePath, Some(field)));
        return None;
    };
    if !file_exists(&path) {
        issues.push(Issue::new(IssueCode::FileNotFound, Some(field)));
        return None;
    }
    Some(path)
}

/// Ruta relativa segura con `/` como separador, o `None` si es absoluta o sale de la carpeta.
pub fn normalize_relative(raw: &str) -> Option<String> {
    let unified = raw.replace('\\', "/");
    if unified.starts_with('/') || unified.contains(':') {
        // Absoluta en Unix, o con unidad / flujo alterno en Windows (`C:`, `file:stream`).
        return None;
    }
    let mut parts = Vec::new();
    for part in unified.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

fn has_extension(path: &str, allowed: &[&str]) -> bool {
    path.rsplit_once('.')
        .map(|(_, ext)| allowed.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exists(files: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |p| files.contains(&p)
    }

    fn codes(text: &str, files: &'static [&'static str]) -> Vec<String> {
        validate(text, exists(files))
            .unwrap_err()
            .iter()
            .map(Issue::to_code_string)
            .collect()
    }

    const VALID: &str = r#"{
        "schema_version": 1, "id": "abc", "type": "video",
        "file": "./media/loop.MP4", "title": " Lluvia ",
        "preview": "preview.webp", "tags": ["noche"]
    }"#;

    #[test]
    fn manifiesto_valido() {
        let v = validate(VALID, exists(&["media/loop.MP4", "preview.webp"])).unwrap();
        assert!(!v.newer_schema);
        assert_eq!(v.manifest.kind, WallpaperKind::Video);
        assert_eq!(v.manifest.file, "media/loop.MP4");
        assert_eq!(v.manifest.title, "Lluvia");
        assert_eq!(v.manifest.preview.as_deref(), Some("preview.webp"));
        assert_eq!(v.manifest.tags, vec!["noche"]);
    }

    #[test]
    fn campos_desconocidos_se_ignoran() {
        let text = r#"{"schema_version":1,"id":"a","type":"image","file":"a.png","title":"A",
            "campo_del_futuro":{"x":1}}"#;
        assert!(validate(text, exists(&["a.png"])).is_ok());
    }

    #[test]
    fn version_futura_se_lee_y_se_marca() {
        let text = r#"{"schema_version":7,"id":"a","type":"image","file":"a.png","title":"A"}"#;
        assert!(validate(text, exists(&["a.png"])).unwrap().newer_schema);
    }

    #[test]
    fn devuelve_todos_los_problemas_juntos() {
        let c = codes(r#"{"type":"gif","file":"x.png"}"#, &[]);
        assert_eq!(
            c,
            vec![
                "missing_field:schema_version",
                "missing_field:id",
                "missing_field:title",
                "unknown_kind:type",
                "file_not_found:file",
            ]
        );
    }

    #[test]
    fn json_roto_y_no_objeto() {
        assert_eq!(codes("{", &[]), vec!["invalid_json"]);
        assert_eq!(codes("[1]", &[]), vec!["not_an_object"]);
    }

    #[test]
    fn tipos_y_vacios() {
        let c = codes(
            r#"{"schema_version":"1","id":5,"type":"image","file":"a.png","title":"  ","tags":"x"}"#,
            &["a.png"],
        );
        assert_eq!(
            c,
            vec![
                "wrong_type:schema_version",
                "wrong_type:id",
                "empty_field:title",
                "wrong_type:tags"
            ]
        );
        let c = codes(
            r#"{"schema_version":0,"id":"a","type":"image","file":"a.png","title":"A"}"#,
            &["a.png"],
        );
        assert_eq!(c, vec!["unsupported_schema:schema_version"]);
    }

    #[test]
    fn rutas_que_salen_de_la_carpeta() {
        for bad in [
            "../a.png",
            "/etc/a.png",
            "C:\\a.png",
            "a/../../b.png",
            "x\\..\\..\\a.png",
            "./",
        ] {
            let text = format!(
                r#"{{"schema_version":1,"id":"a","type":"image","file":{},"title":"A"}}"#,
                serde_json::to_string(bad).unwrap()
            );
            let c: Vec<String> = validate(&text, |_| true)
                .unwrap_err()
                .iter()
                .map(Issue::to_code_string)
                .collect();
            assert_eq!(c, vec!["unsafe_path:file"], "ruta {bad}");
        }
        assert_eq!(
            normalize_relative("a\\b/./c.png").as_deref(),
            Some("a/b/c.png")
        );
    }

    #[test]
    fn extension_segun_tipo() {
        let c = codes(
            r#"{"schema_version":1,"id":"a","type":"image","file":"a.mp4","title":"A","preview":"p.mp4"}"#,
            &["a.mp4", "p.mp4"],
        );
        assert_eq!(
            c,
            vec!["extension_mismatch:file", "extension_mismatch:preview"]
        );
    }
}
