//! Prueba mínima de la fase 1.1: pone un color sólido con franjas en la capa `background`
//! de `wlr-layer-shell`, en cada monitor, para ver dónde queda respecto de los íconos del
//! escritorio (sobre todo en KDE Plasma). Herramienta interna: no se distribuye.
//!
//! Uso: miraishin-layer-probe [--color RRGGBB] [--seconds N]

#[cfg(target_os = "linux")]
fn main() {
    probe::run();
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("Esta prueba es solo para Linux (Wayland).");
}

#[cfg(target_os = "linux")]
mod probe {
    use smithay_client_toolkit::{
        compositor::{CompositorHandler, CompositorState, Region},
        delegate_registry,
        output::{OutputHandler, OutputState},
        reexports::client::{
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
            Connection, QueueHandle,
        },
        registry::{ProvidesRegistryState, RegistryState},
        registry_handlers,
        shell::{
            wlr_layer::{
                Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
                LayerSurfaceConfigure,
            },
            WaylandSurface,
        },
        shm::{slot::SlotPool, Shm, ShmHandler},
    };
    use std::time::Duration;

    struct Surface {
        output: wl_output::WlOutput,
        layer: LayerSurface,
        size: (u32, u32),
        scale: i32,
        drawn: bool,
    }

    struct Probe {
        registry: RegistryState,
        outputs: OutputState,
        compositor: CompositorState,
        layer_shell: LayerShell,
        shm: Shm,
        pool: SlotPool,
        surfaces: Vec<Surface>,
        color: u32,
        closed: bool,
    }

    pub fn run() {
        let mut color = 0x2f6f73_u32;
        let mut seconds: Option<u64> = None;
        let args: Vec<String> = std::env::args().skip(1).collect();
        for pair in args.chunks(2) {
            match (pair[0].as_str(), pair.get(1)) {
                ("--color", Some(v)) => {
                    color = u32::from_str_radix(v.trim_start_matches('#'), 16).unwrap_or(color)
                }
                ("--seconds", Some(v)) => seconds = v.parse().ok(),
                _ => {}
            }
        }

        println!(
            "Escritorio (XDG_CURRENT_DESKTOP): {}",
            env_or("XDG_CURRENT_DESKTOP")
        );
        println!("Sesión (XDG_SESSION_TYPE): {}", env_or("XDG_SESSION_TYPE"));
        let conn = match Connection::connect_to_env() {
            Ok(c) => c,
            Err(e) => {
                println!("No hay sesión Wayland a la que conectarse ({e}). En X11 esta prueba no aplica.");
                std::process::exit(2);
            }
        };
        let (globals, mut queue) = registry_queue_init(&conn).expect("registro de Wayland");
        let qh = queue.handle();
        let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
        let layer_shell = match LayerShell::bind(&globals, &qh) {
            Ok(l) => l,
            Err(_) => {
                println!("RESULTADO: este compositor NO tiene wlr-layer-shell (pasa en GNOME). No se puede poner el fondo así.");
                std::process::exit(3);
            }
        };
        println!("wlr-layer-shell: sí");
        let shm = Shm::bind(&globals, &qh).expect("wl_shm");
        let pool = SlotPool::new(4096, &shm).expect("memoria compartida");
        let mut probe = Probe {
            registry: RegistryState::new(&globals),
            outputs: OutputState::new(&globals, &qh),
            compositor,
            layer_shell,
            shm,
            pool,
            surfaces: Vec::new(),
            color,
            closed: false,
        };

        if let Some(secs) = seconds {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(secs));
                std::process::exit(0);
            });
        }
        println!("Fondo de prueba puesto. Cierra con Ctrl+C.");
        while !probe.closed {
            queue
                .blocking_dispatch(&mut probe)
                .expect("eventos de Wayland");
        }
    }

    fn env_or(key: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| "(sin definir)".into())
    }

    impl Probe {
        fn add_output(&mut self, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
            let info = self.outputs.info(&output);
            if let Some(i) = &info {
                println!(
                    "Monitor: {} — {} — {:?} lógicos, escala {}",
                    i.name.clone().unwrap_or_default(),
                    i.description.clone().unwrap_or_default(),
                    i.logical_size,
                    i.scale_factor
                );
            }
            let surface = self.compositor.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(
                qh,
                surface,
                Layer::Background,
                Some("miraishin-probe"),
                Some(&output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_size(0, 0);
            // Región de entrada vacía: los clics pasan al escritorio.
            if let Ok(region) = Region::new(&self.compositor) {
                layer
                    .wl_surface()
                    .set_input_region(Some(region.wl_region()));
            }
            layer.commit();
            self.surfaces.push(Surface {
                output,
                layer,
                size: (0, 0),
                scale: info.map(|i| i.scale_factor).unwrap_or(1).max(1),
                drawn: false,
            });
        }

        fn draw(&mut self, index: usize) {
            let s = &mut self.surfaces[index];
            let (w, h) = (s.size.0 as i32 * s.scale, s.size.1 as i32 * s.scale);
            if w == 0 || h == 0 {
                return;
            }
            let (buffer, canvas) = self
                .pool
                .create_buffer(w, h, w * 4, wl_shm::Format::Xrgb8888)
                .expect("buffer");
            let base = self.color;
            let darker = darken(base);
            let stripe = (40 * s.scale) as usize;
            for (i, px) in canvas.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (x, y) = (i % w as usize, i / w as usize);
                let c = if ((x + y) / stripe).is_multiple_of(2) {
                    base
                } else {
                    darker
                };
                px.copy_from_slice(&c.to_le_bytes());
            }
            let surface = s.layer.wl_surface();
            surface.set_buffer_scale(s.scale);
            surface.damage_buffer(0, 0, w, h);
            buffer.attach_to(surface).expect("attach");
            s.layer.commit();
            if !s.drawn {
                println!("Dibujado: {}x{} píxeles (escala {}).", w, h, s.scale);
            }
            s.drawn = true;
        }
    }

    fn darken(c: u32) -> u32 {
        let ch = |shift: u32| (((c >> shift) & 0xff) * 3 / 4) << shift;
        ch(16) | ch(8) | ch(0)
    }

    impl CompositorHandler for Probe {
        fn scale_factor_changed(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            surface: &wl_surface::WlSurface,
            f: i32,
        ) {
            if let Some(i) = self
                .surfaces
                .iter()
                .position(|s| s.layer.wl_surface() == surface)
            {
                self.surfaces[i].scale = f.max(1);
                self.draw(i);
            }
        }
        fn transform_changed(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            _: &wl_surface::WlSurface,
            _: wl_output::Transform,
        ) {
        }
        fn frame(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            _: &wl_surface::WlSurface,
            _: u32,
        ) {
        }
        fn surface_enter(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            _: &wl_surface::WlSurface,
            _: &wl_output::WlOutput,
        ) {
        }
        fn surface_leave(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            _: &wl_surface::WlSurface,
            _: &wl_output::WlOutput,
        ) {
        }
    }

    impl OutputHandler for Probe {
        fn output_state(&mut self) -> &mut OutputState {
            &mut self.outputs
        }
        fn new_output(
            &mut self,
            _: &Connection,
            qh: &QueueHandle<Self>,
            output: wl_output::WlOutput,
        ) {
            self.add_output(qh, output);
        }
        fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        }
        fn output_destroyed(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            output: wl_output::WlOutput,
        ) {
            self.surfaces.retain(|s| s.output != output);
            println!("Monitor desconectado.");
        }
    }

    impl LayerShellHandler for Probe {
        fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
            self.surfaces.retain(|s| &s.layer != layer);
            if self.surfaces.is_empty() {
                self.closed = true;
            }
        }
        fn configure(
            &mut self,
            _: &Connection,
            _: &QueueHandle<Self>,
            layer: &LayerSurface,
            configure: LayerSurfaceConfigure,
            _: u32,
        ) {
            if let Some(i) = self.surfaces.iter().position(|s| &s.layer == layer) {
                self.surfaces[i].size = configure.new_size;
                self.draw(i);
            }
        }
    }

    impl ShmHandler for Probe {
        fn shm_state(&mut self) -> &mut Shm {
            &mut self.shm
        }
    }

    delegate_registry!(Probe);
    impl ProvidesRegistryState for Probe {
        fn registry(&mut self) -> &mut RegistryState {
            &mut self.registry
        }
        registry_handlers![OutputState];
    }
    smithay_client_toolkit::delegate_dispatch2!(Probe);
}
