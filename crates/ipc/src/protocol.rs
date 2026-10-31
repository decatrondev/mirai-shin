//! Mensajes del protocolo v1.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

/// Largo máximo de una línea; una más larga corta la conexión.
pub const MAX_LINE: usize = 1 << 20;

pub mod methods {
    pub const HELLO: &str = "hello";
    pub const PING: &str = "ping";
    pub const STATUS: &str = "status";
    pub const LIBRARY_LIST: &str = "library.list";
    pub const LIBRARY_RESCAN: &str = "library.rescan";
    pub const SHUTDOWN: &str = "shutdown";
}

pub mod events {
    pub const LIBRARY_CHANGED: &str = "library_changed";
}

/// Códigos de error. Nunca se manda el mensaje interno, solo el código.
pub mod codes {
    pub const UNKNOWN_METHOD: &str = "unknown_method";
    pub const INVALID_REQUEST: &str = "invalid_request";
    pub const INVALID_PARAMS: &str = "invalid_params";
    pub const LINE_TOO_LONG: &str = "line_too_long";
    pub const INTERNAL: &str = "internal";
    pub const WALLPAPERS_DIR_MISSING: &str = "wallpapers_dir_missing";
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: String,
}

impl RpcError {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub v: u32,
    /// `None` cuando el pedido era tan malo que no se pudo leer su id.
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub event: String,
    #[serde(default)]
    pub data: Value,
}

impl Event {
    pub fn new(event: &str, data: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            event: event.to_string(),
            data,
        }
    }
}

// ---- parámetros y resultados de cada método (todo opcional tiene valor por defecto)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HelloParams {
    #[serde(default)]
    pub protocol: u32,
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HelloResult {
    pub protocol: u32,
    #[serde(default)]
    pub daemon_version: String,
    #[serde(default)]
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StatusResult {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub protocol: u32,
    #[serde(default)]
    pub uptime_secs: u64,
    /// Ruta sin la carpeta personal (`~/…`).
    #[serde(default)]
    pub wallpapers_dir: String,
    #[serde(default)]
    pub wallpapers_dir_exists: bool,
    #[serde(default)]
    pub valid: u64,
    #[serde(default)]
    pub invalid: u64,
    #[serde(default)]
    pub license: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LibraryItem {
    pub folder: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub valid: bool,
    #[serde(default)]
    pub newer_schema: bool,
    #[serde(default)]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LibraryList {
    #[serde(default)]
    pub items: Vec<LibraryItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RescanResult {
    #[serde(default)]
    pub valid: u64,
    #[serde(default)]
    pub invalid: u64,
    #[serde(default)]
    pub removed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campos_desconocidos_se_ignoran() {
        let r: Request =
            serde_json::from_str(r#"{"v":1,"id":3,"method":"ping","futuro":true}"#).unwrap();
        assert_eq!(r.params, Value::Null);
        let s: StatusResult = serde_json::from_str(r#"{"version":"9","gpu":"x"}"#).unwrap();
        assert_eq!(s.version, "9");
    }

    #[test]
    fn respuesta_sin_campos_vacios() {
        let r = Response {
            v: 1,
            id: Some(1),
            result: Some(Value::Bool(true)),
            error: None,
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"v":1,"id":1,"result":true}"#
        );
    }
}
