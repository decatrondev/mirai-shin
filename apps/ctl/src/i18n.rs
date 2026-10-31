//! Textos de `ctl` en español e inglés. Cada clave tiene los dos idiomas (lo comprueba la prueba).

use miraishin_store::Language;

/// (clave, español, inglés). `{}` se reemplaza en orden con `fill`.
const TEXTS: &[(&str, &str, &str)] = &[
    ("usage",
     "{} — consola del daemon\n\nUso: miraishin-ctl <comando> [--json]\n\nComandos:\n  status    estado del daemon y de la biblioteca\n  list      fondos de la biblioteca (válidos e inválidos)\n  rescan    vuelve a leer la carpeta de fondos\n  ping      comprueba que el daemon responde\n  shutdown  apaga el daemon\n\n--json     muestra la respuesta tal cual, en JSON",
     "{} — daemon console\n\nUsage: miraishin-ctl <command> [--json]\n\nCommands:\n  status    daemon and library status\n  list      wallpapers in the library (valid and invalid)\n  rescan    read the wallpapers folder again\n  ping      check that the daemon answers\n  shutdown  stop the daemon\n\n--json     print the raw JSON answer"),
    ("unknown_command", "Comando desconocido: {}", "Unknown command: {}"),
    ("not_running",
     "El daemon no está corriendo. No es un error: arráncalo con miraishin-daemon.",
     "The daemon is not running. This is not an error: start it with miraishin-daemon."),
    ("connection_lost", "Se perdió la conexión con el daemon.", "Lost the connection to the daemon."),
    ("daemon_error", "El daemon respondió con un error: {}", "The daemon answered with an error: {}"),
    ("dir_missing",
     "La carpeta de fondos no existe ({}). Si está en un disco externo, conéctalo; si no, elige otra carpeta en los ajustes.",
     "The wallpapers folder does not exist ({}). If it is on an external drive, connect it; otherwise choose another folder in the settings."),
    ("status_version", "Versión del daemon: {} (protocolo {})", "Daemon version: {} (protocol {})"),
    ("status_uptime", "Encendido hace: {}", "Running for: {}"),
    ("status_folder", "Carpeta de fondos: {}", "Wallpapers folder: {}"),
    ("status_folder_missing", "  (no existe)", "  (does not exist)"),
    ("status_counts", "Fondos: {} válidos, {} con problemas", "Wallpapers: {} valid, {} with problems"),
    ("status_license", "Licencia: {}", "License: {}"),
    ("list_empty", "La biblioteca está vacía. Copia carpetas de fondos a la carpeta de fondos y usa «rescan».",
     "The library is empty. Copy wallpaper folders into the wallpapers folder and run \"rescan\"."),
    ("list_invalid", "con problemas", "with problems"),
    ("list_newer", "hecho con una versión más nueva", "made with a newer version"),
    ("rescan_done", "Listo: {} válidos, {} con problemas, {} quitados.", "Done: {} valid, {} with problems, {} removed."),
    ("pong", "El daemon responde.", "The daemon answers."),
    ("stopping", "El daemon se está apagando.", "The daemon is stopping."),
];

pub fn t(lang: Language, key: &str) -> &'static str {
    let Some((_, es, en)) = TEXTS.iter().find(|(k, _, _)| *k == key) else {
        return "";
    };
    match lang {
        Language::Es => es,
        _ => en,
    }
}

/// Reemplaza cada `{}` en orden.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = template;
    for a in args {
        match rest.split_once("{}") {
            Some((before, after)) => {
                out.push_str(before);
                out.push_str(a);
                rest = after;
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_clave_tiene_los_dos_idiomas_y_los_mismos_huecos() {
        for (key, es, en) in TEXTS {
            assert!(!es.is_empty() && !en.is_empty(), "{key}");
            assert_eq!(es.matches("{}").count(), en.matches("{}").count(), "{key}");
        }
        let mut keys: Vec<_> = TEXTS.iter().map(|(k, _, _)| *k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), TEXTS.len(), "clave repetida");
    }

    #[test]
    fn rellena_en_orden() {
        assert_eq!(fill("{} y {}", &["a", "b"]), "a y b");
        assert_eq!(fill("sin huecos", &["a"]), "sin huecos");
    }
}
