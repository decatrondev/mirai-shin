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

/// Pone nuestro complemento en la pantalla del monitor `output` y escribe su configuración
/// en el orden dado (Plasma avisa al complemento de cada clave). Imprime `ok:<tipo anterior>`.
pub fn show(output: &str, entries: &[(&str, &str)], plugin_id: &str) -> String {
    let writes: String = entries
        .iter()
        .map(|(k, v)| format!("\n  d.writeConfig({}, {});", lit(k), lit(v)))
        .collect();
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d) {{ print(\"no_desktop\"); }} else {{
  var prev = d.wallpaperPlugin;
  d.wallpaperPlugin = {id};
  d.currentConfigGroup = [\"Wallpaper\", {id}, \"General\"];{writes}
  print(\"ok:\" + prev);
}}",
        out = lit(output),
        id = lit(plugin_id),
    )
}

/// Configuración para mostrar un cuadro fijo (PNG ya encuadrado por el daemon).
/// La imagen primero: el video se quita cuando la imagen ya está.
pub fn image_entries(url: &str) -> Vec<(&'static str, String)> {
    vec![("Image", url.to_string()), ("Video", String::new())]
}

/// Configuración para reproducir un video. El modo y la pausa antes que el video, para que
/// arranque ya encuadrado (y en pausa si toca); la imagen anterior la suelta el complemento
/// cuando el video se ve. `pause_when`: ver `pause_when`.
pub fn video_entries(
    url: &str,
    mode: &str,
    paused: bool,
    pause_when: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("Mode", mode.to_string()),
        ("Paused", paused_value(paused)),
        ("PauseWhen", pause_when.to_string()),
        ("Video", url.to_string()),
        ("Image", String::new()),
    ]
}

/// Valor de la clave `PauseWhen` (fase 3.3): con qué ventanas encima pausa solo el
/// complemento.
pub fn pause_when(on_fullscreen: bool, on_maximized: bool) -> String {
    let mut words = Vec::new();
    if on_fullscreen {
        words.push("fullscreen");
    }
    if on_maximized {
        words.push("maximized");
    }
    words.join(" ")
}

/// Valor de la clave `Paused` (fase 3).
pub fn paused_value(paused: bool) -> String {
    if paused { "true" } else { "" }.to_string()
}

/// Escribe claves en la configuración de nuestro complemento en la pantalla de `output`, sin
/// cambiar el tipo de fondo. Imprime `ok:` o `none` si ahí no está el nuestro.
pub fn write(output: &str, entries: &[(&str, &str)], plugin_id: &str) -> String {
    let writes: String = entries
        .iter()
        .map(|(k, v)| format!("\n  d.writeConfig({}, {});", lit(k), lit(v)))
        .collect();
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d || d.wallpaperPlugin != {id}) {{ print(\"none\"); }} else {{
  d.currentConfigGroup = [\"Wallpaper\", {id}, \"General\"];{writes}
  print(\"ok:\");
}}",
        out = lit(output),
        id = lit(plugin_id),
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

/// Lee el estado que el complemento dejó en su configuración para el monitor `output`
/// (`Status`, ver `main.qml`). Imprime `status:<valor>` o `none` si ahí no está el nuestro.
pub fn read_status(output: &str, plugin_id: &str) -> String {
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d || d.wallpaperPlugin != {id}) {{ print(\"none\"); }} else {{
  d.currentConfigGroup = [\"Wallpaper\", {id}, \"General\"];
  print(\"status:\" + d.readConfig(\"Status\"));
}}",
        out = lit(output),
        id = lit(plugin_id),
    )
}

/// Lee por qué ventana pausó solo el complemento en `output` (`Covered`, fase 3.3). Imprime
/// `covered:<valor>` o `none`.
pub fn read_covered(output: &str, plugin_id: &str) -> String {
    format!(
        "var s = screenForConnector({out});
var d = s >= 0 ? desktopForScreen(s) : undefined;
if (!d || d.wallpaperPlugin != {id}) {{ print(\"none\"); }} else {{
  d.currentConfigGroup = [\"Wallpaper\", {id}, \"General\"];
  print(\"covered:\" + d.readConfig(\"Covered\"));
}}",
        out = lit(output),
        id = lit(plugin_id),
    )
}

/// De la respuesta de `read_covered`: `fullscreen`, `maximized` o nada.
pub fn parse_covered(reply: &str) -> Option<&'static str> {
    match reply.trim().strip_prefix("covered:")? {
        "fullscreen" => Some("fullscreen"),
        "maximized" => Some("maximized"),
        _ => None,
    }
}

/// De la respuesta de `read_status`, el código del problema si el complemento informó uno
/// (`video_…`). `loading`, `playing`, vacío o algo raro → `None`. Solo deja pasar letras
/// minúsculas y `_`: el código termina en un texto de `ctl`.
pub fn parse_status(reply: &str) -> Option<String> {
    let value = reply.trim().strip_prefix("status:")?;
    let ok = value.starts_with("video_")
        && value.len() <= 40
        && value.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
    ok.then(|| value.to_string())
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
        let s = show(
            "DP-1\"); evil(); (\"",
            &[("Image", "file:///x.png"), ("Video", "\");x(\"")],
            "net.x.Y",
        );
        assert!(s.contains(r#"screenForConnector("DP-1\"); evil(); (\"")"#));
        assert!(s.contains(r#"d.wallpaperPlugin = "net.x.Y";"#));
        assert!(s.contains(r#"d.writeConfig("Image", "file:///x.png");"#));
        assert!(s.contains(r#"d.writeConfig("Video", "\");x(\"");"#));
        // En orden.
        assert!(s.find("\"Image\"").unwrap() < s.find("\"Video\"").unwrap());
        let c = clear("Virtual-1", PLASMA_DEFAULT, "net.x.Y");
        assert!(c.contains(
            r#"if (d.wallpaperPlugin == "net.x.Y") { d.wallpaperPlugin = "org.kde.image"; }"#
        ));
        assert!(probe("Mirai Shin").contains(r#"knownWallpaperPlugins()["Mirai Shin"]"#));
    }

    #[test]
    fn claves_de_imagen_y_video() {
        assert_eq!(
            image_entries("file:///a.png"),
            vec![("Image", "file:///a.png".into()), ("Video", String::new())]
        );
        let v = video_entries("file:///v.mp4", "fit", false, "fullscreen");
        assert_eq!(v[0], ("Mode", "fit".into()));
        assert_eq!(v[1], ("Paused", String::new()));
        assert_eq!(v[2], ("PauseWhen", "fullscreen".into()));
        assert_eq!(v[3], ("Video", "file:///v.mp4".into()));
        assert_eq!(v[4], ("Image", String::new()));
        assert_eq!(
            video_entries("file:///v.mp4", "fill", true, "")[1],
            ("Paused", "true".into())
        );
        assert_eq!(pause_when(true, false), "fullscreen");
        assert_eq!(pause_when(true, true), "fullscreen maximized");
        assert_eq!(pause_when(false, true), "maximized");
        assert_eq!(pause_when(false, false), "");
        let w = write("DP-1", &[("Paused", "true")], "net.x.Y");
        assert!(w.contains(r#"d.wallpaperPlugin != "net.x.Y""#));
        assert!(w.contains(r#"d.writeConfig("Paused", "true");"#));
        assert!(!w.contains("d.wallpaperPlugin ="));
    }

    #[test]
    fn respuestas() {
        assert_eq!(parse("ok:org.kde.image\n"), Ok("org.kde.image".into()));
        assert_eq!(parse("ok:"), Ok(String::new()));
        assert_eq!(parse("no_desktop"), Err("kde_no_desktop".into()));
        assert_eq!(parse("TypeError: x"), Err("kde_script_failed".into()));
    }

    #[test]
    fn estado_del_complemento() {
        let js = read_status("DP-1", "net.x.Y");
        assert!(js.contains(r#"screenForConnector("DP-1")"#));
        assert!(js.contains(r#"d.wallpaperPlugin != "net.x.Y""#));
        assert!(js.contains(r#"d.readConfig("Status")"#));
        assert_eq!(
            parse_status("status:video_no_decoder\n"),
            Some("video_no_decoder".into())
        );
        assert_eq!(
            parse_status("status:video_too_slow"),
            Some("video_too_slow".into())
        );
        for nada in [
            "status:playing",
            "status:loading",
            "status:",
            "status:undefined",
            "none",
            "",
            "status:video_x; rm",
            "status:video_ÑO",
            "TypeError: x",
        ] {
            assert_eq!(parse_status(nada), None, "{nada}");
        }
    }

    #[test]
    fn ventana_encima() {
        let js = read_covered("DP-1", "net.x.Y");
        assert!(js.contains(r#"d.readConfig("Covered")"#));
        assert_eq!(parse_covered("covered:fullscreen\n"), Some("fullscreen"));
        assert_eq!(parse_covered("covered:maximized"), Some("maximized"));
        for nada in ["covered:", "none", "covered:x", "", "TypeError"] {
            assert_eq!(parse_covered(nada), None, "{nada}");
        }
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
