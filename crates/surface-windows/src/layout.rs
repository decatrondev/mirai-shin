//! Cuentas puras de la superficie de Windows (se prueban en cualquier sistema, también en el
//! CI de Linux): escala, posición de cada monitor dentro del escritorio, cambios de monitores
//! y el formato de los píxeles.

use miraishin_core::surface::{OutputInfo, Scale, Size, SurfaceEvent};

/// DPI de Windows al 100 %.
pub const BASE_DPI: u32 = 96;

/// Rectángulo en píxeles reales del escritorio virtual (puede tener coordenadas negativas:
/// monitores a la izquierda o arriba del principal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PxRect {
    pub fn size(self) -> Size {
        Size::new(
            (self.right - self.left).max(0) as u32,
            (self.bottom - self.top).max(0) as u32,
        )
    }
}

/// Un monitor como lo da Windows (`GetMonitorInfo` + `GetDpiForMonitor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// `\\.\DISPLAY1`…
    pub name: String,
    pub rect: PxRect,
    pub dpi: u32,
}

/// Escala desde el DPI del monitor: 96 = 100 %, 120 = 125 %, 144 = 150 %.
pub fn scale_from_dpi(dpi: u32) -> Scale {
    if dpi == 0 {
        return Scale::ONE;
    }
    Scale(((u64::from(dpi) * 120 + u64::from(BASE_DPI) / 2) / u64::from(BASE_DPI)) as u32)
}

/// El monitor en el formato del contrato. Windows da píxeles reales (el proceso tiene
/// conciencia de DPI por monitor); el tamaño lógico se calcula para que `pixel_size()` dé
/// de vuelta esos mismos píxeles.
pub fn output_info(m: &Monitor) -> OutputInfo {
    let scale = scale_from_dpi(m.dpi);
    let px = m.rect.size();
    let logical = Size::new(logical_for(px.width, scale), logical_for(px.height, scale));
    OutputInfo {
        name: m.name.clone(),
        description: String::new(),
        logical,
        scale,
    }
}

/// Medida lógica cuya conversión a píxeles da `px`. Con escalas raras puede no existir una
/// exacta; entonces la más cercana (el dibujo se estira a la ventana real, así que un píxel
/// de diferencia no se nota).
fn logical_for(px: u32, scale: Scale) -> u32 {
    let s = u64::from(scale.0.max(1));
    let guess = ((u64::from(px) * 120 + s / 2) / s) as u32;
    [guess, guess.saturating_sub(1), guess + 1]
        .into_iter()
        .min_by_key(|&l| scale.to_pixels(l).abs_diff(px))
        .unwrap_or(guess)
}

/// Posición y tamaño de nuestra ventana como hija de `parent` (coordenadas relativas a la
/// esquina del padre, que cubre todo el escritorio virtual).
pub fn child_rect(monitor: PxRect, parent_origin: (i32, i32)) -> (i32, i32, i32, i32) {
    (
        monitor.left - parent_origin.0,
        monitor.top - parent_origin.1,
        monitor.right - monitor.left,
        monitor.bottom - monitor.top,
    )
}

/// Avisos entre la lista de monitores anterior y la nueva.
pub fn diff(old: &[OutputInfo], new: &[OutputInfo]) -> Vec<SurfaceEvent> {
    let mut events = Vec::new();
    for o in old {
        if !new.iter().any(|n| n.name == o.name) {
            events.push(SurfaceEvent::OutputRemoved {
                name: o.name.clone(),
            });
        }
    }
    for n in new {
        match old.iter().find(|o| o.name == n.name) {
            None => events.push(SurfaceEvent::OutputAdded(n.clone())),
            Some(o) if o != n => events.push(SurfaceEvent::OutputChanged(n.clone())),
            Some(_) => {}
        }
    }
    events
}

/// Forma del escritorio de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopForm {
    /// Windows 10 y 11 hasta 23H2: `0x052C` crea una `WorkerW` detrás de los íconos.
    Classic,
    /// Windows 11 24H2 en adelante: `Progman` sin mapa de bits propio, los íconos y la
    /// `WorkerW` son hijas de `Progman`.
    Raised,
}

impl DesktopForm {
    /// Nombre de la superficie para `status`.
    pub fn surface_name(self) -> &'static str {
        match self {
            Self::Classic => "windows_classic",
            Self::Raised => "windows_raised",
        }
    }
}

/// `WS_EX_NOREDIRECTIONBITMAP`.
pub const EX_NO_REDIRECTION_BITMAP: u32 = 0x0020_0000;

/// Se detecta por el estilo de `Progman`, no por el número de versión.
pub fn form_from_exstyle(exstyle: u32) -> DesktopForm {
    if exstyle & EX_NO_REDIRECTION_BITMAP != 0 {
        DesktopForm::Raised
    } else {
        DesktopForm::Classic
    }
}

/// Qué se le manda a `Progman` con `0x052C`. Los informes de otros proyectos no coinciden en
/// los argumentos para la forma elevada; por eso se pueden cambiar sin recompilar con una
/// variable de entorno (`<SLUG>_WORKERW`, ver `docs/daemon.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnArgs {
    /// Según la forma: clásica `(0, 0)`, elevada `(0xD, 1)`.
    Auto,
    /// No mandar nada.
    Skip,
    Custom(usize, isize),
}

impl SpawnArgs {
    /// Lee el valor de la variable: vacío o `auto`, `none`, o `w,l` (decimal o `0x…`).
    /// Lo que no se entiende vale `Auto`.
    pub fn parse(text: Option<&str>) -> Self {
        let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) else {
            return Self::Auto;
        };
        if t.eq_ignore_ascii_case("none") {
            return Self::Skip;
        }
        let mut parts = t.split(',').map(str::trim);
        match (
            parts.next().and_then(number),
            parts.next().and_then(number),
            parts.next(),
        ) {
            (Some(w), Some(l), None) => Self::Custom(w as usize, l as isize),
            _ => Self::Auto,
        }
    }

    /// Argumentos a mandar para esta forma, o `None` si no se manda nada.
    pub fn resolve(self, form: DesktopForm) -> Option<(usize, isize)> {
        match (self, form) {
            (Self::Skip, _) => None,
            (Self::Custom(w, l), _) => Some((w, l)),
            (Self::Auto, DesktopForm::Classic) => Some((0, 0)),
            (Self::Auto, DesktopForm::Raised) => Some((0xD, 1)),
        }
    }
}

fn number(s: &str) -> Option<i64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// RGBA → el formato de GDI con 32 bits por píxel (en memoria B, G, R, X). El fondo es
/// opaco: el alfa se descarta.
pub fn rgba_to_bgrx(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for p in rgba.as_chunks::<4>().0 {
        out.extend_from_slice(&[p[2], p[1], p[0], 0xff]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, l: i32, t: i32, r: i32, b: i32, dpi: u32) -> Monitor {
        Monitor {
            name: name.into(),
            rect: PxRect {
                left: l,
                top: t,
                right: r,
                bottom: b,
            },
            dpi,
        }
    }

    #[test]
    fn escala_desde_el_dpi() {
        assert_eq!(scale_from_dpi(96), Scale::ONE);
        assert_eq!(scale_from_dpi(120), Scale(150)); // 125 %
        assert_eq!(scale_from_dpi(144), Scale(180)); // 150 %
        assert_eq!(scale_from_dpi(168), Scale(210)); // 175 %
        assert_eq!(scale_from_dpi(192), Scale(240)); // 200 %
        assert_eq!(scale_from_dpi(0), Scale::ONE);
    }

    #[test]
    fn los_pixeles_vuelven_iguales_o_casi() {
        // Monitores y escalas comunes de Windows. Casi siempre pixel_size() da los píxeles
        // reales; con algunas combinaciones (2560 al 150 %) no existe un tamaño lógico exacto
        // y queda a 1 píxel: la ventana mide lo real y el cuadro se estira ese píxel.
        let mut exact = 0;
        for (w, h) in [
            (1920, 1080),
            (2560, 1440),
            (3840, 2160),
            (1366, 768),
            (3440, 1440),
        ] {
            for dpi in [96, 120, 144, 168, 192, 240] {
                let info = output_info(&mon("D", 0, 0, w, h, dpi));
                let px = info.pixel_size();
                assert!(
                    px.width.abs_diff(w as u32) <= 1 && px.height.abs_diff(h as u32) <= 1,
                    "{w}×{h} a {dpi} dpi da {px:?}"
                );
                if px == Size::new(w as u32, h as u32) {
                    exact += 1;
                }
            }
        }
        assert!(exact >= 25, "solo {exact} de 30 exactos");
        // Los de siempre, exactos.
        for dpi in [96, 120, 144, 192] {
            let info = output_info(&mon("D", 0, 0, 1920, 1080, dpi));
            assert_eq!(info.pixel_size(), Size::new(1920, 1080), "{dpi} dpi");
        }
    }

    #[test]
    fn monitor_al_125() {
        let info = output_info(&mon("\\\\.\\DISPLAY1", 0, 0, 1920, 1080, 120));
        assert_eq!(info.logical, Size::new(1536, 864));
        assert_eq!(info.scale.percent(), 125);
    }

    #[test]
    fn posicion_dentro_del_escritorio_virtual() {
        // Principal en (0,0) y otro a la izquierda: Progman empieza en -1280.
        let origin = (-1280, 0);
        let principal = mon("a", 0, 0, 1920, 1080, 96).rect;
        let izquierda = mon("b", -1280, 0, 0, 1024, 96).rect;
        assert_eq!(child_rect(principal, origin), (1280, 0, 1920, 1080));
        assert_eq!(child_rect(izquierda, origin), (0, 0, 1280, 1024));
        // Uno arriba: coordenada negativa en y.
        let arriba = mon("c", 0, -1080, 1920, 0, 96).rect;
        assert_eq!(child_rect(arriba, (0, -1080)), (0, 0, 1920, 1080));
        assert_eq!(child_rect(principal, (0, -1080)), (0, 1080, 1920, 1080));
    }

    #[test]
    fn cambios_de_monitores() {
        let a = output_info(&mon("A", 0, 0, 1920, 1080, 96));
        let b = output_info(&mon("B", 1920, 0, 3840, 1080, 96));
        let a2 = output_info(&mon("A", 0, 0, 1920, 1080, 120));
        let one = std::slice::from_ref(&a);
        assert_eq!(diff(one, one), vec![]);
        assert_eq!(
            diff(one, &[a.clone(), b.clone()]),
            vec![SurfaceEvent::OutputAdded(b.clone())]
        );
        assert_eq!(
            diff(&[a, b], std::slice::from_ref(&a2)),
            vec![
                SurfaceEvent::OutputRemoved { name: "B".into() },
                SurfaceEvent::OutputChanged(a2)
            ]
        );
    }

    #[test]
    fn forma_del_escritorio() {
        assert_eq!(form_from_exstyle(0x0000_0080), DesktopForm::Classic);
        assert_eq!(form_from_exstyle(0x0020_0080), DesktopForm::Raised);
        assert_eq!(DesktopForm::Raised.surface_name(), "windows_raised");
    }

    #[test]
    fn argumentos_de_0x052c() {
        use DesktopForm::*;
        assert_eq!(SpawnArgs::parse(None), SpawnArgs::Auto);
        assert_eq!(SpawnArgs::parse(Some(" auto ")), SpawnArgs::Auto);
        assert_eq!(SpawnArgs::parse(Some("NONE")), SpawnArgs::Skip);
        assert_eq!(SpawnArgs::parse(Some("0,0")), SpawnArgs::Custom(0, 0));
        assert_eq!(SpawnArgs::parse(Some("0xD, 1")), SpawnArgs::Custom(13, 1));
        assert_eq!(SpawnArgs::parse(Some("1,2,3")), SpawnArgs::Auto);
        assert_eq!(SpawnArgs::parse(Some("x,1")), SpawnArgs::Auto);
        assert_eq!(SpawnArgs::Auto.resolve(Classic), Some((0, 0)));
        assert_eq!(SpawnArgs::Auto.resolve(Raised), Some((13, 1)));
        assert_eq!(SpawnArgs::Skip.resolve(Raised), None);
        assert_eq!(SpawnArgs::Custom(0, 0).resolve(Raised), Some((0, 0)));
    }

    #[test]
    fn rgba_a_bgrx() {
        assert_eq!(
            rgba_to_bgrx(&[1, 2, 3, 0, 4, 5, 6, 128]),
            vec![3, 2, 1, 255, 6, 5, 4, 255]
        );
    }
}
