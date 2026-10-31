//! Hilo de Wayland y la manija (`WaylandSurface`) que cumple el contrato `Surface`.

use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use miraishin_core::surface::{
    check_frame, EventSink, Frame, OutputInfo, Size, Surface, SurfaceError, SurfaceEvent,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_registry,
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            channel::{channel, Event as ChannelEvent, Sender},
            EventLoop,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
            Connection, QueueHandle,
        },
        protocols::wp::viewporter::client::{wp_viewport, wp_viewporter},
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface as _,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};

use crate::pixels::{integer_buffer_scale, rgba_to_xrgb, scale_from_mode};

/// Por qué no se pudo conectar. Códigos estables para `status` (fase 1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// No hay sesión Wayland (X11, o sin escritorio).
    NoWayland,
    /// El compositor no tiene `wlr-layer-shell` (GNOME).
    NoLayerShell,
    /// Falta otro protocolo básico (`wl_compositor`, `wl_shm`) o el hilo no arrancó.
    Unavailable,
}

impl ConnectError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoWayland => "no_wayland",
            Self::NoLayerShell => "no_layer_shell",
            Self::Unavailable => "wayland_unavailable",
        }
    }
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for ConnectError {}

enum Command {
    Show(String, Frame),
    Clear(String),
    Shutdown,
}

struct Shared {
    outputs: Vec<OutputInfo>,
    alive: bool,
}

/// Manija del fondo por layer-shell. Se usa desde el hilo del daemon; el trabajo de Wayland
/// lo hace un hilo propio.
pub struct WaylandSurface {
    commands: Sender<Command>,
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
}

impl WaylandSurface {
    /// Se conecta al compositor, espera a conocer los monitores y arranca el hilo.
    /// `events` recibe los cambios de monitores que pasen después.
    pub fn connect(events: EventSink) -> Result<Self, ConnectError> {
        let shared = Arc::new(Mutex::new(Shared {
            outputs: Vec::new(),
            alive: true,
        }));
        let (tx, rx) = channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("surface-wayland".into())
            .spawn(move || run(rx, thread_shared, events, ready_tx))
            .map_err(|_| ConnectError::Unavailable)?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                commands: tx,
                shared,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(ConnectError::Unavailable),
        }
    }

    fn send(&self, cmd: Command) -> Result<(), SurfaceError> {
        self.commands
            .send(cmd)
            .map_err(|_| SurfaceError::Disconnected)
    }

    fn output(&self, name: &str) -> Result<OutputInfo, SurfaceError> {
        let shared = self.shared.lock().map_err(|_| SurfaceError::Disconnected)?;
        if !shared.alive {
            return Err(SurfaceError::Disconnected);
        }
        shared
            .outputs
            .iter()
            .find(|o| o.name == name)
            .cloned()
            .ok_or(SurfaceError::UnknownOutput)
    }
}

impl Surface for WaylandSurface {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.shared
            .lock()
            .map(|s| s.outputs.clone())
            .unwrap_or_default()
    }

    fn show(&mut self, output: &str, frame: Frame) -> Result<(), SurfaceError> {
        check_frame(&self.output(output)?, &frame)?;
        self.send(Command::Show(output.to_string(), frame))
    }

    fn clear(&mut self, output: &str) -> Result<(), SurfaceError> {
        self.output(output)?;
        self.send(Command::Clear(output.to_string()))
    }
}

impl Drop for WaylandSurface {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ---------------------------------------------------------------- hilo de Wayland

/// Objetos de Wayland sin eventos (viewporter).
struct NoEvents;

struct Background {
    layer: LayerSurface,
    viewport: Option<wp_viewport::WpViewport>,
    /// Tamaño lógico que dio el compositor (0×0 hasta el primer `configure`).
    configured: Size,
    /// Cuadro esperando el primer `configure`.
    pending: Option<Frame>,
    /// Memoria del cuadro que se ve. Se suelta al cambiar de cuadro o al quitar el fondo.
    shown: Option<Shown>,
}

/// El buffer va primero: se destruye antes que la memoria que usa.
struct Shown {
    _buffer: Buffer,
    _pool: SlotPool,
}

impl Drop for Background {
    fn drop(&mut self) {
        if let Some(v) = self.viewport.take() {
            v.destroy();
        }
    }
}

struct Output {
    wl: wl_output::WlOutput,
    info: OutputInfo,
    background: Option<Background>,
}

struct State {
    registry: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    viewporter: Option<wp_viewporter::WpViewporter>,
    qh: QueueHandle<State>,
    outputs: Vec<Output>,
    shared: Arc<Mutex<Shared>>,
    events: EventSink,
    namespace: String,
    /// Los avisos se mandan solo después de la lista inicial.
    ready: bool,
    exit: bool,
}

fn run(
    commands: smithay_client_toolkit::reexports::calloop::channel::Channel<Command>,
    shared: Arc<Mutex<Shared>>,
    events: EventSink,
    ready: mpsc::Sender<Result<(), ConnectError>>,
) {
    let fail = |e| {
        let _ = ready.send(Err(e));
    };
    let Ok(conn) = Connection::connect_to_env() else {
        return fail(ConnectError::NoWayland);
    };
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        return fail(ConnectError::NoWayland);
    };
    let qh = queue.handle();
    let Ok(compositor) = CompositorState::bind(&globals, &qh) else {
        return fail(ConnectError::Unavailable);
    };
    let Ok(layer_shell) = LayerShell::bind(&globals, &qh) else {
        return fail(ConnectError::NoLayerShell);
    };
    let Ok(shm) = Shm::bind(&globals, &qh) else {
        return fail(ConnectError::Unavailable);
    };
    let viewporter = globals
        .bind::<wp_viewporter::WpViewporter, _, _>(&qh, 1..=1, NoEvents)
        .ok();

    let mut state = State {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        shm,
        viewporter,
        qh: qh.clone(),
        outputs: Vec::new(),
        shared,
        events,
        namespace: format!("{}-wallpaper", miraishin_brand::get().slug),
        ready: false,
        exit: false,
    };

    // Dos idas y vueltas: la primera trae los monitores, la segunda sus datos completos.
    for _ in 0..2 {
        if queue.roundtrip(&mut state).is_err() {
            return fail(ConnectError::Unavailable);
        }
    }

    let Ok(mut event_loop) = EventLoop::<State>::try_new() else {
        return fail(ConnectError::Unavailable);
    };
    let handle = event_loop.handle();
    if WaylandSource::new(conn, queue)
        .insert(handle.clone())
        .is_err()
    {
        return fail(ConnectError::Unavailable);
    }
    let inserted = handle.insert_source(commands, |event, _, state: &mut State| match event {
        ChannelEvent::Msg(Command::Show(name, frame)) => state.show(&name, frame),
        ChannelEvent::Msg(Command::Clear(name)) => state.clear(&name),
        ChannelEvent::Msg(Command::Shutdown) | ChannelEvent::Closed => state.exit = true,
    });
    if inserted.is_err() {
        return fail(ConnectError::Unavailable);
    }

    state.ready = true;
    state.publish();
    let _ = ready.send(Ok(()));

    while !state.exit {
        if event_loop.dispatch(None, &mut state).is_err() {
            break;
        }
    }
    let shared = state.shared.clone();
    drop(state);
    if let Ok(mut s) = shared.lock() {
        s.alive = false;
    };
}

impl State {
    fn publish(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.outputs = self.outputs.iter().map(|o| o.info.clone()).collect();
        }
    }

    fn emit(&mut self, event: SurfaceEvent) {
        if self.ready {
            (self.events)(event);
        }
    }

    fn info_of(&self, wl: &wl_output::WlOutput) -> Option<OutputInfo> {
        let i = self.output_state.info(wl)?;
        let mode = i
            .modes
            .iter()
            .find(|m| m.current)
            .map(|m| Size::new(m.dimensions.0.max(0) as u32, m.dimensions.1.max(0) as u32))
            .unwrap_or(Size::new(0, 0));
        let integer = i.scale_factor.max(1) as u32;
        let logical = match i.logical_size {
            Some((w, h)) if w > 0 && h > 0 => Size::new(w as u32, h as u32),
            _ => Size::new(mode.width / integer, mode.height / integer),
        };
        let rotated = matches!(
            i.transform,
            wl_output::Transform::_90
                | wl_output::Transform::_270
                | wl_output::Transform::Flipped90
                | wl_output::Transform::Flipped270
        );
        Some(OutputInfo {
            name: i
                .name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("output-{}", i.id)),
            description: i.description.clone().unwrap_or_default(),
            logical,
            scale: scale_from_mode(mode, logical, rotated, integer),
        })
    }

    fn show(&mut self, name: &str, frame: Frame) {
        let Some(i) = self.outputs.iter().position(|o| o.info.name == name) else {
            return;
        };
        if self.outputs[i].background.is_none() {
            let bg = self.create_background(&self.outputs[i].wl.clone());
            self.outputs[i].background = Some(bg);
        }
        let viewporter = self.viewporter.is_some();
        let shm = &self.shm;
        if let Some(bg) = self.outputs[i].background.as_mut() {
            if bg.configured.is_empty() {
                bg.pending = Some(frame);
            } else {
                attach(bg, &frame, shm, viewporter);
            }
        }
    }

    fn clear(&mut self, name: &str) {
        if let Some(o) = self.outputs.iter_mut().find(|o| o.info.name == name) {
            o.background = None;
        }
    }

    fn create_background(&self, wl: &wl_output::WlOutput) -> Background {
        let qh = self.qh.clone();
        let surface = self.compositor.create_surface(&qh);
        let viewport = self
            .viewporter
            .as_ref()
            .map(|v| v.get_viewport(&surface, &qh, NoEvents));
        let layer = self.layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Background,
            Some(self.namespace.clone()),
            Some(wl),
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
        Background {
            layer,
            viewport,
            configured: Size::new(0, 0),
            pending: None,
            shown: None,
        }
    }
}

/// Copia el cuadro a memoria compartida y lo pone en la superficie. La memoria del cuadro
/// anterior se suelta después de poner el nuevo.
fn attach(bg: &mut Background, frame: &Frame, shm: &Shm, viewporter: bool) {
    let (w, h) = (frame.size.width as i32, frame.size.height as i32);
    let Ok(mut pool) = SlotPool::new((w * h * 4) as usize, shm) else {
        return;
    };
    let Ok((buffer, canvas)) = pool.create_buffer(w, h, w * 4, wl_shm::Format::Xrgb8888) else {
        return;
    };
    rgba_to_xrgb(&frame.rgba, canvas);
    let surface = bg.layer.wl_surface();
    match (&bg.viewport, viewporter) {
        (Some(vp), true) => {
            surface.set_buffer_scale(1);
            vp.set_destination(bg.configured.width as i32, bg.configured.height as i32);
        }
        _ => surface.set_buffer_scale(integer_buffer_scale(frame.size, bg.configured)),
    }
    surface.damage_buffer(0, 0, w, h);
    if buffer.attach_to(surface).is_err() {
        return;
    }
    bg.layer.commit();
    // El cuadro anterior ya no está puesto: se suelta su memoria.
    bg.shown = Some(Shown {
        _buffer: buffer,
        _pool: pool,
    });
}

// ---------------------------------------------------------------- eventos de Wayland

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: wl_output::WlOutput) {
        let Some(info) = self.info_of(&wl) else {
            return;
        };
        self.outputs.push(Output {
            wl,
            info: info.clone(),
            background: None,
        });
        self.publish();
        self.emit(SurfaceEvent::OutputAdded(info));
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: wl_output::WlOutput) {
        let Some(info) = self.info_of(&wl) else {
            return;
        };
        let Some(o) = self.outputs.iter_mut().find(|o| o.wl == wl) else {
            // Llegó sin datos completos la primera vez: se cuenta como nuevo.
            return self.new_output_from_update(wl, info);
        };
        if o.info == info {
            return;
        }
        o.info = info.clone();
        self.publish();
        self.emit(SurfaceEvent::OutputChanged(info));
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: wl_output::WlOutput) {
        let Some(i) = self.outputs.iter().position(|o| o.wl == wl) else {
            return;
        };
        let gone = self.outputs.remove(i);
        self.publish();
        self.emit(SurfaceEvent::OutputRemoved {
            name: gone.info.name,
        });
    }
}

impl State {
    fn new_output_from_update(&mut self, wl: wl_output::WlOutput, info: OutputInfo) {
        self.outputs.push(Output {
            wl,
            info: info.clone(),
            background: None,
        });
        self.publish();
        self.emit(SurfaceEvent::OutputAdded(info));
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        // El compositor cerró la superficie (por ejemplo, el monitor se apagó): se olvida.
        for o in &mut self.outputs {
            if o.background.as_ref().is_some_and(|b| &b.layer == layer) {
                o.background = None;
            }
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
        let viewporter = self.viewporter.is_some();
        let shm = &self.shm;
        let Some(bg) = self
            .outputs
            .iter_mut()
            .filter_map(|o| o.background.as_mut())
            .find(|b| &b.layer == layer)
        else {
            return;
        };
        let size = Size::new(configure.new_size.0, configure.new_size.1);
        if size.is_empty() {
            return;
        }
        let changed = bg.configured != size;
        bg.configured = size;
        if let Some(frame) = bg.pending.take() {
            attach(bg, &frame, shm, viewporter);
        } else if changed {
            if let (Some(vp), true) = (&bg.viewport, viewporter) {
                // Mismo cuadro, otro tamaño lógico: el compositor lo escala hasta que el daemon
                // mande uno nuevo (recibe `OutputChanged`).
                vp.set_destination(size.width as i32, size.height as i32);
            }
            bg.layer.commit();
        }
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
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

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl Dispatch2<wp_viewporter::WpViewporter, State> for NoEvents {
    fn event(
        &self,
        _: &mut State,
        _: &wp_viewporter::WpViewporter,
        _: wp_viewporter::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch2<wp_viewport::WpViewport, State> for NoEvents {
    fn event(
        &self,
        _: &mut State,
        _: &wp_viewport::WpViewport,
        _: wp_viewport::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

delegate_registry!(State);
impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}
smithay_client_toolkit::delegate_dispatch2!(State);
