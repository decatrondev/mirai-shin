//! Prueba de la fase 3.0 (no se distribuye).
//!
//! `watch`: imprime cada ventana (título, app_id, si está en pantalla completa o maximizada y en
//! qué monitores) con `wlr-foreign-toplevel-management`, y el estado de cada monitor (on/off) con
//! `wlr-output-power-management`.
//! `window`: abre una ventana normal (app_id `power-probe`) para ponerla en pantalla completa con
//! `swaymsg`.

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("watch") => linux::watch(),
        Some("window") => linux::window(),
        _ => eprintln!("uso: miraishin-power-probe watch|window"),
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use smithay_client_toolkit::reexports::client::{
        self as wc,
        globals::{registry_queue_init, GlobalListContents},
        protocol::{
            wl_buffer, wl_compositor, wl_output, wl_registry, wl_shm, wl_shm_pool, wl_surface,
        },
        Connection, Dispatch, Proxy, QueueHandle,
    };
    use smithay_client_toolkit::reexports::protocols::xdg::shell::client::{
        xdg_surface, xdg_toplevel, xdg_wm_base,
    };
    use smithay_client_toolkit::reexports::protocols_wlr::{
        foreign_toplevel::v1::client::{
            zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
        },
        output_power_management::v1::client::{
            zwlr_output_power_manager_v1 as power_manager, zwlr_output_power_v1 as power,
        },
    };
    use std::collections::HashMap;
    use std::time::Instant;

    fn t(start: Instant) -> String {
        format!("{:7.3}s", start.elapsed().as_secs_f64())
    }

    #[derive(Default)]
    struct Toplevel {
        title: String,
        app_id: String,
        states: Vec<u32>,
        outputs: Vec<wl_output::WlOutput>,
    }

    struct Watch {
        start: Instant,
        output_names: HashMap<wc::backend::ObjectId, String>,
        toplevels: HashMap<wc::backend::ObjectId, Toplevel>,
    }

    impl Watch {
        fn name(&self, o: &wl_output::WlOutput) -> String {
            self.output_names
                .get(&o.id())
                .cloned()
                .unwrap_or_else(|| "?".into())
        }
    }

    pub fn watch() {
        let conn = Connection::connect_to_env().expect("no hay WAYLAND_DISPLAY");
        let (globals, mut queue) = registry_queue_init::<Watch>(&conn).expect("registro");
        let qh = queue.handle();
        let mut st = Watch {
            start: Instant::now(),
            output_names: HashMap::new(),
            toplevels: HashMap::new(),
        };
        let mut outputs = Vec::new();
        for g in globals.contents().clone_list() {
            if g.interface == "wl_output" {
                outputs.push(globals.registry().bind::<wl_output::WlOutput, _, _>(
                    g.name,
                    g.version.min(4),
                    &qh,
                    (),
                ));
            }
        }
        let list = globals.contents().clone_list();
        let ver = |i: &str| list.iter().find(|g| g.interface == i).map(|g| g.version);
        println!("protocolos: foreign-toplevel v{:?}, output-power v{:?}, ext-idle-notify v{:?}, ext-foreign-toplevel-list v{:?}",
            ver("zwlr_foreign_toplevel_manager_v1"), ver("zwlr_output_power_manager_v1"),
            ver("ext_idle_notifier_v1"), ver("ext_foreign_toplevel_list_v1"));
        queue.roundtrip(&mut st).unwrap();
        match globals.bind::<manager::ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ()) {
            Ok(_) => {}
            Err(e) => println!("sin wlr-foreign-toplevel: {e}"),
        }
        match globals.bind::<power_manager::ZwlrOutputPowerManagerV1, _, _>(&qh, 1..=1, ()) {
            Ok(pm) => {
                for o in &outputs {
                    pm.get_output_power(o, &qh, st.name(o));
                }
            }
            Err(e) => println!("sin wlr-output-power-management: {e}"),
        }
        loop {
            queue.blocking_dispatch(&mut st).unwrap();
        }
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watch {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<wl_output::WlOutput, ()> for Watch {
        fn event(
            st: &mut Self,
            o: &wl_output::WlOutput,
            e: wl_output::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_output::Event::Name { name } = e {
                st.output_names.insert(o.id(), name);
            }
        }
    }
    impl Dispatch<manager::ZwlrForeignToplevelManagerV1, ()> for Watch {
        fn event(
            _: &mut Self,
            _: &manager::ZwlrForeignToplevelManagerV1,
            _: manager::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
        wc::event_created_child!(Watch, manager::ZwlrForeignToplevelManagerV1, [
            manager::EVT_TOPLEVEL_OPCODE => (handle::ZwlrForeignToplevelHandleV1, ()),
        ]);
    }
    impl Dispatch<handle::ZwlrForeignToplevelHandleV1, ()> for Watch {
        fn event(
            st: &mut Self,
            h: &handle::ZwlrForeignToplevelHandleV1,
            e: handle::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            let id = h.id();
            match e {
                handle::Event::Title { title } => st.toplevels.entry(id).or_default().title = title,
                handle::Event::AppId { app_id } => {
                    st.toplevels.entry(id).or_default().app_id = app_id
                }
                handle::Event::State { state } => {
                    st.toplevels.entry(id).or_default().states = state
                        .chunks_exact(4)
                        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                        .collect()
                }
                handle::Event::OutputEnter { output } => {
                    st.toplevels.entry(id).or_default().outputs.push(output)
                }
                handle::Event::OutputLeave { output } => st
                    .toplevels
                    .entry(id)
                    .or_default()
                    .outputs
                    .retain(|o| o != &output),
                handle::Event::Done => {
                    let tl = &st.toplevels[&id];
                    let has = |s: handle::State| tl.states.contains(&(s as u32));
                    let outs: Vec<String> = tl.outputs.iter().map(|o| st.name(o)).collect();
                    println!("{} ventana {:?} ({}) pantalla_completa={} maximizada={} minimizada={} activa={} monitores={:?}",
                        t(st.start), tl.title, tl.app_id, has(handle::State::Fullscreen), has(handle::State::Maximized),
                        has(handle::State::Minimized), has(handle::State::Activated), outs);
                }
                handle::Event::Closed => {
                    println!(
                        "{} ventana cerrada {:?}",
                        t(st.start),
                        st.toplevels.remove(&id).map(|t| t.title)
                    );
                }
                _ => {}
            }
        }
    }
    impl Dispatch<power_manager::ZwlrOutputPowerManagerV1, ()> for Watch {
        fn event(
            _: &mut Self,
            _: &power_manager::ZwlrOutputPowerManagerV1,
            _: power_manager::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<power::ZwlrOutputPowerV1, String> for Watch {
        fn event(
            st: &mut Self,
            _: &power::ZwlrOutputPowerV1,
            e: power::Event,
            name: &String,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match e {
                power::Event::Mode { mode } => {
                    println!("{} monitor {name}: {:?}", t(st.start), mode)
                }
                power::Event::Failed => {
                    println!("{} monitor {name}: output-power falló", t(st.start))
                }
                _ => {}
            }
        }
    }

    // ---- ventana de prueba ----
    struct Win {
        shm: wl_shm::WlShm,
        surface: wl_surface::WlSurface,
        size: (i32, i32),
        quit: bool,
    }

    pub fn window() {
        let conn = Connection::connect_to_env().expect("no hay WAYLAND_DISPLAY");
        let (globals, mut queue) = registry_queue_init::<Win>(&conn).expect("registro");
        let qh = queue.handle();
        let comp: wl_compositor::WlCompositor = globals.bind(&qh, 4..=4, ()).unwrap();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
        let wm: xdg_wm_base::XdgWmBase = globals.bind(&qh, 1..=2, ()).unwrap();
        let surface = comp.create_surface(&qh, ());
        let xs = wm.get_xdg_surface(&surface, &qh, ());
        let top = xs.get_toplevel(&qh, ());
        top.set_app_id("power-probe".into());
        top.set_title("power-probe".into());
        surface.commit();
        let mut st = Win {
            shm,
            surface,
            size: (300, 200),
            quit: false,
        };
        while !st.quit {
            queue.blocking_dispatch(&mut st).unwrap();
        }
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Win {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Win {
        fn event(
            _: &mut Self,
            wm: &xdg_wm_base::XdgWmBase,
            e: xdg_wm_base::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let xdg_wm_base::Event::Ping { serial } = e {
                wm.pong(serial);
            }
        }
    }
    impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Win {
        fn event(
            st: &mut Self,
            _: &xdg_toplevel::XdgToplevel,
            e: xdg_toplevel::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match e {
                xdg_toplevel::Event::Configure { width, height, .. } if width > 0 && height > 0 => {
                    st.size = (width, height)
                }
                xdg_toplevel::Event::Close => st.quit = true,
                _ => {}
            }
        }
    }
    impl Dispatch<xdg_surface::XdgSurface, ()> for Win {
        fn event(
            st: &mut Self,
            xs: &xdg_surface::XdgSurface,
            e: xdg_surface::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let xdg_surface::Event::Configure { serial } = e {
                xs.ack_configure(serial);
                let (w, h) = st.size;
                let path = format!("/dev/shm/power-probe-{}", std::process::id());
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&path)
                    .unwrap();
                let _ = std::fs::remove_file(&path);
                let px: Vec<u8> = [0x30u8, 0x80, 0xd0, 0xff].repeat((w * h) as usize);
                std::io::Write::write_all(&mut &file, &px).unwrap();
                let pool = st
                    .shm
                    .create_pool(std::os::fd::AsFd::as_fd(&file), w * h * 4, qh, ());
                let buf = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, qh, ());
                pool.destroy();
                st.surface.attach(Some(&buf), 0, 0);
                st.surface.damage_buffer(0, 0, w, h);
                st.surface.commit();
            }
        }
    }
    impl Dispatch<wl_buffer::WlBuffer, ()> for Win {
        fn event(
            _: &mut Self,
            b: &wl_buffer::WlBuffer,
            e: wl_buffer::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_buffer::Event::Release = e {
                b.destroy();
            }
        }
    }
    wc::delegate_noop!(Win: ignore wl_compositor::WlCompositor);
    wc::delegate_noop!(Win: ignore wl_shm::WlShm);
    wc::delegate_noop!(Win: ignore wl_shm_pool::WlShmPool);
    wc::delegate_noop!(Win: ignore wl_surface::WlSurface);
}
