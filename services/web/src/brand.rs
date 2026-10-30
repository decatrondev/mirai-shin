use serde::{Deserialize, Serialize};

/// Identidad del producto. La única fuente es `brand.json` en la raíz del repo,
/// embebido al compilar: para renombrar, se edita ese archivo y se recompila.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brand {
    pub name: String,
    pub slug: String,
    pub domain: String,
    pub repo: String,
}

const BRAND_JSON: &str = include_str!("../../../brand.json");

impl Brand {
    /// Solo falla si `brand.json` está mal escrito, que es un error de compilación, no de uso.
    pub fn load() -> Self {
        serde_json::from_str(BRAND_JSON).expect("brand.json mal formado")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn brand_json_se_lee() {
        let b = super::Brand::load();
        assert!(!b.name.is_empty());
        assert_eq!(b.slug, b.slug.to_lowercase());
    }
}
