//! Velocidad, brillo y límite de cuadros del video de fondo (fase 3.5). Valen para todos los
//! monitores a la vez. Aquí solo las reglas; cada superficie los aplica a su manera:
//! Sway/Hyprland con mpv (`speed`, `brightness` y un filtro que descarta cuadros), KDE con
//! el reproductor de Plasma (velocidad y brillo) y una copia del video para el límite.

use crate::copies::{CopyTarget, VideoInfo};
use crate::surface::Size;

pub const SPEED_MIN: f64 = 0.25;
pub const SPEED_MAX: f64 = 4.0;
pub const BRIGHTNESS_MIN: i32 = -100;
pub const BRIGHTNESS_MAX: i32 = 100;
/// Límite de cuadros más alto que se acepta (0 = sin límite).
pub const FPS_LIMIT_MAX: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSettings {
    /// 1 = normal; de `SPEED_MIN` a `SPEED_MAX`, con dos decimales.
    pub speed: f64,
    /// 0 = normal; de −100 (negro) a 100 (blanco).
    pub brightness: i32,
    /// Cuadros por segundo como máximo; 0 = sin límite.
    pub fps_limit: u32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            brightness: 0,
            fps_limit: 0,
        }
    }
}

impl PlaybackSettings {
    /// ¿Todo en su valor normal? (Entonces no hace falta tocar el reproductor.)
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// La velocidad como texto corto para mpv, Plasma y `ctl`: `1`, `1.5`, `0.25`.
    pub fn speed_text(&self) -> String {
        speed_text(self.speed)
    }
}

/// Velocidad válida, redondeada a dos decimales; `None` si está fuera del rango.
pub fn check_speed(speed: f64) -> Option<f64> {
    if !speed.is_finite() {
        return None;
    }
    let rounded = (speed * 100.0).round() / 100.0;
    (SPEED_MIN..=SPEED_MAX)
        .contains(&rounded)
        .then_some(rounded)
}

pub fn check_brightness(brightness: i64) -> Option<i32> {
    let b = i32::try_from(brightness).ok()?;
    (BRIGHTNESS_MIN..=BRIGHTNESS_MAX).contains(&b).then_some(b)
}

pub fn check_fps_limit(limit: i64) -> Option<u32> {
    let l = u32::try_from(limit).ok()?;
    (l <= FPS_LIMIT_MAX).then_some(l)
}

pub fn speed_text(speed: f64) -> String {
    let s = format!("{speed:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Filtro de mpv (`vf`) para el límite: descarta cuadros de más y nunca agrega (un video de
/// 24 cuadros con límite 30 queda en 24). Vacío = sin filtro.
pub fn mpv_fps_filter(limit: u32) -> String {
    if limit == 0 {
        return String::new();
    }
    format!("lavfi=[fps=fps='min(source_fps,{limit})']")
}

/// Filtro de reserva por si el de arriba no se puede usar (mpv no sabe los cuadros del
/// video): un ritmo fijo. Puede repetir cuadros en un video más lento que el límite.
pub fn mpv_fps_filter_plain(limit: u32) -> String {
    if limit == 0 {
        return String::new();
    }
    format!("lavfi=[fps={limit}]")
}

/// Brillo de Plasma (`MultiEffect.brightness`, de −1 a 1) como texto.
pub fn plasma_brightness(brightness: i32) -> String {
    let b = f64::from(brightness.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX)) / 100.0;
    speed_text(b)
}

/// Límite de cuadros donde el reproductor no lo sabe hacer (KDE): con qué copia se ve el
/// video. `target` es la copia que ya tocaba por compatibilidad (`None` = el original).
/// Con el original, si el video no pasa del límite (y su códec se ve en todas partes)
/// sigue el original; si no, una copia del tamaño de la pantalla a ese ritmo. Una copia que
/// ya tocaba solo baja sus cuadros. Sin límite, no cambia nada.
pub fn limit_copy(
    target: Option<CopyTarget>,
    limit: u32,
    largest: Size,
    info: Option<&VideoInfo>,
) -> Option<CopyTarget> {
    if limit == 0 {
        return target;
    }
    match target {
        Some(t) => Some(CopyTarget {
            fps_cap: t.fps_cap.min(limit),
            ..t
        }),
        None => {
            let screen = CopyTarget {
                fps_cap: limit,
                ..CopyTarget::screen(largest)
            };
            // Solo los cuadros: el tamaño del original no importa para el límite.
            let fits = info.is_some_and(|i| {
                crate::copies::already_fits(
                    i,
                    &CopyTarget {
                        max_width: u32::MAX,
                        max_height: u32::MAX,
                        fps_cap: limit,
                    },
                )
            });
            (!fits).then_some(screen)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(codec: &str, fps: f64) -> VideoInfo {
        VideoInfo {
            codec: codec.into(),
            width: 3840,
            height: 2160,
            fps,
        }
    }

    #[test]
    fn rangos() {
        assert_eq!(check_speed(1.5), Some(1.5));
        assert_eq!(check_speed(0.254), Some(0.25));
        assert_eq!(check_speed(0.2), None);
        assert_eq!(check_speed(4.01), None);
        assert_eq!(check_speed(f64::NAN), None);
        assert_eq!(check_brightness(-100), Some(-100));
        assert_eq!(check_brightness(101), None);
        assert_eq!(check_fps_limit(0), Some(0));
        assert_eq!(check_fps_limit(240), Some(240));
        assert_eq!(check_fps_limit(241), None);
        assert_eq!(check_fps_limit(-1), None);
    }

    #[test]
    fn textos() {
        assert_eq!(speed_text(1.0), "1");
        assert_eq!(speed_text(1.5), "1.5");
        assert_eq!(speed_text(0.25), "0.25");
        assert_eq!(plasma_brightness(-30), "-0.3");
        assert_eq!(plasma_brightness(0), "0");
        assert_eq!(mpv_fps_filter(0), "");
        assert_eq!(mpv_fps_filter(30), "lavfi=[fps=fps='min(source_fps,30)']");
        assert_eq!(mpv_fps_filter_plain(24), "lavfi=[fps=24]");
        assert!(PlaybackSettings::default().is_default());
    }

    #[test]
    fn copia_por_limite() {
        let screen = Size::new(1920, 1080);
        // Sin límite: lo que tocaba.
        assert_eq!(limit_copy(None, 0, screen, None), None);
        assert_eq!(
            limit_copy(Some(CopyTarget::SAVER), 0, screen, None),
            Some(CopyTarget::SAVER)
        );
        // Original de 60 con límite 30: copia del tamaño de la pantalla a 30.
        let t = limit_copy(None, 30, screen, Some(&info("h264", 60.0))).unwrap();
        assert_eq!((t.max_width, t.max_height, t.fps_cap), (1920, 1080, 30));
        // Original de 24 (aunque sea 4K): sigue el original.
        assert_eq!(
            limit_copy(None, 30, screen, Some(&info("h264", 24.0))),
            None
        );
        // HEVC no se ve en todas partes: copia igual.
        assert!(limit_copy(None, 30, screen, Some(&info("hevc", 24.0))).is_some());
        // Sin saber cómo es el video: copia.
        assert!(limit_copy(None, 30, screen, None).is_some());
        // La copia de ahorro (30) con límite 15 baja a 15; con límite 60 queda en 30.
        assert_eq!(
            limit_copy(Some(CopyTarget::SAVER), 15, screen, None).map(|t| t.fps_cap),
            Some(15)
        );
        assert_eq!(
            limit_copy(Some(CopyTarget::SAVER), 60, screen, None).map(|t| t.fps_cap),
            Some(30)
        );
    }
}
