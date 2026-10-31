//! Reglas puras: qué opciones de mpv se usan y cómo se traduce cada error. Sin libmpv.

use miraishin_core::fit::FitMode;

/// Grafito del sistema de diseño (bordes de «ajustar» y «centrar»).
pub const BACKGROUND: &str = "#1c1d21";

/// Opciones fijas, antes de `mpv_initialize`. Ninguna lee la configuración del usuario ni
/// abre el sonido.
pub const BASE: &[(&str, &str)] = &[
    ("config", "no"),
    ("load-scripts", "no"),
    ("osc", "no"),
    ("input-default-bindings", "no"),
    ("input-vo-keyboard", "no"),
    ("terminal", "no"),
    ("vo", "libmpv"),
    ("loop-file", "inf"),
    ("audio", "no"),
    ("ao", "null"),
    ("sub-auto", "no"),
    ("idle", "yes"),
    ("keep-open", "yes"),
    // Decodificación por hardware solo con métodos que mpv considera seguros; si no hay,
    // usa la CPU sin error.
    ("hwdec", "auto-safe"),
    // Escalado y dibujo baratos: es un fondo, no una película; prioridad bajo consumo.
    ("profile", "fast"),
    // Scripts que trae mpv dentro (no son archivos: `load-scripts` no los apaga).
    ("ytdl", "no"),
    ("load-stats-overlay", "no"),
    ("load-console", "no"),
    ("load-auto-profiles", "no"),
    // No buscar DVD, Blu-ray ni archivos externos junto al video.
    ("audio-file-auto", "no"),
    ("cover-art-auto", "no"),
];

/// Opciones del encuadre. Se aplican antes de cargar el archivo.
pub fn mode(mode: FitMode) -> Vec<(&'static str, &'static str)> {
    match mode {
        // Cubre todo y recorta lo que sobra.
        FitMode::Fill => vec![
            ("keepaspect", "yes"),
            ("panscan", "1.0"),
            ("video-unscaled", "no"),
        ],
        FitMode::Fit => vec![
            ("keepaspect", "yes"),
            ("panscan", "0.0"),
            ("video-unscaled", "no"),
        ],
        FitMode::Stretch => vec![
            ("keepaspect", "no"),
            ("panscan", "0.0"),
            ("video-unscaled", "no"),
        ],
        // Tamaño real, centrado (recorta si es más grande).
        FitMode::Center => vec![
            ("keepaspect", "yes"),
            ("panscan", "0.0"),
            ("video-unscaled", "yes"),
        ],
    }
}

/// Color de fondo: el nombre de la opción cambió en mpv 0.38 (`background=color` +
/// `background-color`); en 0.37 era `background=<color>`. Se prueban las dos, en orden.
pub fn background() -> Vec<Vec<(&'static str, &'static str)>> {
    vec![
        vec![("background", "color"), ("background-color", BACKGROUND)],
        vec![("background", BACKGROUND)],
    ]
}

/// Código estable del final de un archivo, o `None` si no fue un error.
/// `reason` y `error` son los de `mpv_event_end_file`.
pub fn end_file_code(reason: i32, error: i32) -> Option<&'static str> {
    const REASON_ERROR: i32 = 4;
    // MPV_ERROR_LOADING_FAILED = -13, MPV_ERROR_UNKNOWN_FORMAT = -17,
    // MPV_ERROR_NOTHING_TO_PLAY = -16.
    if reason != REASON_ERROR {
        return None;
    }
    Some(match error {
        -17 | -16 => "video_no_decoder",
        _ => "video_unplayable",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_sonido_ni_configuracion_del_usuario() {
        let get = |k: &str| BASE.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
        assert_eq!(get("audio"), Some("no"));
        assert_eq!(get("config"), Some("no"));
        assert_eq!(get("vo"), Some("libmpv"));
        assert_eq!(get("loop-file"), Some("inf"));
    }

    #[test]
    fn modos() {
        assert!(mode(FitMode::Fill).contains(&("panscan", "1.0")));
        assert!(mode(FitMode::Stretch).contains(&("keepaspect", "no")));
        assert!(mode(FitMode::Center).contains(&("video-unscaled", "yes")));
        // Cada modo deja las tres opciones en un valor conocido (al cambiar de modo no queda
        // nada del anterior).
        for m in FitMode::ALL {
            let names: Vec<_> = mode(m).iter().map(|(n, _)| *n).collect();
            assert_eq!(names, ["keepaspect", "panscan", "video-unscaled"]);
        }
    }

    #[test]
    fn errores_de_fin_de_archivo() {
        assert_eq!(end_file_code(0, 0), None); // fin normal
        assert_eq!(end_file_code(2, 0), None); // detenido
        assert_eq!(end_file_code(4, -17), Some("video_no_decoder"));
        assert_eq!(end_file_code(4, -13), Some("video_unplayable"));
    }
}
