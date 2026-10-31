//! Contrato de superficie (fase 1.2): lo que cualquier forma de poner un fondo debe cumplir.
//!
//! Hay una implementación por plataforma: layer-shell en Sway/Hyprland, el complemento de
//! fondo de Plasma en KDE y, al final de la fase, Windows. El daemon solo habla con este
//! contrato, así que no sabe cuál está detrás.
//!
//! Aquí no hay hilos, disco ni Wayland: solo tipos y reglas.

use std::path::Path;

use serde::Serialize;

use crate::fit::FitMode;

/// Escala de un monitor en 1/120 (la misma unidad que `wp_fractional_scale` de Wayland).
/// 120 = 100 %, 150 = 125 %, 240 = 200 %. Entero para que se pueda comparar sin errores de
/// redondeo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Scale(pub u32);

impl Scale {
    pub const ONE: Self = Self(120);

    /// Desde una escala entera (la que da un compositor sin escala fraccionaria).
    pub fn from_integer(n: u32) -> Self {
        Self(n.max(1).saturating_mul(120))
    }

    /// Porcentaje para mostrar: 120 → 100.
    pub fn percent(self) -> u32 {
        (self.0 * 100 + 60) / 120
    }

    /// Pasa una medida lógica a píxeles reales, redondeando al más cercano.
    pub fn to_pixels(self, logical: u32) -> u32 {
        ((u64::from(logical) * u64::from(self.0) + 60) / 120) as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn full(size: Size) -> Self {
        Self::new(0, 0, size.width, size.height)
    }
}

/// Un monitor tal como lo ve el daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutputInfo {
    /// Nombre estable que da el sistema (`DP-1`, `HDMI-A-1`, `Virtual-1`). Es la clave para
    /// guardar qué fondo va en cada monitor.
    pub name: String,
    /// Marca y modelo, para mostrar. Puede venir vacío.
    pub description: String,
    /// Tamaño lógico (el que usa el escritorio, antes de la escala).
    pub logical: Size,
    pub scale: Scale,
}

impl OutputInfo {
    /// Tamaño en píxeles reales: el de la imagen que hay que dibujar para que se vea nítida.
    pub fn pixel_size(&self) -> Size {
        Size::new(
            self.scale.to_pixels(self.logical.width),
            self.scale.to_pixels(self.logical.height),
        )
    }
}

/// Un cuadro listo para mostrar: píxeles RGBA de 8 bits, fila por fila, sin relleno.
/// Cada implementación lo pasa a su formato (por ejemplo XRGB de `wl_shm`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub size: Size,
    pub rgba: Vec<u8>,
}

impl Frame {
    /// Arma un cuadro comprobando que el tamaño y los bytes cuadran.
    pub fn new(size: Size, rgba: Vec<u8>) -> Result<Self, SurfaceError> {
        let expected = u64::from(size.width) * u64::from(size.height) * 4;
        if size.is_empty() || rgba.len() as u64 != expected {
            return Err(SurfaceError::InvalidFrame);
        }
        Ok(Self { size, rgba })
    }

    /// Cuadro de un solo color (el respaldo del 1.7 usa esto).
    pub fn solid(size: Size, rgba: [u8; 4]) -> Result<Self, SurfaceError> {
        let n = u64::from(size.width) * u64::from(size.height);
        if size.is_empty() || n > usize::MAX as u64 / 4 {
            return Err(SurfaceError::InvalidFrame);
        }
        Ok(Self {
            size,
            rgba: rgba.repeat(n as usize),
        })
    }
}

/// Lo que avisa una superficie cuando cambian los monitores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SurfaceEvent {
    OutputAdded(OutputInfo),
    /// Cambió el tamaño o la escala: hay que volver a dibujar ese monitor.
    OutputChanged(OutputInfo),
    OutputRemoved {
        name: String,
    },
}

/// Por dónde una superficie entrega sus avisos. Se le da al crearla; la llama desde su
/// propio hilo.
pub type EventSink = Box<dyn FnMut(SurfaceEvent) + Send>;

/// Errores con código estable (la app los traduce; aquí no hay texto para la persona).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceError {
    /// No hay un monitor con ese nombre (se desconectó, o el nombre está mal).
    UnknownOutput,
    /// El tamaño y los bytes del cuadro no cuadran.
    InvalidFrame,
    /// El cuadro no tiene el tamaño en píxeles del monitor.
    SizeMismatch,
    /// El sistema de ventanas dejó de responder; la superficie hay que crearla de nuevo.
    Disconnected,
    /// Esta superficie todavía no reproduce video (fase 2): el daemon pone el respaldo.
    VideoUnsupported,
}

impl SurfaceError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownOutput => "unknown_output",
            Self::InvalidFrame => "invalid_frame",
            Self::SizeMismatch => "size_mismatch",
            Self::Disconnected => "disconnected",
            Self::VideoUnsupported => "video_unsupported",
        }
    }
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for SurfaceError {}

/// Lo que cumple cada forma de poner el fondo.
pub trait Surface: Send {
    /// Monitores conectados ahora.
    fn outputs(&self) -> Vec<OutputInfo>;

    /// Muestra un cuadro en un monitor. El cuadro debe medir `pixel_size()` de ese monitor.
    /// Para una imagen fija se llama una vez: la superficie la conserva sin volver a dibujar.
    fn show(&mut self, output: &str, frame: Frame) -> Result<(), SurfaceError>;

    /// Reproduce un video en bucle y sin sonido en un monitor (fase 2). La superficie lo
    /// encuadra con `mode` y lo decodifica a su manera (en KDE lo hace Plasma). Las que
    /// todavía no saben responden `VideoUnsupported`.
    fn show_video(
        &mut self,
        output: &str,
        video: &Path,
        mode: FitMode,
    ) -> Result<(), SurfaceError> {
        let _ = (output, video, mode);
        Err(SurfaceError::VideoUnsupported)
    }

    /// Quita el fondo de un monitor (vuelve a verse el del sistema).
    fn clear(&mut self, output: &str) -> Result<(), SurfaceError>;
}

/// Comprueba que un cuadro sirve para un monitor. Las implementaciones la usan en `show`.
pub fn check_frame(output: &OutputInfo, frame: &Frame) -> Result<(), SurfaceError> {
    if frame.size != output.pixel_size() {
        return Err(SurfaceError::SizeMismatch);
    }
    Ok(())
}

pub(crate) fn full_rect(size: Size) -> Rect {
    Rect::full(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(w: u32, h: u32, scale: u32) -> OutputInfo {
        OutputInfo {
            name: "DP-1".into(),
            description: String::new(),
            logical: Size::new(w, h),
            scale: Scale(scale),
        }
    }

    #[test]
    fn escala_entera_y_fraccionaria() {
        assert_eq!(Scale::from_integer(1), Scale::ONE);
        assert_eq!(Scale::from_integer(2).percent(), 200);
        assert_eq!(Scale::from_integer(0), Scale::ONE);
        assert_eq!(Scale(150).percent(), 125);
        assert_eq!(Scale(180).percent(), 150);
    }

    #[test]
    fn tamano_en_pixeles() {
        assert_eq!(output(1918, 920, 120).pixel_size(), Size::new(1918, 920));
        assert_eq!(output(1280, 720, 240).pixel_size(), Size::new(2560, 1440));
        // 125 %: 1536×864 lógicos = 1920×1080 reales.
        assert_eq!(output(1536, 864, 150).pixel_size(), Size::new(1920, 1080));
        // 150 % con número impar: se redondea al más cercano.
        assert_eq!(Scale(180).to_pixels(1001), 1502);
    }

    #[test]
    fn cuadro_valido_e_invalido() {
        assert!(Frame::new(Size::new(2, 2), vec![0; 16]).is_ok());
        assert_eq!(
            Frame::new(Size::new(2, 2), vec![0; 15]),
            Err(SurfaceError::InvalidFrame)
        );
        assert_eq!(
            Frame::new(Size::new(0, 2), vec![]),
            Err(SurfaceError::InvalidFrame)
        );
        let s = Frame::solid(Size::new(3, 1), [1, 2, 3, 4]).unwrap();
        assert_eq!(s.rgba, vec![1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn cuadro_del_tamano_del_monitor() {
        let o = output(4, 2, 240);
        let ok = Frame::solid(Size::new(8, 4), [0; 4]).unwrap();
        let mal = Frame::solid(Size::new(4, 2), [0; 4]).unwrap();
        assert_eq!(check_frame(&o, &ok), Ok(()));
        assert_eq!(check_frame(&o, &mal), Err(SurfaceError::SizeMismatch));
    }

    #[test]
    fn eventos_en_json() {
        let e = SurfaceEvent::OutputRemoved {
            name: "HDMI-A-1".into(),
        };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"event":"output_removed","name":"HDMI-A-1"}"#
        );
    }
}
