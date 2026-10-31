//! Archivos del complemento de Plasma y su instalación. El nombre sale de `brand.json`.

use std::io;
use std::path::{Path, PathBuf};

const MAIN_QML: &str = include_str!("../../../packaging/kde-wallpaper/contents/ui/main.qml");
const MAIN_XML: &str = include_str!("../../../packaging/kde-wallpaper/contents/config/main.xml");

/// Id del complemento en Plasma: el id de la app.
pub fn plugin_id() -> &'static str {
    &miraishin_brand::get().app_id
}

/// Nombre con que Plasma lista el complemento (`knownWallpaperPlugins` usa el nombre).
pub fn plugin_name() -> &'static str {
    &miraishin_brand::get().name
}

/// (ruta relativa, contenido) de cada archivo del complemento.
pub fn files() -> Vec<(&'static str, String)> {
    let b = miraishin_brand::get();
    let metadata = serde_json::json!({
        "KPackageStructure": "Plasma/Wallpaper",
        "KPlugin": {
            "Id": b.app_id,
            "Name": b.name,
            "Description": "Animated wallpapers",
            "Description[es]": "Fondos animados",
            "Icon": "preferences-desktop-wallpaper",
            "License": "AGPL-3.0-only",
            "Website": format!("https://{}", b.domain),
        },
        "X-KDE-ParentApp": "org.kde.plasmashell",
    });
    vec![
        (
            "metadata.json",
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        ),
        ("contents/ui/main.qml", MAIN_QML.to_string()),
        ("contents/config/main.xml", MAIN_XML.to_string()),
    ]
}

/// Carpeta de complementos de fondo de Plasma del usuario (`~/.local/share/plasma/wallpapers`).
/// Dentro de Flatpak, `XDG_DATA_HOME` apunta a la carpeta de la app: se usa la del sistema,
/// que el permiso `--filesystem=xdg-data/plasma/wallpapers:create` deja ver en la misma ruta.
pub fn user_wallpapers_dir() -> Option<PathBuf> {
    let flatpak = std::env::var_os("FLATPAK_ID").is_some();
    let data = if flatpak {
        std::env::var_os("HOST_XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local/share")))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home().map(|h| h.join(".local/share")))
    }?;
    Some(data.join("plasma/wallpapers"))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Deja el complemento en `wallpapers_dir/<id>/`. Solo escribe lo que cambió.
/// Devuelve `true` si escribió algo (instalado o actualizado).
pub fn install(wallpapers_dir: &Path) -> io::Result<bool> {
    let root = wallpapers_dir.join(plugin_id());
    let mut changed = false;
    for (rel, content) in files() {
        let path = root.join(rel);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &path)?;
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_con_el_nombre_de_la_marca() {
        let f = files();
        let meta: serde_json::Value = serde_json::from_str(&f[0].1).unwrap();
        assert_eq!(meta["KPackageStructure"], "Plasma/Wallpaper");
        assert_eq!(meta["KPlugin"]["Id"], plugin_id());
        assert_eq!(meta["KPlugin"]["Name"], plugin_name());
        assert!(f[1].1.contains("WallpaperItem"));
        assert!(f[2].1.contains(r#"name="Image""#));
    }

    #[test]
    fn instala_y_no_reescribe_si_no_cambio() {
        let t = tempfile::tempdir().unwrap();
        assert!(install(t.path()).unwrap());
        let main = t.path().join(plugin_id()).join("contents/ui/main.qml");
        assert!(main.exists());
        assert!(!install(t.path()).unwrap());
        std::fs::write(&main, "viejo").unwrap();
        assert!(install(t.path()).unwrap());
        assert_eq!(std::fs::read_to_string(&main).unwrap(), MAIN_QML);
    }
}
