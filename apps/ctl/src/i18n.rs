//! Textos de `ctl` en español e inglés. Cada clave tiene los dos idiomas (lo comprueba la prueba).

use miraishin_store::Language;

/// (clave, español, inglés). `{}` se reemplaza en orden con `fill`.
const TEXTS: &[(&str, &str, &str)] = &[
    ("usage",
     "{} — consola del daemon\n\nUso: miraishin-ctl <comando> [opciones] [--json]\n\nComandos:\n  status                 estado del daemon, de la biblioteca y del fondo en pantalla\n  list                   fondos de la biblioteca (válidos e inválidos)\n  rescan                 vuelve a leer la carpeta de fondos\n  outputs                monitores y el fondo de cada uno\n  set <carpeta>          pone un fondo (por defecto en todos los monitores)\n      --monitor <nombre>   solo en ese monitor (ver «outputs»)\n      --mode <modo>        fill (rellenar, por defecto), fit (ajustar), stretch (estirar), center (centrar)\n  clear                  quita el fondo de todos los monitores\n      --monitor <nombre>   solo de ese monitor\n  pause                  pausa el video de fondo en todos los monitores (hasta «resume» o reiniciar el daemon)\n      --monitor <nombre>   solo en ese monitor\n  resume                 vuelve a reproducir el video pausado con «pause»\n      --monitor <nombre>   solo en ese monitor\n  speed <n>              velocidad del video, de 0,25 a 4 (1 = normal)\n  brightness <n>         brillo del video, de -100 (negro) a 100 (blanco); 0 = normal\n  fps <n>                cuadros por segundo como máximo, de 1 a 240; «fps off» quita el límite\n  ping                   comprueba que el daemon responde\n  shutdown               apaga el daemon\n\n--json     muestra la respuesta tal cual, en JSON",
     "{} — daemon console\n\nUsage: miraishin-ctl <command> [options] [--json]\n\nCommands:\n  status                 daemon, library and on-screen wallpaper status\n  list                   wallpapers in the library (valid and invalid)\n  rescan                 read the wallpapers folder again\n  outputs                monitors and the wallpaper on each one\n  set <folder>           set a wallpaper (on all monitors by default)\n      --monitor <name>     only on that monitor (see \"outputs\")\n      --mode <mode>        fill (default), fit, stretch, center\n  clear                  remove the wallpaper from all monitors\n      --monitor <name>     only from that monitor\n  pause                  pause the video wallpaper on all monitors (until \"resume\" or a daemon restart)\n      --monitor <name>     only on that monitor\n  resume                 play again the video paused with \"pause\"\n      --monitor <name>     only on that monitor\n  speed <n>              video speed, from 0.25 to 4 (1 = normal)\n  brightness <n>         video brightness, from -100 (black) to 100 (white); 0 = normal\n  fps <n>                frames per second at most, from 1 to 240; \"fps off\" removes the limit\n  ping                   check that the daemon answers\n  shutdown               stop the daemon\n\n--json     print the raw JSON answer"),
    ("unknown_option", "Opción desconocida: {}", "Unknown option: {}"),
    ("missing_value", "Falta el valor de {}", "Missing the value of {}"),
    ("extra_argument", "Sobra: {}", "Unexpected: {}"),
    ("bad_number", "No es un número válido: {}", "Not a valid number: {}"),
    ("needs_value", "Falta el valor: miraishin-ctl {} <número>", "Missing the value: miraishin-ctl {} <number>"),
    ("set_needs_folder", "Falta la carpeta del fondo: miraishin-ctl set <carpeta>. Las carpetas salen en «list».",
     "Missing the wallpaper folder: miraishin-ctl set <folder>. Folders are shown by \"list\"."),
    ("applied", "Listo.", "Done."),
    ("saved_not_shown", "Guardado. Se verá en cuanto este escritorio pueda mostrar el fondo (mira «status»).",
     "Saved. It will show as soon as this desktop can display the wallpaper (see \"status\")."),
    ("err_wallpaper_not_found", "No hay un fondo con esa carpeta. Mira las carpetas con «list» (y usa «rescan» si lo acabas de copiar).",
     "There is no wallpaper with that folder. See the folders with \"list\" (and run \"rescan\" if you just copied it)."),
    ("err_wallpaper_invalid", "Ese fondo tiene problemas y no se puede poner. «list» muestra cuáles.",
     "That wallpaper has problems and cannot be set. \"list\" shows which ones."),
    ("err_kind_unsupported", "Esta versión no sabe mostrar ese tipo de fondo. No es un error: probablemente lo hizo una versión más nueva.",
     "This version cannot show that kind of wallpaper. This is not an error: it was probably made with a newer version."),
    ("err_unknown_output", "No hay un monitor con ese nombre. «outputs» muestra los nombres.",
     "There is no monitor with that name. \"outputs\" shows the names."),
    ("err_invalid_mode", "Modo desconocido. Usa fill, fit, stretch o center.",
     "Unknown mode. Use fill, fit, stretch or center."),
    ("err_out_of_range",
     "Valor fuera de rango. Velocidad: de 0,25 a 4. Brillo: de -100 a 100. Cuadros por segundo: de 1 a 240 (u «off»).",
     "Value out of range. Speed: 0.25 to 4. Brightness: -100 to 100. Frames per second: 1 to 240 (or \"off\")."),
    ("surface_ok", "Fondo en pantalla: sí ({})", "On-screen wallpaper: yes ({})"),
    ("surface_kde_restart",
     "Fondo en pantalla: falta un paso en KDE Plasma ({}). No es un error: se instaló el complemento de fondo y Plasma lo ve después de reiniciarse. Cierra sesión y vuelve a entrar (o reinicia Plasma) y luego vuelve a abrir el daemon. Lo que elijas se guarda.",
     "On-screen wallpaper: one step left on KDE Plasma ({}). This is not an error: the wallpaper add-on was installed and Plasma sees it after restarting. Log out and back in (or restart Plasma), then open the daemon again. What you choose is saved."),
    ("surface_kde_unavailable",
     "Fondo en pantalla: no ({}). No se pudo hablar con Plasma (plasmashell). Si Plasma está abierto, vuelve a abrir el daemon. Lo que elijas se guarda.",
     "On-screen wallpaper: no ({}). Could not talk to Plasma (plasmashell). If Plasma is running, open the daemon again. What you choose is saved."),
    ("surface_kde_install_failed",
     "Fondo en pantalla: no ({}). No se pudo instalar el complemento de fondo en ~/.local/share/plasma/wallpapers. Revisa que esa carpeta se pueda escribir.",
     "On-screen wallpaper: no ({}). Could not install the wallpaper add-on in ~/.local/share/plasma/wallpapers. Check that the folder is writable."),
    ("surface_no_wayland",
     "Fondo en pantalla: no ({}). No hay una sesión Wayland: en X11 llega en una próxima versión. No es un error; la biblioteca funciona igual.",
     "On-screen wallpaper: no ({}). There is no Wayland session: X11 comes in an upcoming version. This is not an error; the library works the same."),
    ("surface_no_layer_shell",
     "Fondo en pantalla: no ({}). Este escritorio (por ejemplo GNOME) no permite poner fondos así; llega en una próxima versión. No es un error.",
     "On-screen wallpaper: no ({}). This desktop (for example GNOME) does not allow wallpapers this way; it comes in an upcoming version. This is not an error."),
    ("surface_platform_pending",
     "Fondo en pantalla: todavía no en este sistema ({}). No es un error: llega en una próxima versión.",
     "On-screen wallpaper: not yet on this system ({}). This is not an error: it comes in an upcoming version."),
    ("surface_windows_no_desktop",
     "Fondo en pantalla: no ({}). No se encontró el escritorio de Windows (el Explorador no está abierto). Si lo reinicias, vuelve a abrir el daemon. Lo que elijas se guarda.",
     "On-screen wallpaper: no ({}). The Windows desktop was not found (File Explorer is not running). If you restart it, open the daemon again. What you choose is saved."),
    ("surface_unavailable", "Fondo en pantalla: no disponible ({}).", "On-screen wallpaper: not available ({})."),
    ("outputs_none", "No se ve ningún monitor.", "No monitors found."),
    ("outputs_all", "Para todos: {} [{}]", "For all: {} [{}]"),
    ("outputs_showing", "fondo: {} [{}]", "wallpaper: {} [{}]"),
    ("outputs_showing_copy", "fondo: {} [{}] — copia compatible de {}, para que este equipo lo reproduzca fluido", "wallpaper: {} [{}] — compatible copy at {}, so this computer plays it smoothly"),
    ("outputs_showing_fps_copy", "fondo: {} [{}] — copia a {} cuadros por segundo (límite de cuadros)", "wallpaper: {} [{}] — copy at {} frames per second (frame limit)"),
    ("outputs_copying_fps",
     "{}: creando una copia a {} cuadros por segundo por el límite de cuadros ({} %). Se hace una sola vez; mientras tanto sigue el fondo anterior",
     "{}: making a copy at {} frames per second for the frame limit ({} %). It is done only once; the previous wallpaper stays meanwhile"),
    ("outputs_copying",
     "{}: creando una copia que este equipo pueda reproducir ({} %). Se hace una sola vez; mientras tanto sigue el fondo anterior",
     "{}: making a copy this computer can play ({} %). It is done only once; the previous wallpaper stays meanwhile"),
    ("outputs_problem", "no se pudo mostrar {} ({}); se ve el color de respaldo", "could not show {} ({}); showing the fallback color"),
    // Problemas de un monitor: «problem_<código>». Si un código no tiene texto propio se usa
    // «outputs_problem».
    ("problem_video_unsupported",
     "{} es un video, y en este escritorio los videos llegan en una próxima versión. No es un error: mientras tanto se ve el color de respaldo",
     "{} is a video, and videos on this desktop come in an upcoming version. This is not an error: the fallback color shows meanwhile"),
    ("problem_mpv_unavailable",
     "{} es un video y falta libmpv, que hace falta para reproducirlo. No es un error de la app: en Linux instala el paquete «mpv» de tu distribución (la versión Flatpak ya la trae); en Windows va con la app (libmpv-2.dll junto a miraishin-daemon.exe), así que vuelve a instalarla. Mientras tanto se ve el color de respaldo",
     "{} is a video and libmpv, which is needed to play it, is missing. This is not an error in the app: on Linux install your distribution's \"mpv\" package (the Flatpak version already includes it); on Windows it comes with the app (libmpv-2.dll next to miraishin-daemon.exe), so reinstall it. The fallback color shows meanwhile"),
    ("problem_video_no_gpu",
     "{}: no se pudo usar la tarjeta gráfica (EGL) para dibujar el video. Se ve el color de respaldo; las imágenes siguen funcionando",
     "{}: the graphics card (EGL) could not be used to draw the video. The fallback color shows; images still work"),
    ("problem_video_no_decoder",
     "{}: este sistema no puede decodificar ese video (formato o códec no disponible). Prueba con otro formato, por ejemplo WebM. Se ve el color de respaldo",
     "{}: this system cannot decode that video (format or codec not available). Try another format, for example WebM. The fallback color shows"),
    ("problem_video_unplayable",
     "{}: el video no se pudo abrir (archivo dañado o no es un video). Se ve el color de respaldo",
     "{}: the video could not be opened (damaged file, or not a video). The fallback color shows"),
    ("problem_video_unreadable",
     "{}: no se encuentra el archivo del video. Si lo moviste o borraste, usa «rescan». Se ve el color de respaldo",
     "{}: the video file is missing. If you moved or deleted it, run \"rescan\". The fallback color shows"),
    ("problem_video_gl_failed",
     "{}: el reproductor de video no pudo usar la tarjeta gráfica. Se ve el color de respaldo",
     "{}: the video player could not use the graphics card. The fallback color shows"),
    ("problem_video_no_window",
     "{}: no se pudo colgar el video en el escritorio porque el Explorador de Windows no respondió. Si reinicias el Explorador, el fondo vuelve solo",
     "{}: the video could not be placed on the desktop because Windows Explorer did not respond. If you restart Explorer, the wallpaper comes back by itself"),
    ("problem_video_no_frames",
     "{}: el video se abrió, pero en 10 segundos no llegó ni un cuadro a la pantalla: este equipo no lo puede mostrar. No es un error de la app: pasa en máquinas virtuales o sin el controlador de la tarjeta gráfica. Prueba con un video de menor resolución (por ejemplo 720p). Se ve el color de respaldo",
     "{}: the video opened, but not a single frame reached the screen in 10 seconds: this computer cannot show it. This is not an error in the app: it happens in virtual machines or without the graphics card driver. Try a lower-resolution video (for example 720p). The fallback color shows"),
    ("problem_video_too_slow",
     "{}: este equipo no da abasto con el video (llegan bastante menos cuadros de los que trae el video, así que se ve a saltos). No es un error de la app: pasa en máquinas virtuales o sin decodificación por la tarjeta gráfica. Prueba con un video de menor resolución (por ejemplo 720p)",
     "{}: this computer cannot keep up with the video (far fewer frames arrive than the video has, so it stutters). This is not an error in the app: it happens in virtual machines or without decoding on the graphics card. Try a lower-resolution video (for example 720p)"),
    ("problem_copy_failed",
     "{}: este equipo no puede reproducir el video tal cual, y no se pudo crear una copia compatible (por ejemplo, falta espacio en disco). Se ve el color de respaldo; al volver a elegirlo se intenta otra vez",
     "{}: this computer cannot play the video as it is, and a compatible copy could not be made (for example, the disk is full). The fallback color shows; choosing it again tries once more"),
    ("problem_video_damaged",
     "{}: el archivo del video está dañado o incompleto (por ejemplo, una descarga que se cortó), así que no se puede abrir. No es un error de la app ni del equipo: vuelve a bajar o copiar el video y elígelo otra vez. Se ve el color de respaldo",
     "{}: the video file is damaged or incomplete (for example, a download that was cut short), so it cannot be opened. This is not an error in the app or the computer: download or copy the video again and choose it once more. The fallback color shows"),
    ("problem_copy_unavailable",
     "{}: este equipo no puede reproducir el video tal cual, y para crear una copia compatible hace falta ffmpeg (la versión Flatpak ya lo trae; fuera de ella, instala el paquete «ffmpeg»). Se ve el color de respaldo",
     "{}: this computer cannot play the video as it is, and making a compatible copy needs ffmpeg (the Flatpak version already includes it; otherwise install the \"ffmpeg\" package). The fallback color shows"),
    ("outputs_empty", "sin fondo", "no wallpaper"),
    ("outputs_paused", "en pausa: {}", "paused: {}"),
    ("pause_manual", "a mano", "by you"),
    ("pause_sleep", "equipo suspendido", "computer asleep"),
    ("pause_locked", "pantalla bloqueada", "screen locked"),
    ("pause_screen_off", "pantalla apagada", "screen off"),
    ("pause_battery", "con batería", "on battery"),
    ("pause_fullscreen", "app en pantalla completa", "app in full screen"),
    ("pause_maximized", "ventana maximizada", "maximized window"),
    ("paused_done", "Listo. Los videos quedan en pausa hasta «resume» (o hasta reiniciar el daemon); las imágenes no cambian.",
     "Done. Videos stay paused until \"resume\" (or until the daemon restarts); images do not change."),
    ("resumed_done", "Listo. Los videos se reproducen, salvo que otra cosa los pause (se dice en cada monitor).",
     "Done. Videos are playing, unless something else pauses them (shown on each monitor)."),
    ("pause_unsupported", "Este escritorio todavía no puede pausar el video: se sigue reproduciendo. No es un error; llega en una próxima versión.",
     "This desktop cannot pause the video yet: it keeps playing. This is not an error; it comes in an upcoming version."),
    ("playback_line", "velocidad {}×, brillo {}, {}", "speed {}×, brightness {}, {}"),
    ("playback_no_limit", "sin límite de cuadros", "no frame limit"),
    ("playback_limit", "hasta {} cuadros por segundo", "up to {} frames per second"),
    ("playback_done", "Listo: {}.", "Done: {}."),
    ("playback_unsupported",
     "Este escritorio todavía no puede cambiar la velocidad, el brillo ni los cuadros: se guardó y se usará cuando pueda. No es un error; llega en una próxima versión.",
     "This desktop cannot change speed, brightness or frames yet: it was saved and will be used when it can. This is not an error; it comes in an upcoming version."),
    ("playback_fps_copy",
     "En KDE Plasma el límite de cuadros se hace con una copia del video a ese ritmo: se crea una sola vez por fondo y mientras tanto sigue el video de antes.",
     "On KDE Plasma the frame limit uses a copy of the video at that rate: it is made only once per wallpaper, and the previous video keeps playing meanwhile."),
    ("playback_not_saved",
     "No se pudo guardar en los ajustes (el archivo está dañado o no se puede escribir): vale hasta que se reinicie el daemon.",
     "Could not save it in the settings (the file is damaged or not writable): it lasts until the daemon restarts."),
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
    ("status_desktop", "Escritorio: {} (sesión {})", "Desktop: {} ({} session)"),
    ("status_desktop_only", "Escritorio: {}", "Desktop: {}"),
    ("status_unknown", "no se sabe", "unknown"),
    ("status_license", "Licencia: {}", "License: {}"),
    ("status_power_ac", "Energía: con cargador", "Power: plugged in"),
    ("status_power_battery", "Energía: con batería", "Power: on battery"),
    ("status_power_unavailable",
     "Energía: no se sabe (el sistema no lo informa); el video no se pausa por batería",
     "Power: unknown (the system does not report it); videos are not paused on battery"),
    ("status_lock_unlocked", "Bloqueo de pantalla: desbloqueada", "Screen lock: unlocked"),
    ("status_lock_locked", "Bloqueo de pantalla: bloqueada", "Screen lock: locked"),
    ("status_lock_unavailable",
     "Bloqueo de pantalla: no se sabe (el sistema no lo informa); el video no se pausa al bloquear",
     "Screen lock: unknown (the system does not report it); videos are not paused on lock"),
    ("status_playback", "Reproducción: {}", "Playback: {}"),
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
    fn cada_problema_de_video_tiene_explicacion() {
        for code in [
            "video_unsupported",
            "mpv_unavailable",
            "video_no_gpu",
            "video_no_decoder",
            "video_unplayable",
            "video_unreadable",
            "video_gl_failed",
            "video_no_window",
            "video_no_frames",
            "video_too_slow",
            "copy_failed",
            "copy_unavailable",
            "video_damaged",
        ] {
            let key = format!("problem_{code}");
            assert!(!t(Language::Es, &key).is_empty(), "{key}");
            assert_eq!(t(Language::En, &key).matches("{}").count(), 1, "{key}");
        }
    }

    #[test]
    fn cada_motivo_de_pausa_tiene_texto() {
        for code in [
            "manual",
            "sleep",
            "locked",
            "screen_off",
            "battery",
            "fullscreen",
            "maximized",
        ] {
            let key = format!("pause_{code}");
            assert!(!t(Language::Es, &key).is_empty(), "{key}");
        }
    }

    #[test]
    fn rellena_en_orden() {
        assert_eq!(fill("{} y {}", &["a", "b"]), "a y b");
        assert_eq!(fill("sin huecos", &["a"]), "sin huecos");
    }
}
