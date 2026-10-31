//! Guiones para `org.kde.PlasmaShell.evaluateScript` y la lectura de su respuesta.
//! Puro: se prueba sin Plasma. Todo texto que entra va como literal JSON (válido en JS).

/// Tipo de fondo de Plasma por defecto, para devolver si no se sabe cuál había.
pub const PLASMA_DEFAULT: &str = "org.kde.image";

fn lit(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// Comprueba que Plasma responde y si ya ve el complemento.
pub fn probe(plugin_name: &str) -> String {
    format!(
        "print(knownWallpaperPlugins()[{}] !== undefined ? \"known\" : \"unknown\");",
        lit(plugin_name)
    )
}

/// Pone el cuadro `url` en la pantalla del monitor `output`. Imprime `ok:<tipo anterior>`.
pub fn show(output: &str, url: &str, plugin_id: &str) -> String {
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d) {{ print(\"no_desktop\"); }} else {{
  var prev = d.wallpaperPlugin;
  d.wallpaperPlugin = {id};
  d.currentConfigGroup = [\"Wallpaper\", {id}, \"General\"];
  d.writeConfig(\"Image\", {url});
  print(\"ok:\" + prev);
}}",
        out = lit(output),
        id = lit(plugin_id),
        url = lit(url),
    )
}

/// Devuelve el tipo de fondo anterior en la pantalla de `output`, si tiene el nuestro.
pub fn clear(output: &str, restore: &str, plugin_id: &str) -> String {
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d) {{ print(\"no_desktop\"); }} else {{
  if (d.wallpaperPlugin == {id}) {{ d.wallpaperPlugin = {restore}; }}
  print(\"ok:\");
}}",
        out = lit(output),
        id = lit(plugin_id),
        restore = lit(restore),
    )
}

/// Lee la respuesta de `show`/`clear`: `Ok(tipo anterior)` o el código del problema.
pub fn parse(reply: &str) -> Result<String, String> {
    let reply = reply.trim();
    if let Some(prev) = reply.strip_prefix("ok:") {
        return Ok(prev.to_string());
    }
    if reply == "no_desktop" {
        return Err("kde_no_desktop".into());
    }
    Err("kde_script_failed".into())
}

/// `file://` de una ruta absoluta, con los caracteres raros escapados.
pub fn file_url(path: &std::path::Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_texto_entra_escapado() {
        let s = show("DP-1\"); evil(); (\"", "file:///x.png", "net.x.Y");
        assert!(s.contains(r#"screenForConnector("DP-1\"); evil(); (\"")"#));
        assert!(s.contains(r#"d.wallpaperPlugin = "net.x.Y";"#));
        assert!(s.contains(r#"d.writeConfig("Image", "file:///x.png");"#));
        let c = clear("Virtual-1", PLASMA_DEFAULT, "net.x.Y");
        assert!(c.contains(
            r#"if (d.wallpaperPlugin == "net.x.Y") { d.wallpaperPlugin = "org.kde.image"; }"#
        ));
        assert!(probe("Mirai Shin").contains(r#"knownWallpaperPlugins()["Mirai Shin"]"#));
    }

    #[test]
    fn respuestas() {
        assert_eq!(parse("ok:org.kde.image\n"), Ok("org.kde.image".into()));
        assert_eq!(parse("ok:"), Ok(String::new()));
        assert_eq!(parse("no_desktop"), Err("kde_no_desktop".into()));
        assert_eq!(parse("TypeError: x"), Err("kde_script_failed".into()));
    }

    #[test]
    fn url_de_archivo() {
        let p = std::path::Path::new("/home/ana maría/.var/app/x/a-1.png");
        assert_eq!(
            file_url(p),
            "file:///home/ana%20mar%C3%ADa/.var/app/x/a-1.png"
        );
    }
}
