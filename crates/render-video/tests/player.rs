//! Abre archivos reales con libmpv (sin dibujar). Necesita libmpv instalada: en el CI de
//! Linux se instala; si falta, la prueba falla en vez de saltarse en silencio. En Windows el
//! video llega al final de la fase 2: ahí todavía no corre.
#![cfg(target_os = "linux")]

use std::path::Path;
use std::time::Duration;

use miraishin_core::fit::FitMode;
use miraishin_render_video::{Player, VideoError};

fn open(path: &Path) -> Result<(), VideoError> {
    Player::check(path, Duration::from_secs(10))
}

/// Un video de 1 s generado a mano: un solo cuadro en Y4M (sin códec, mpv lo lee siempre).
fn y4m(dir: &Path) -> std::path::PathBuf {
    let (w, h) = (16, 8);
    let mut data = format!("YUV4MPEG2 W{w} H{h} F1:1 Ip A1:1 C420jpeg\n").into_bytes();
    data.extend_from_slice(b"FRAME\n");
    data.extend(std::iter::repeat_n(128u8, w * h * 3 / 2));
    let p = dir.join("v.y4m");
    std::fs::write(&p, data).unwrap();
    p
}

#[test]
fn abre_un_video_y_rechaza_lo_que_no_sirve() {
    let t = tempfile::tempdir().unwrap();
    let bueno = y4m(t.path());
    if let Err(e) = open(&bueno) {
        panic!("libmpv debe estar instalada y abrir un Y4M: {e}");
    }
    // Sin contexto de dibujo no hay cuadros que pedir.
    let p = Player::new(FitMode::Fit, Box::new(|| {})).unwrap();
    assert!(!p.needs_frame());
    assert_eq!(p.drain_events(), None);
    drop(p);

    let roto = t.path().join("roto.mp4");
    std::fs::write(&roto, b"esto no es un video").unwrap();
    assert!(matches!(
        open(&roto).err(),
        Some(VideoError::Unplayable | VideoError::NoDecoder)
    ));
    assert!(open(&t.path().join("no-existe.mp4")).is_err());
}
