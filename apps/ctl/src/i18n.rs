//! Textos de `ctl` en español e inglés. Cada clave tiene los dos idiomas (lo comprueba la prueba).

use miraishin_store::Language;

/// (clave, español, inglés). `{}` se reemplaza en orden con `fill`.
const TEXTS: &[(&str, &str, &str)] = &[
    ("usage",
     "{} — consola del daemon\n\nUso: miraishin-ctl <comando> [opciones] [--json]\n\nComandos:\n  status                 estado del daemon, de la biblioteca y del fondo en pantalla\n  list                   fondos de la biblioteca (válidos e inválidos)\n  rescan                 vuelve a leer la carpeta de fondos\n  outputs                monitores y el fondo de cada uno\n  set <carpeta>          pone un fondo (por defecto en todos los monitores)\n      --monitor <nombre>   solo en ese monitor (ver «outputs»)\n      --mode <modo>        fill (rellenar, por defecto), fit (ajustar), stretch (estirar), center (centrar)\n  clear                  quita el fondo de todos los monitores\n      --monitor <nombre>   solo de ese monitor\n  ping                   comprueba que el daemon responde\n  shutdown               apaga el daemon\n\n--json     muestra la respuesta tal cual, en JSON",
     "{} — daemon console\n\nUsage: miraishin-ctl <command> [options] [--json]\n\nCommands:\n  status                 daemon, library and on-screen wallpaper status\n  list                   wallpapers in the library (valid and invalid)\n  rescan                 read the wallpapers folder again\n  outputs                monitors and the wallpaper on each one\n  set <folder>           set a wallpaper (on all monitors by default)\n      --monitor <name>     only on that monitor (see \"outputs\")\n      --mode <mode>        fill (default), fit, stretch, center\n  clear                  remove the wallpaper from all monitors\n      --monitor <name>     only from that monitor\n  ping                   check that the daemon answers\n  shutdown               stop the daemon\n\n--json     print the raw JSON answer"),
    ("unknown_option", "Opción desconocida: {}", "Unknown option: {}"),
    ("missing_value", "Falta el valor de {}", "Missing the value of {}"),
    ("extra_argument", "Sobra: {}", "Unexpected: {}"),
    ("set_needs_folder", "Falta la carpeta del fondo: miraishin-ctl set <carpeta>. Las carpetas salen en «list».",
     "Missing the wallpaper folder: miraishin-ctl set <folder>. Folders are shown by \"list\"."),
    ("applied", "Listo.", "Done."),
    ("saved_not_shown", "Guardado. Se verá en cuanto este escritorio pueda mostrar el fondo (mira «status»).",
     "Saved. It will show as soon as this desktop can display the wallpaper (see \"status\")."),
    ("err_wallpaper_not_found", "No hay un fondo con esa carpeta. Mira las carpetas con «list» (y usa «rescan» si lo acabas de copiar).",
     "There is no wallpaper with that folder. See the folders with \"list\" (and run \"rescan\" if you just copied it)."),
    ("err_wallpaper_invalid", "Ese fondo tiene problemas y no se puede poner. «list» muestra cuáles.",
     "That wallpaper has problems and cannot be set. \"list\" shows which ones."),
    ("err_kind_unsupported", "Ese tipo de fondo todavía no se puede mostrar. No es un error: los videos llegan en una próxima versión.",
     "That kind of wallpaper cannot be shown yet. This is not an error: videos come in an upcoming version."),
    ("err_unknown_output", "No hay un monitor con ese nombre. «outputs» muestra los nombres.",
     "There is no monitor with that name. \"outputs\" shows the names."),
    ("err_invalid_mode", "Modo desconocido. Usa fill, fit, stretch o center.",
     "Unknown mode. Use fill, fit, stretch or center."),
    ("surface_ok", "Fondo en pantalla: sí ({})", "On-screen wallpaper: yes ({})"),
    ("surface_kde_pending",
     "Fondo en pantalla: todavía no en KDE Plasma ({}). No es un error: en KDE el fondo va por un complemento de Plasma que llega en una próxima versión. Lo que elijas se guarda.",
     "On-screen wallpaper: not yet on KDE Plasma ({}). This is not an error: on KDE the wallpaper goes through a Plasma add-on that comes in an upcoming version. What you choose is saved."),
    ("surface_no_wayland",
     "Fondo en pantalla: no ({}). No hay una sesión Wayland: en X11 llega en una próxima versión. No es un error; la biblioteca funciona igual.",
     "On-screen wallpaper: no ({}). There is no Wayland session: X11 comes in an upcoming version. This is not an error; the library works the same."),
    ("surface_no_layer_shell",
     "Fondo en pantalla: no ({}). Este escritorio (por ejemplo GNOME) no permite poner fondos así; llega en una próxima versión. No es un error.",
     "On-screen wallpaper: no ({}). This desktop (for example GNOME) does not allow wallpapers this way; it comes in an upcoming version. This is not an error."),
    ("surface_platform_pending",
     "Fondo en pantalla: todavía no en este sistema ({}). No es un error: llega en una próxima versión.",
     "On-screen wallpaper: not yet on this system ({}). This is not an error: it comes in an upcoming version."),
    ("surface_unavailable", "Fondo en pantalla: no disponible ({}).", "On-screen wallpaper: not available ({})."),
    ("outputs_none", "No se ve ningún monitor.", "No monitors found."),
    ("outputs_all", "Para todos: {} [{}]", "For all: {} [{}]"),
    ("outputs_showing", "fondo: {} [{}]", "wallpaper: {} [{}]"),
    ("outputs_problem", "no se pudo mostrar {} ({}); se ve el color de respaldo", "could not show {} ({}); showing the fallback color"),
    ("outputs_empty", "sin fondo", "no wallpaper"),
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
