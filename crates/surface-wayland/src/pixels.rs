//! Cuentas puras de esta superficie (se prueban en cualquier sistema).

use miraishin_core::surface::{Scale, Size};

/// Pasa RGBA (orden de bytes R, G, B, A) a `Xrgb8888` de `wl_shm`, que en memoria es
/// B, G, R, X (entero de 32 bits en little-endian). El fondo es opaco: el alfa se descarta.
pub fn rgba_to_xrgb(rgba: &[u8], out: &mut [u8]) {
    for (src, dst) in rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(out.as_chunks_mut::<4>().0)
    {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 0xff;
    }
}

/// Escala real de un monitor a partir de su modo actual (píxeles) y su tamaño lógico.
/// Así se conoce la escala fraccionaria (125 %, 150 %) aunque `wl_output` solo dé enteros.
/// `rotated` = el monitor está girado 90° o 270° (el modo viene con ancho y alto cambiados).
pub fn scale_from_mode(mode: Size, logical: Size, rotated: bool, integer: u32) -> Scale {
    let mode_w = if rotated { mode.height } else { mode.width };
    if mode.is_empty() || logical.is_empty() {
        return Scale::from_integer(integer);
    }
    let s = (u64::from(mode_w) * 120 + u64::from(logical.width) / 2) / u64::from(logical.width);
    Scale(s.clamp(1, u64::from(u32::MAX)) as u32)
}

/// Escala entera para `set_buffer_scale` cuando el compositor no tiene `wp_viewporter`:
/// solo vale si el cuadro es un múltiplo exacto del tamaño lógico; si no, 1.
pub fn integer_buffer_scale(frame: Size, logical: Size) -> i32 {
    if logical.is_empty() || !frame.width.is_multiple_of(logical.width) {
        return 1;
    }
    let s = frame.width / logical.width;
    if s >= 1 && frame.height == logical.height * s {
        s as i32
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_a_xrgb() {
        let rgba = [0x11, 0x22, 0x33, 0x00, 0xaa, 0xbb, 0xcc, 0x80];
        let mut out = [0u8; 8];
        rgba_to_xrgb(&rgba, &mut out);
        assert_eq!(out, [0x33, 0x22, 0x11, 0xff, 0xcc, 0xbb, 0xaa, 0xff]);
        assert_eq!(
            u32::from_le_bytes(out[..4].try_into().unwrap()),
            0xff11_2233
        );
    }

    #[test]
    fn escala_desde_el_modo() {
        let s = |mw, mh, lw, lh| scale_from_mode(Size::new(mw, mh), Size::new(lw, lh), false, 1);
        assert_eq!(s(1920, 1080, 1920, 1080), Scale::ONE);
        assert_eq!(s(1920, 1080, 1536, 864), Scale(150)); // 125 %
        assert_eq!(s(2560, 1440, 1280, 720), Scale(240)); // 200 %
        assert_eq!(s(3840, 2160, 2560, 1440), Scale(180)); // 150 %
                                                           // Girado: el modo trae 1080×1920 y lo lógico es 1080×1920 también.
        let r = scale_from_mode(Size::new(1920, 1080), Size::new(1080, 1920), true, 1);
        assert_eq!(r, Scale::ONE);
        // Sin datos: se usa la escala entera.
        let e = scale_from_mode(Size::new(0, 0), Size::new(1280, 720), false, 2);
        assert_eq!(e, Scale(240));
    }

    #[test]
    fn escala_entera_del_buffer() {
        assert_eq!(
            integer_buffer_scale(Size::new(1920, 1080), Size::new(1920, 1080)),
            1
        );
        assert_eq!(
            integer_buffer_scale(Size::new(2560, 1440), Size::new(1280, 720)),
            2
        );
        // 125 % no es entero: 1 (se verá un poco más grande, pero sin error de protocolo).
        assert_eq!(
            integer_buffer_scale(Size::new(1920, 1080), Size::new(1536, 864)),
            1
        );
        assert_eq!(
            integer_buffer_scale(Size::new(2560, 1000), Size::new(1280, 720)),
            1
        );
        assert_eq!(integer_buffer_scale(Size::new(10, 10), Size::new(0, 0)), 1);
    }
}
