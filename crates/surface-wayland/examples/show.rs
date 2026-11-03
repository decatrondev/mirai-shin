//! Prueba manual de la superficie (sin pantalla: Sway con `WLR_BACKENDS=headless` + grim).
//! Pone un color distinto en cada monitor, y lo vuelve a poner cuando uno aparece o cambia.
//!
//! Con una imagen, la pone en todos los monitores (rellenar) en vez de los colores.
//!
//! Uso: cargo run -p miraishin-surface-wayland --example show -- [segundos] [imagen]

#[cfg(target_os = "linux")]
fn main() {
    use miraishin_core::surface::{Frame, OutputInfo, Surface, SurfaceEvent};
    use miraishin_surface_wayland::WaylandSurface;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    const COLORS: [[u8; 4]; 4] = [
        [0xd0, 0x30, 0x30, 0xff],
        [0x30, 0xa0, 0x40, 0xff],
        [0x30, 0x50, 0xd0, 0xff],
        [0xe0, 0xc0, 0x20, 0xff],
    ];
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let image = std::env::args().nth(2).map(std::path::PathBuf::from);
    let (tx, rx) = mpsc::channel();
    let mut surface = match WaylandSurface::connect(Box::new(move |e| {
        let _ = tx.send(e);
    })) {
        Ok(s) => s,
        Err(e) => {
            println!("no conecta: {e}");
            std::process::exit(2);
        }
    };

    let paint = |surface: &mut WaylandSurface, o: &OutputInfo, n: usize| {
        let px = o.pixel_size();
        let frame = match &image {
            Some(path) => {
                let t = Instant::now();
                let opts = miraishin_render_image::Options::default();
                match miraishin_render_image::render_file(path, px, &opts) {
                    Ok(f) => {
                        println!("imagen lista en {} ms", t.elapsed().as_millis());
                        f
                    }
                    Err(e) => {
                        println!("imagen: {e}");
                        return;
                    }
                }
            }
            None => Frame::solid(px, COLORS[n % COLORS.len()]).expect("cuadro"),
        };
        let r = surface.show(&o.name, frame);
        println!(
            "show {} ({}×{} lógicos, {} %, {}×{} px) color #{}: {:?}",
            o.name,
            o.logical.width,
            o.logical.height,
            o.scale.percent(),
            px.width,
            px.height,
            n % COLORS.len(),
            r
        );
    };

    let mut count = 0;
    for o in surface.outputs() {
        paint(&mut surface, &o, count);
        count += 1;
    }
    let end = Instant::now() + Duration::from_secs(seconds);
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(SurfaceEvent::OutputAdded(o)) => {
                println!("evento: monitor nuevo {}", o.name);
                paint(&mut surface, &o, count);
                count += 1;
            }
            Ok(SurfaceEvent::OutputChanged(o)) => {
                println!("evento: cambió {}", o.name);
                paint(&mut surface, &o, count);
                count += 1;
            }
            Ok(SurfaceEvent::OutputRemoved { name }) => {
                println!("evento: se fue {name}");
                println!(
                    "show en el que se fue: {:?}",
                    surface.show(
                        &name,
                        Frame::solid(miraishin_core::surface::Size::new(1, 1), [0; 4]).unwrap()
                    )
                );
            }
            Ok(SurfaceEvent::Covered {
                output,
                fullscreen,
                maximized,
            }) => println!(
                "evento: tapado {output} pantalla_completa={fullscreen} maximizada={maximized}"
            ),
            Err(_) => break,
        }
    }
    println!("fin");
}

#[cfg(not(target_os = "linux"))]
fn main() {}
