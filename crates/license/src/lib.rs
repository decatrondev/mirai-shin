//! Interfaz de licencias. Se diseña desde la fase 0 para que la tienda propia (fase 6B)
//! entre como una implementación más sin tocar el daemon ni la app.
//! Con AGPL cualquiera puede quitarla al compilar: no se invierte en anti-piratería.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Valid,
    Trial,
    Missing,
    Invalid,
}

pub trait LicenseProvider: Send + Sync {
    fn status(&self) -> LicenseStatus;
    /// Nombre corto de la implementación, para `status` y los logs.
    fn kind(&self) -> &'static str;
}

/// Versión sin licencia requerida: la que usa el daemon hasta la fase 6B.
pub struct NotRequired;

impl LicenseProvider for NotRequired {
    fn status(&self) -> LicenseStatus {
        LicenseStatus::Valid
    }

    fn kind(&self) -> &'static str {
        "not_required"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_licencia_requerida_es_valida() {
        let p: Box<dyn LicenseProvider> = Box::new(NotRequired);
        assert_eq!(p.status(), LicenseStatus::Valid);
        assert_eq!(p.kind(), "not_required");
    }
}
