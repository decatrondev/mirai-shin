//! Identidad del producto. La única fuente es `brand.json` en la raíz del repo,
//! embebido al compilar: para renombrar, se edita ese archivo y se recompila.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Brand {
    pub name: String,
    pub slug: String,
    /// Identificador de la app (`net.decatron.MiraiShin`): carpetas de datos, socket y Flatpak.
    #[serde(rename = "appId")]
    pub app_id: String,
    pub domain: String,
    pub repo: String,
}

const BRAND_JSON: &str = include_str!("../../../brand.json");

/// Solo falla si `brand.json` está mal escrito, que es un error de compilación, no de uso.
pub fn get() -> &'static Brand {
    static BRAND: OnceLock<Brand> = OnceLock::new();
    BRAND.get_or_init(|| serde_json::from_str(BRAND_JSON).expect("brand.json mal formado"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn brand_json_se_lee() {
        let b = super::get();
        assert!(!b.name.is_empty());
        assert_eq!(b.slug, b.slug.to_lowercase());
        // Debe servir como nombre de carpeta y de socket en todos los sistemas.
        assert!(b
            .app_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.'));
    }
}
