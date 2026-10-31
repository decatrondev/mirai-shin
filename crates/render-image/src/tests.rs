use super::*;
use image::codecs::{jpeg::JpegEncoder, png::PngEncoder, webp::WebPEncoder};
use image::{ExtendedColorType, ImageEncoder, Rgba};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Imagen de prueba: mitad izquierda roja, mitad derecha azul.
fn halves(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, _| Rgba(if x < w / 2 { RED } else { BLUE }))
}

fn png(img: &RgbaImage, exif: Option<Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = PngEncoder::new(&mut out);
    if let Some(e) = exif {
        enc.set_exif_metadata(e).unwrap();
    }
    enc.write_image(img, img.width(), img.height(), ExtendedColorType::Rgba8)
        .unwrap();
    out
}

/// EXIF mínimo (TIFF big-endian) con solo la etiqueta de orientación.
fn exif_orientation(value: u16) -> Vec<u8> {
    let mut e = b"MM\0\x2a\0\0\0\x08".to_vec();
    e.extend_from_slice(&1u16.to_be_bytes()); // una entrada
    e.extend_from_slice(&0x0112u16.to_be_bytes()); // Orientation
    e.extend_from_slice(&3u16.to_be_bytes()); // SHORT
    e.extend_from_slice(&1u32.to_be_bytes());
    e.extend_from_slice(&value.to_be_bytes());
    e.extend_from_slice(&[0, 0]);
    e.extend_from_slice(&0u32.to_be_bytes()); // sin más IFD
    e
}

fn px(f: &Frame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * f.size.width + x) * 4) as usize;
    f.rgba[i..i + 4].try_into().unwrap()
}

fn near(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 12)
}

fn opts(mode: FitMode) -> Options {
    Options {
        mode,
        ..Options::default()
    }
}

#[test]
fn png_rellenar_escala_al_monitor() {
    let bytes = png(&halves(400, 200), None);
    let f = render_bytes(&bytes, Size::new(200, 100), &Options::default()).unwrap();
    assert_eq!(f.size, Size::new(200, 100));
    assert_eq!(f.rgba.len(), 200 * 100 * 4);
    assert!(near(px(&f, 10, 50), RED));
    assert!(near(px(&f, 190, 50), BLUE));
}

#[test]
fn jpg_y_webp() {
    let img = halves(64, 32);
    let rgb = DynamicImage::ImageRgba8(img.clone()).into_rgb8();
    let mut jpg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpg, 95)
        .write_image(&rgb, 64, 32, ExtendedColorType::Rgb8)
        .unwrap();
    let mut webp = Vec::new();
    WebPEncoder::new_lossless(&mut webp)
        .write_image(&img, 64, 32, ExtendedColorType::Rgba8)
        .unwrap();
    for bytes in [jpg, webp] {
        let f = render_bytes(&bytes, Size::new(64, 32), &Options::default()).unwrap();
        assert!(near(px(&f, 4, 16), RED));
        assert!(near(px(&f, 60, 16), BLUE));
    }
}

#[test]
fn respeta_la_orientacion_exif() {
    // Orientación 6 = girar 90° a la derecha: lo rojo (izquierda) queda arriba.
    let bytes = png(&halves(40, 20), Some(exif_orientation(6)));
    let f = render_bytes(&bytes, Size::new(20, 40), &opts(FitMode::Stretch)).unwrap();
    assert!(near(px(&f, 10, 2), RED));
    assert!(near(px(&f, 10, 38), BLUE));
}

#[test]
fn ajustar_deja_bordes_del_color_de_fondo() {
    let bytes = png(&halves(100, 100), None);
    let o = opts(FitMode::Fit);
    let f = render_bytes(&bytes, Size::new(300, 100), &o).unwrap();
    assert_eq!(px(&f, 5, 50), o.background);
    assert!(near(px(&f, 110, 50), RED));
    assert!(near(px(&f, 190, 50), BLUE));
    assert_eq!(px(&f, 295, 50), o.background);
}

#[test]
fn centrar_sin_escalar() {
    let bytes = png(&halves(10, 10), None);
    let o = opts(FitMode::Center);
    let f = render_bytes(&bytes, Size::new(30, 30), &o).unwrap();
    assert_eq!(px(&f, 0, 0), o.background);
    assert_eq!(px(&f, 10, 15), RED);
    assert_eq!(px(&f, 19, 15), BLUE);
}

#[test]
fn transparencia_sobre_el_color_de_fondo() {
    let img = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 0]));
    let o = Options::default();
    let f = render_bytes(&png(&img, None), Size::new(4, 4), &o).unwrap();
    assert_eq!(px(&f, 1, 1), o.background);
}

#[test]
fn errores_con_codigo() {
    let t = Size::new(10, 10);
    let o = Options::default();
    assert_eq!(
        render_bytes(b"esto no es una imagen", t, &o),
        Err(RenderError::UnsupportedFormat)
    );
    // Cabecera PNG válida y el resto roto.
    let mut broken = png(&halves(8, 8), None);
    broken.truncate(40);
    assert_eq!(render_bytes(&broken, t, &o), Err(RenderError::Corrupt));
    assert_eq!(
        render_bytes(&png(&halves(8, 8), None), Size::new(0, 5), &o),
        Err(RenderError::EmptyTarget)
    );
    assert_eq!(
        render_file(Path::new("/no/existe.png"), t, &o),
        Err(RenderError::Unreadable)
    );
}

#[test]
fn limite_de_pixeles_antes_de_decodificar() {
    let bytes = png(&halves(100, 100), None);
    let o = Options {
        max_pixels: 100 * 100 - 1,
        ..Options::default()
    };
    assert_eq!(
        render_bytes(&bytes, Size::new(10, 10), &o),
        Err(RenderError::TooLarge)
    );
    let o = Options {
        max_pixels: 100 * 100,
        ..Options::default()
    };
    assert!(render_bytes(&bytes, Size::new(10, 10), &o).is_ok());
}

#[test]
fn codigos() {
    assert_eq!(RenderError::TooLarge.as_str(), "image_too_large");
    assert_eq!(RenderError::Corrupt.to_string(), "image_corrupt");
}
