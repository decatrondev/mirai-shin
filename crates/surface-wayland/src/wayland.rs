//! Hilo de Wayland y la manija (`WaylandSurface`) que cumple el contrato `Surface`.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use khronos_egl as egl_api;
use miraishin_core::fit::FitMode;
use miraishin_core::surface::{
    check_frame, EventSink, Frame, OutputInfo, Size, Surface, SurfaceError, SurfaceEvent,
};
use miraishin_render_video::Player;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    delegate_registry,
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            channel::{channel, Event as ChannelEvent, Sender},
            ping::{make_ping, Ping},
            EventLoop,
        },
        calloop_wayland_source::WaylandSource,
        client::{
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
            Connection, Proxy, QueueHandle,
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

use crate::egl::{self, Egl};
use crate::pixels::{integer_buffer_scale, rgba_to_xrgb, scale_from_mode};

/// Cuánto se espera a que mpv abra un video antes de responder al daemon.
const VIDEO_LOAD_TIMEOUT: Duration = Duration::from_secs(8);

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

type VideoReply = mpsc::Sender<Result<(), &'static str>>;

enum Command {
    Show(String, Frame),
    ShowVideo(String, PathBuf, FitMode, VideoReply),
    Clear(String),
    SetPaused(String, bool),
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
        Self::start(events, true)
    }

    fn start(events: EventSink, backgrounds: bool) -> Result<Self, ConnectError> {
        let shared = Arc::new(Mutex::new(Shared {
            outputs: Vec::new(),
            alive: true,
        }));
        let (tx, rx) = channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("surface-wayland".into())
            .spawn(move || run(rx, thread_shared, events, ready_tx, backgrounds))
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

/// Solo la lista de monitores y sus avisos, sin poner nada en pantalla. La usa KDE, donde
/// el fondo lo dibuja Plasma pero los monitores se conocen por Wayland igual.
pub struct OutputWatcher(WaylandSurface);

impl OutputWatcher {
    pub fn connect(events: EventSink) -> Result<Self, ConnectError> {
        WaylandSurface::start(events, false).map(Self)
    }

    pub fn outputs(&self) -> Vec<OutputInfo> {
        self.0.outputs()
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

    fn show_video(
        &mut self,
        output: &str,
        video: &Path,
        mode: FitMode,
    ) -> Result<(), SurfaceError> {
        self.output(output)?;
        let (tx, rx) = mpsc::channel();
        self.send(Command::ShowVideo(
            output.to_string(),
            video.to_path_buf(),
            mode,
            tx,
        ))?;
        match rx.recv_timeout(VIDEO_LOAD_TIMEOUT + Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(code)) => Err(SurfaceError::Video(code)),
            Err(_) => Err(SurfaceError::Video("video_unplayable")),
        }
    }

    fn clear(&mut self, output: &str) -> Result<(), SurfaceError> {
        self.output(output)?;
        self.send(Command::Clear(output.to_string()))
    }

    fn set_paused(&mut self, output: &str, paused: bool) -> Result<(), SurfaceError> {
        self.output(output)?;
        self.send(Command::SetPaused(output.to_string(), paused))
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
    /// Video en esta superficie (en vez de `shown`).
    video: Option<VideoTarget>,
}

/// Un video y su encuadre: los monitores con la misma clave comparten reproductor.
type VideoKey = (PathBuf, FitMode);

/// Superficie EGL de un monitor que muestra un video.
struct VideoTarget {
    key: VideoKey,
    /// La superficie EGL va antes que la ventana de Wayland en que se apoya.
    surface: egl_api::Surface,
    window: wayland_egl::WlEglSurface,
    /// Tamaño en píxeles de la ventana EGL.
    pixels: Size,
    /// Se pidió un aviso de cuadro y todavía no llegó: no se dibuja hasta entonces.
    frame_pending: bool,
    /// Hay un cuadro nuevo esperando ese aviso.
    redraw: bool,
    /// En pausa (fase 3): no se dibuja más, queda el último cuadro. Si el reproductor lo
    /// comparte otro monitor que no está en pausa, mpv sigue y solo este deja de dibujar.
    paused: bool,
    /// Ya se puso un cuadro de verdad del video (en pausa se espera a tenerlo antes de
    /// dejar de dibujar; el primer dibujo, al ponerlo, puede ser antes de que mpv tenga uno).
    drawn: bool,
    /// mpv avisó un cuadro nuevo que todavía no se puso.
    fresh: bool,
}

/// Un reproductor compartido y el contexto OpenGL en que dibuja.
struct Slot {
    player: Player,
    context: egl_api::Context,
    users: usize,
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
    /// `None` en `OutputWatcher`: no se crean superficies.
    layer_shell: Option<LayerShell>,
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
    /// Conexión de libwayland, para EGL.
    wl_display: *mut c_void,
    /// EGL se prepara al primer video: `None` = sin intentar, `Some(None)` = no hay.
    egl: Option<Option<Egl>>,
    players: HashMap<VideoKey, Slot>,
    /// libmpv avisa por aquí (desde sus hilos) que hay eventos o un cuadro nuevo.
    ping: Ping,
}

fn run(
    commands: smithay_client_toolkit::reexports::calloop::channel::Channel<Command>,
    shared: Arc<Mutex<Shared>>,
    events: EventSink,
    ready: mpsc::Sender<Result<(), ConnectError>>,
    backgrounds: bool,
) {
    let fail = |e| {
        let _ = ready.send(Err(e));
    };
    let Ok(conn) = Connection::connect_to_env() else {
        return fail(ConnectError::NoWayland);
    };
    let wl_display = conn.backend().display_ptr() as *mut c_void;
    let Ok((ping, ping_source)) = make_ping() else {
        return fail(ConnectError::Unavailable);
    };
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        return fail(ConnectError::NoWayland);
    };
    let qh = queue.handle();
    let Ok(compositor) = CompositorState::bind(&globals, &qh) else {
        return fail(ConnectError::Unavailable);
    };
    let layer_shell = if backgrounds {
        let Ok(l) = LayerShell::bind(&globals, &qh) else {
            return fail(ConnectError::NoLayerShell);
        };
        Some(l)
    } else {
        None
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
        wl_display,
        egl: None,
        players: HashMap::new(),
        ping,
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
        ChannelEvent::Msg(Command::ShowVideo(name, path, mode, reply)) => {
            let _ = reply.send(state.show_video(&name, path, mode));
        }
        ChannelEvent::Msg(Command::Clear(name)) => state.clear(&name),
        ChannelEvent::Msg(Command::SetPaused(name, paused)) => state.set_paused(&name, paused),
        ChannelEvent::Msg(Command::Shutdown) | ChannelEvent::Closed => state.exit = true,
    });
    if inserted.is_err() {
        return fail(ConnectError::Unavailable);
    }
    if handle
        .insert_source(ping_source, |_, _, state: &mut State| state.on_mpv())
        .is_err()
    {
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
    // Los videos se sueltan con su contexto OpenGL activo, antes que EGL.
    let names: Vec<String> = state.outputs.iter().map(|o| o.info.name.clone()).collect();
    for name in names {
        state.clear(&name);
    }
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
            let Some(bg) = self.create_background(&self.outputs[i].wl.clone()) else {
                return;
            };
            self.outputs[i].background = Some(bg);
        }
        let viewporter = self.viewporter.is_some();
        let shm = &self.shm;
        let mut old_video = None;
        if let Some(bg) = self.outputs[i].background.as_mut() {
            if bg.configured.is_empty() {
                bg.pending = Some(frame);
            } else {
                attach(bg, &frame, shm, viewporter);
            }
            // La imagen ya está puesta: recién ahora se suelta el video (sin hueco).
            old_video = bg.video.take();
        }
        if let Some(v) = old_video {
            self.release_video(v);
        }
    }

    fn clear(&mut self, name: &str) {
        let Some(o) = self.outputs.iter_mut().find(|o| o.info.name == name) else {
            return;
        };
        let video = o.background.as_mut().and_then(|b| b.video.take());
        o.background = None;
        if let Some(v) = video {
            self.release_video(v);
        }
    }

    fn create_background(&self, wl: &wl_output::WlOutput) -> Option<Background> {
        let layer_shell = self.layer_shell.as_ref()?;
        let qh = self.qh.clone();
        let surface = self.compositor.create_surface(&qh);
        let viewport = self
            .viewporter
            .as_ref()
            .map(|v| v.get_viewport(&surface, &qh, NoEvents));
        let layer = layer_shell.create_layer_surface(
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
        Some(Background {
            layer,
            viewport,
            configured: Size::new(0, 0),
            pending: None,
            shown: None,
            video: None,
        })
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

// ---------------------------------------------------------------- video (fase 2.1)

impl State {
    /// EGL, preparado al primer video.
    fn egl(&mut self) -> Option<&Egl> {
        if self.egl.is_none() {
            let e = Egl::new(self.wl_display);
            if e.is_none() {
                tracing::warn!("EGL is not available: videos cannot be drawn");
            }
            self.egl = Some(e);
        }
        self.egl.as_ref().and_then(|e| e.as_ref())
    }

    /// Pone un video en un monitor. Responde cuando mpv ya abrió el archivo (o falló).
    fn show_video(&mut self, name: &str, path: PathBuf, mode: FitMode) -> Result<(), &'static str> {
        let i = self
            .outputs
            .iter()
            .position(|o| o.info.name == name)
            .ok_or("unknown_output")?;
        if self.egl().is_none() {
            return Err("video_no_gpu");
        }
        tracing::debug!(output = name, "video: EGL ready");
        let key: VideoKey = (path, mode);
        if self.outputs[i]
            .background
            .as_ref()
            .and_then(|b| b.video.as_ref())
            .is_some_and(|v| v.key == key)
        {
            return Ok(());
        }
        if self.outputs[i].background.is_none() {
            let bg = self
                .create_background(&self.outputs[i].wl.clone())
                .ok_or("video_no_gpu")?;
            self.outputs[i].background = Some(bg);
        }
        let pixels = self.outputs[i].info.pixel_size();
        let surface_id = match &self.outputs[i].background {
            Some(bg) => bg.layer.wl_surface().id(),
            None => return Err("video_no_gpu"),
        };
        let window = wayland_egl::WlEglSurface::new(
            surface_id,
            pixels.width.max(1) as i32,
            pixels.height.max(1) as i32,
        )
        .map_err(|_| "video_no_gpu")?;
        tracing::debug!("video: wl_egl_window created");
        let egl = self
            .egl
            .as_ref()
            .and_then(|e| e.as_ref())
            .ok_or("video_no_gpu")?;
        let surface = egl.create_surface(window.ptr()).ok_or("video_no_gpu")?;
        tracing::debug!("video: EGL surface created");

        // Reproductor: el mismo para todos los monitores con este video y encuadre.
        if !self.players.contains_key(&key) {
            match self.new_player(&key, surface) {
                Ok(slot) => {
                    self.players.insert(key.clone(), slot);
                }
                Err(code) => {
                    if let Some(egl) = self.egl.as_ref().and_then(|e| e.as_ref()) {
                        egl.destroy_surface(surface);
                    }
                    return Err(code);
                }
            }
        }
        if let Some(slot) = self.players.get_mut(&key) {
            slot.users += 1;
        }
        let target = VideoTarget {
            key,
            surface,
            window,
            pixels,
            frame_pending: false,
            redraw: true,
            paused: false,
            drawn: false,
            fresh: false,
        };
        let old = self.outputs[i].background.as_mut().and_then(|bg| {
            bg.pending = None;
            bg.video.replace(target)
        });
        if let Some(old) = old {
            self.release_video(old);
        }
        self.render_output(i);
        Ok(())
    }

    fn new_player(&self, key: &VideoKey, surface: egl_api::Surface) -> Result<Slot, &'static str> {
        let egl = self
            .egl
            .as_ref()
            .and_then(|e| e.as_ref())
            .ok_or("video_no_gpu")?;
        let context = egl.create_context().ok_or("video_no_gpu")?;
        let fail = |code: &'static str| {
            egl.destroy_context(context);
            Err(code)
        };
        if !egl.make_current(Some(surface), context) {
            return fail("video_no_gpu");
        }
        tracing::debug!("video: context current");
        let ping = self.ping.clone();
        let mut player = match Player::new(key.1, Box::new(move || ping.ping())) {
            Ok(p) => p,
            Err(e) => return fail(e.as_str()),
        };
        tracing::debug!("video: mpv started");
        // SAFETY: el contexto de este reproductor está activo en este hilo, y lo estará en
        // cada dibujo (`render_output`) y al cerrarlo (`release_video`).
        if let Err(e) = unsafe { player.attach_gl(egl::get_proc_address) } {
            drop(player);
            return fail(e.as_str());
        }
        tracing::debug!("video: mpv render context ready");
        if let Err(e) = player.load(&key.0, VIDEO_LOAD_TIMEOUT) {
            drop(player);
            return fail(e.as_str());
        }
        Ok(Slot {
            player,
            context,
            users: 0,
        })
    }

    /// Quita un video de una superficie; el reproductor se cierra si nadie más lo usa.
    fn release_video(&mut self, target: VideoTarget) {
        let Some(egl) = self.egl.as_ref().and_then(|e| e.as_ref()) else {
            return;
        };
        let VideoTarget {
            key,
            surface,
            window,
            ..
        } = target;
        let last = match self.players.get_mut(&key) {
            Some(slot) => {
                slot.users = slot.users.saturating_sub(1);
                slot.users == 0
            }
            None => false,
        };
        if last {
            if let Some(slot) = self.players.remove(&key) {
                // El reproductor se cierra con su contexto activo.
                egl.make_current(Some(surface), slot.context);
                drop(slot.player);
                egl.destroy_context(slot.context);
            }
        }
        egl.destroy_surface(surface);
        drop(window);
    }

    /// Pausa o reanuda el video de un monitor. mpv se pausa solo si todos los monitores que
    /// lo comparten están en pausa.
    fn set_paused(&mut self, name: &str, paused: bool) {
        let Some(i) = self.outputs.iter().position(|o| o.info.name == name) else {
            return;
        };
        let Some(v) = self.outputs[i]
            .background
            .as_mut()
            .and_then(|b| b.video.as_mut())
        else {
            return;
        };
        let key = v.key.clone();
        v.paused = paused;
        if !paused {
            v.redraw = true;
        }
        let all_paused = self.outputs.iter().all(|o| {
            o.background
                .as_ref()
                .and_then(|b| b.video.as_ref())
                .is_none_or(|v| v.key != key || v.paused)
        });
        if let Some(slot) = self.players.get(&key) {
            if let Err(e) = slot.player.set_paused(all_paused) {
                tracing::warn!(code = e.as_str(), "video: could not change the pause");
            }
        }
        if !paused {
            self.render_output(i);
        }
    }

    /// libmpv avisó: eventos y cuadros nuevos.
    fn on_mpv(&mut self) {
        let keys: Vec<VideoKey> = self.players.keys().cloned().collect();
        for key in keys {
            let Some(slot) = self.players.get(&key) else {
                continue;
            };
            if let Some(code) = slot.player.drain_events() {
                tracing::warn!(code, "video stopped");
            }
            if !slot.player.needs_frame() {
                continue;
            }
            for i in 0..self.outputs.len() {
                let pending = match self.outputs[i]
                    .background
                    .as_mut()
                    .and_then(|b| b.video.as_mut())
                {
                    Some(v) if v.key == key => {
                        v.redraw = true;
                        v.fresh = true;
                        v.frame_pending
                    }
                    _ => continue,
                };
                if !pending {
                    self.render_output(i);
                }
            }
        }
    }

    /// Dibuja el cuadro actual del video de un monitor y pide el próximo aviso de cuadro.
    fn render_output(&mut self, i: usize) {
        let qh = self.qh.clone();
        let Some(egl) = self.egl.as_ref().and_then(|e| e.as_ref()) else {
            return;
        };
        let Some(bg) = self.outputs[i].background.as_mut() else {
            return;
        };
        // Antes del primer `configure` no se puede poner nada en la superficie.
        if bg.configured.is_empty() {
            return;
        }
        let Some(v) = bg.video.as_mut() else {
            return;
        };
        // En pausa queda el último cuadro puesto (si ya hay uno).
        if v.paused && v.drawn {
            return;
        }
        let Some(slot) = self.players.get(&v.key) else {
            return;
        };
        if !egl.make_current(Some(v.surface), slot.context) {
            return;
        }
        // SAFETY: el contexto del reproductor está activo con la superficie de este monitor.
        unsafe {
            slot.player
                .render(v.pixels.width as i32, v.pixels.height as i32)
        };
        let surface = bg.layer.wl_surface();
        surface.frame(&qh, FrameCallbackData(surface.clone()));
        if egl.swap(v.surface) {
            slot.player.report_swap();
            v.frame_pending = true;
            v.redraw = false;
            if v.fresh {
                v.drawn = true;
                v.fresh = false;
            }
            // El primer cuadro del video ya está puesto: se suelta la imagen anterior.
            bg.shown = None;
        }
    }

    /// Ajusta la ventana EGL de un monitor a su tamaño en píxeles y su tamaño lógico.
    fn resize_video(&mut self, i: usize) {
        let pixels = self.outputs[i].info.pixel_size();
        let viewporter = self.viewporter.is_some();
        let Some(bg) = self.outputs[i].background.as_mut() else {
            return;
        };
        let configured = bg.configured;
        let Some(v) = bg.video.as_mut() else {
            return;
        };
        if v.pixels != pixels {
            v.window
                .resize(pixels.width as i32, pixels.height as i32, 0, 0);
            v.pixels = pixels;
        }
        let surface = bg.layer.wl_surface();
        match (&bg.viewport, viewporter) {
            (Some(vp), true) => {
                surface.set_buffer_scale(1);
                vp.set_destination(configured.width as i32, configured.height as i32);
            }
            _ => surface.set_buffer_scale(integer_buffer_scale(pixels, configured)),
        }
        v.redraw = true;
        // Un aviso de cuadro pedido antes de este cambio podría no llegar: se dibuja ya.
        v.frame_pending = false;
        self.render_output(i);
    }
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
        let mut gone = self.outputs.remove(i);
        if let Some(v) = gone.background.as_mut().and_then(|b| b.video.take()) {
            self.release_video(v);
        }
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
        let mut videos = Vec::new();
        for o in &mut self.outputs {
            if o.background.as_ref().is_some_and(|b| &b.layer == layer) {
                if let Some(v) = o.background.as_mut().and_then(|b| b.video.take()) {
                    videos.push(v);
                }
                o.background = None;
            }
        }
        for v in videos {
            self.release_video(v);
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
        let Some(i) = self
            .outputs
            .iter()
            .position(|o| o.background.as_ref().is_some_and(|b| &b.layer == layer))
        else {
            return;
        };
        let size = Size::new(configure.new_size.0, configure.new_size.1);
        if size.is_empty() {
            return;
        }
        let shm = &self.shm;
        let Some(bg) = self.outputs[i].background.as_mut() else {
            return;
        };
        let changed = bg.configured != size;
        bg.configured = size;
        if bg.video.is_some() {
            if changed {
                self.resize_video(i);
            }
            return;
        }
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
    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        // El compositor ya mostró el cuadro anterior de ese monitor: se puede dibujar otro.
        let Some(i) = self.outputs.iter().position(|o| {
            o.background
                .as_ref()
                .is_some_and(|b| b.layer.wl_surface() == surface)
        }) else {
            return;
        };
        let redraw = match self.outputs[i]
            .background
            .as_mut()
            .and_then(|b| b.video.as_mut())
        {
            Some(v) => {
                v.frame_pending = false;
                v.redraw
            }
            None => false,
        };
        if redraw {
            self.render_output(i);
        }
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
