//! Encuadre (fase 1.2): qué parte de la imagen va a qué parte del monitor.
//!
//! Solo cuentas con enteros: entra el tamaño de la imagen, el del monitor y el modo, y sale
//! un par de rectángulos. Lo que queda fuera de `dst` se pinta con el color de fondo.

use serde::Serialize;

use crate::surface::{full_rect, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    /// Cubre todo el monitor y recorta lo que sobra, centrado. Por defecto.
    #[default]
    Fill,
    /// Se ve la imagen entera; sobran bordes del color de fondo.
    Fit,
    /// Ocupa todo el monitor deformando la imagen.
    Stretch,
    /// Tamaño real, sin escalar, centrada (recorta si es más grande que el monitor).
    Center,
}

impl FitMode {
    pub const ALL: [Self; 4] = [Self::Fill, Self::Fit, Self::Stretch, Self::Center];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Fit => "fit",
            Self::Stretch => "stretch",
            Self::Center => "center",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// Resultado del encuadre: el trozo `src` de la imagen se escala a `dst` en el monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Placement {
    pub src: Rect,
    pub dst: Rect,
}

impl Placement {
    /// `true` si la imagen no cubre todo el monitor y hay que pintar el color de fondo.
    pub fn needs_background(&self, target: Size) -> bool {
        self.dst != full_rect(target)
    }
}

/// Calcula el encuadre. `None` si la imagen o el monitor miden 0.
pub fn place(image: Size, target: Size, mode: FitMode) -> Option<Placement> {
    if image.is_empty() || target.is_empty() {
        return None;
    }
    let (iw, ih) = (u64::from(image.width), u64::from(image.height));
    let (tw, th) = (u64::from(target.width), u64::from(target.height));

    let placement = match mode {
        FitMode::Stretch => Placement {
            src: full_rect(image),
            dst: full_rect(target),
        },
        FitMode::Fill => {
            // Más ancha que el monitor (iw/ih > tw/th): se recorta a los lados.
            let src = if iw * th > ih * tw {
                let w = div_round(ih * tw, th).clamp(1, iw);
                Rect::new(((iw - w) / 2) as u32, 0, w as u32, ih as u32)
            } else {
                let h = div_round(iw * th, tw).clamp(1, ih);
                Rect::new(0, ((ih - h) / 2) as u32, iw as u32, h as u32)
            };
            Placement {
                src,
                dst: full_rect(target),
            }
        }
        FitMode::Fit => {
            let dst = if iw * th > ih * tw {
                let h = div_round(ih * tw, iw).clamp(1, th);
                Rect::new(0, ((th - h) / 2) as u32, tw as u32, h as u32)
            } else {
                let w = div_round(iw * th, ih).clamp(1, tw);
                Rect::new(((tw - w) / 2) as u32, 0, w as u32, th as u32)
            };
            Placement {
                src: full_rect(image),
                dst,
            }
        }
        FitMode::Center => {
            let (sx, dx, w) = center_axis(iw, tw);
            let (sy, dy, h) = center_axis(ih, th);
            Placement {
                src: Rect::new(sx, sy, w, h),
                dst: Rect::new(dx, dy, w, h),
            }
        }
    };
    Some(placement)
}

/// En un eje, sin escalar: si la imagen es más grande se recorta al centro; si es más chica
/// se centra. Devuelve (inicio en la imagen, inicio en el monitor, largo).
fn center_axis(image: u64, target: u64) -> (u32, u32, u32) {
    if image >= target {
        (((image - target) / 2) as u32, 0, target as u32)
    } else {
        (0, ((target - image) / 2) as u32, image as u32)
    }
}

fn div_round(n: u64, d: u64) -> u64 {
    (n + d / 2) / d
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Size = Size::new(1920, 1080);

    fn p(image: Size, mode: FitMode) -> Placement {
        place(image, FHD, mode).unwrap()
    }

    #[test]
    fn nombres_de_modos() {
        for m in FitMode::ALL {
            assert_eq!(FitMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(FitMode::parse("zoom"), None);
        assert_eq!(FitMode::default(), FitMode::Fill);
    }

    #[test]
    fn tamanos_cero() {
        assert_eq!(place(Size::new(0, 10), FHD, FitMode::Fill), None);
        assert_eq!(place(FHD, Size::new(10, 0), FitMode::Fit), None);
    }

    #[test]
    fn misma_proporcion_en_todos_los_modos() {
        let img = Size::new(3840, 2160);
        for m in [FitMode::Fill, FitMode::Fit, FitMode::Stretch] {
            let r = p(img, m);
            assert_eq!(r.src, Rect::new(0, 0, 3840, 2160), "{m:?}");
            assert_eq!(r.dst, Rect::new(0, 0, 1920, 1080), "{m:?}");
            assert!(!r.needs_background(FHD));
        }
        // Centrar no escala: toma el centro de la imagen 4K.
        let c = p(img, FitMode::Center);
        assert_eq!(c.src, Rect::new(960, 540, 1920, 1080));
        assert_eq!(c.dst, Rect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn imagen_mas_ancha() {
        let img = Size::new(2560, 1080); // 21:9 en un monitor 16:9
        let fill = p(img, FitMode::Fill);
        assert_eq!(fill.src, Rect::new(320, 0, 1920, 1080));
        assert_eq!(fill.dst, Rect::new(0, 0, 1920, 1080));

        let fit = p(img, FitMode::Fit);
        assert_eq!(fit.src, Rect::new(0, 0, 2560, 1080));
        assert_eq!(fit.dst, Rect::new(0, 135, 1920, 810));
        assert!(fit.needs_background(FHD));
    }

    #[test]
    fn imagen_mas_alta() {
        let img = Size::new(1080, 1920); // vertical
        let fill = p(img, FitMode::Fill);
        assert_eq!(fill.src, Rect::new(0, 656, 1080, 608));

        let fit = p(img, FitMode::Fit);
        assert_eq!(fit.dst, Rect::new(656, 0, 608, 1080));
    }

    #[test]
    fn imagen_mas_chica() {
        let img = Size::new(800, 600);
        // Rellenar agranda y recorta arriba y abajo (4:3 en 16:9).
        let fill = p(img, FitMode::Fill);
        assert_eq!(fill.src, Rect::new(0, 75, 800, 450));
        // Ajustar agranda hasta el alto y deja bordes a los lados.
        let fit = p(img, FitMode::Fit);
        assert_eq!(fit.dst, Rect::new(240, 0, 1440, 1080));
        // Centrar la deja de su tamaño en el medio.
        let c = p(img, FitMode::Center);
        assert_eq!(c.src, Rect::new(0, 0, 800, 600));
        assert_eq!(c.dst, Rect::new(560, 240, 800, 600));
        assert!(c.needs_background(FHD));
        // Estirar ocupa todo.
        assert_eq!(p(img, FitMode::Stretch).dst, Rect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn centrar_mixto() {
        // Más ancha que el monitor pero más baja: recorta a los lados, centra en alto.
        let c = p(Size::new(3000, 500), FitMode::Center);
        assert_eq!(c.src, Rect::new(540, 0, 1920, 500));
        assert_eq!(c.dst, Rect::new(0, 290, 1920, 500));
    }

    #[test]
    fn extremos_no_dan_cero() {
        // Una línea de 1 píxel de alto sigue dando rectángulos de al menos 1.
        let fit = p(Size::new(10_000, 1), FitMode::Fit);
        assert_eq!(fit.dst.height, 1);
        let fill = p(Size::new(1, 10_000), FitMode::Fill);
        assert_eq!(fill.src.height, 1);
        // Medidas muy grandes no se desbordan.
        let big = Size::new(u32::MAX, u32::MAX);
        assert!(place(big, FHD, FitMode::Fill).is_some());
        assert!(place(FHD, big, FitMode::Fit).is_some());
    }

    #[test]
    fn el_monitor_de_nobara() {
        // 1918×920 (VMware) es más ancho que 16:9: una foto 16:9 se recorta arriba y abajo.
        let r = place(Size::new(1920, 1080), Size::new(1918, 920), FitMode::Fill).unwrap();
        assert_eq!(r.src, Rect::new(0, 79, 1920, 921));
    }
}
