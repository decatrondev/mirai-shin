//! Imagen fija (fase 1.4): de un archivo a un cuadro listo para el monitor.
//!
//! - Formatos: png, jpg/jpeg y webp, reconocidos por el contenido (no por la extensión).
//! - Respeta la orientación EXIF (fotos del celular giradas).
//! - Encuadre con `miraishin_core::fit` y escala con filtro Catmull-Rom.
//! - Límite de tamaño antes de decodificar: un archivo enorme da error, no agota la memoria.
//! - Con varios monitores se decodifica una vez (`load_file`) y se encuadra para cada uno
//!   (`compose`); después la imagen original se suelta y solo quedan los cuadros.

use std::io::{BufRead, Cursor, Seek};
use std::path::Path;

use image::{imageops, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use miraishin_core::fit::{place, FitMode};
use miraishin_core::surface::{Frame, Size};

pub use image::RgbaImage;

/// Límite por defecto: 8192×8192 píxeles (unos 268 MB ya decodificada en RGBA).
/// Estimación no verificada de lo razonable para un fondo; se ajusta con datos reales.
pub const DEFAULT_MAX_PIXELS: u64 = 8192 * 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub mode: FitMode,
    /// Color de lo que no cubre la imagen (modos «ajustar» y «centrar»). RGBA.
    pub background: [u8; 4],
    pub max_pixels: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: FitMode::Fill,
            // Grafito del sistema de diseño (el mismo del respaldo del 1.7).
            background: [0x1c, 0x1d, 0x21, 0xff],
            max_pixels: DEFAULT_MAX_PIXELS,
        }
    }
}

/// Errores con código estable (la app los traduce).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    /// No se pudo abrir o leer el archivo.
    Unreadable,
    /// No es png, jpg ni webp.
    UnsupportedFormat,
    /// Es uno de esos formatos pero está dañado.
    Corrupt,
    /// La imagen pasa del límite de píxeles.
    TooLarge,
    /// El monitor mide 0 (no debería pasar).
    EmptyTarget,
}

impl RenderError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreadable => "image_unreadable",
            Self::UnsupportedFormat => "image_unsupported_format",
            Self::Corrupt => "image_corrupt",
            Self::TooLarge => "image_too_large",
            Self::EmptyTarget => "empty_target",
        }
    }
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for RenderError {}

const FORMATS: [ImageFormat; 3] = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP];

/// Decodifica un archivo (con su orientación EXIF y el límite de píxeles).
pub fn load_file(path: &Path, max_pixels: u64) -> Result<RgbaImage, RenderError> {
    let reader = ImageReader::open(path).map_err(|_| RenderError::Unreadable)?;
    decode(reader, max_pixels)
}

/// Igual que `load_file`, desde bytes en memoria.
pub fn load_bytes(bytes: &[u8], max_pixels: u64) -> Result<RgbaImage, RenderError> {
    decode(ImageReader::new(Cursor::new(bytes)), max_pixels)
}

/// Abre un archivo y lo deja listo para un monitor de `target` píxeles.
pub fn render_file(path: &Path, target: Size, opts: &Options) -> Result<Frame, RenderError> {
    compose(&load_file(path, opts.max_pixels)?, target, opts)
}

/// Igual que `render_file`, desde bytes en memoria (sirve para pruebas).
pub fn render_bytes(bytes: &[u8], target: Size, opts: &Options) -> Result<Frame, RenderError> {
    compose(&load_bytes(bytes, opts.max_pixels)?, target, opts)
}

fn decode<R: BufRead + Seek>(
    reader: ImageReader<R>,
    max_pixels: u64,
) -> Result<RgbaImage, RenderError> {
    let reader = reader
        .with_guessed_format()
        .map_err(|_| RenderError::Unreadable)?;
    match reader.format() {
        Some(f) if FORMATS.contains(&f) => {}
        _ => return Err(RenderError::UnsupportedFormat),
    }
    let mut decoder = reader.into_decoder().map_err(|_| RenderError::Corrupt)?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return Err(RenderError::Corrupt);
    }
    if u64::from(w) * u64::from(h) > max_pixels {
        return Err(RenderError::TooLarge);
    }
    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| RenderError::Corrupt)?;
    if let Some(o) = orientation {
        image.apply_orientation(o);
    }
    Ok(image.into_rgba8())
}

/// Encuadra y escala una imagen ya decodificada para un monitor de `target` píxeles.
pub fn compose(image: &RgbaImage, target: Size, opts: &Options) -> Result<Frame, RenderError> {
    let size = Size::new(image.width(), image.height());
    let p = place(size, target, opts.mode).ok_or(RenderError::EmptyTarget)?;

    let src = imageops::crop_imm(image, p.src.x, p.src.y, p.src.width, p.src.height);
    let scaled = if (p.src.width, p.src.height) == (p.dst.width, p.dst.height) {
        src.to_image()
    } else {
        imageops::resize(
            &*src,
            p.dst.width,
            p.dst.height,
            imageops::FilterType::CatmullRom,
        )
    };

    let mut canvas = if p.needs_background(target) {
        RgbaImage::from_pixel(target.width, target.height, image::Rgba(opts.background))
    } else {
        RgbaImage::new(target.width, target.height)
    };
    imageops::replace(&mut canvas, &scaled, i64::from(p.dst.x), i64::from(p.dst.y));
    // El fondo es opaco: lo transparente de la imagen se ve sobre el color de fondo.
    if scaled.pixels().any(|px| px[3] != 0xff) {
        flatten(&mut canvas, opts.background);
    }
    Frame::new(target, canvas.into_raw()).map_err(|_| RenderError::EmptyTarget)
}

/// Mezcla los píxeles con transparencia sobre el color de fondo.
fn flatten(canvas: &mut RgbaImage, bg: [u8; 4]) {
    for px in canvas.pixels_mut() {
        let a = u32::from(px[3]);
        if a == 0xff {
            continue;
        }
        for c in 0..3 {
            px[c] = ((u32::from(px[c]) * a + u32::from(bg[c]) * (255 - a) + 127) / 255) as u8;
        }
        px[3] = 0xff;
    }
}

#[cfg(test)]
mod tests;
