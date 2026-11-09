//! Hilo de ventanas y la manija (`WindowsSurface`) que cumple el contrato `Surface`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use miraishin_core::fit::FitMode;
use miraishin_core::pause::Cover;
use miraishin_core::playback::PlaybackSettings;
use miraishin_core::surface::{
    check_frame, EventSink, Frame, OutputInfo, Size, Surface, SurfaceError, SurfaceEvent,
};
use miraishin_render_video::Player;
use windows_sys::Win32::Foundation::{
    GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, EndPaint, EnumDisplayMonitors, GetMonitorInfoW, InvalidateRect,
    SetStretchBltMode, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HALFTONE, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows_sys::Win32::System::SystemServices::GUID_SESSION_DISPLAY_STATUS;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    MDT_EFFECTIVE_DPI,
};
use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows, FindWindowExW,
    FindWindowW, GetAncestor, GetClassNameW, GetClientRect, GetForegroundWindow, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, KillTimer, PeekMessageW, PostThreadMessageW, RegisterClassExW,
    RegisterWindowMessageW, SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetTimer,
    SetWindowLongPtrW, SetWindowPos, TranslateMessage, CHILDID_SELF, DEVICE_NOTIFY_WINDOW_HANDLE,
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, HTTRANSPARENT, HWND_TOP,
    LWA_ALPHA, MA_NOACTIVATE, MSG, OBJID_WINDOW, PBT_POWERSETTINGCHANGE, PM_NOREMOVE, PM_REMOVE,
    SMTO_ABORTIFHUNG, SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_APP,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCDESTROY, WM_NCHITTEST,
    WM_PAINT, WM_POWERBROADCAST, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::layout::{
    child_rect, covered, diff, form_from_exstyle, monitor_of, output_info, rgba_to_bgrx,
    DesktopForm, Monitor, PxRect, SpawnArgs, TopWindow,
};
use crate::signals::guid_eq;

/// Por qué no se pudo conectar. Códigos estables para `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// No hay escritorio de Explorer (`Progman` no existe).
    NoDesktop,
    /// No se pudo registrar la clase de ventana o arrancar el hilo.
    Unavailable,
}

impl ConnectError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoDesktop => "windows_no_desktop",
            Self::Unavailable => "windows_unavailable",
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
    SetPlayback(PlaybackSettings),
    Shutdown,
}

struct Shared {
    outputs: Vec<OutputInfo>,
    alive: bool,
}

/// Mensaje para despertar al hilo cuando hay órdenes en el canal.
const WM_WAKE: u32 = WM_APP + 1;
const TIMER_DISPLAY: usize = 1;
const TIMER_EXPLORER: usize = 2;
/// Cambio de video: si el nuevo no avisa que empezó, el anterior se cierra igual.
const TIMER_VIDEO_SWAP: usize = 3;
const VIDEO_SWAP_TIMEOUT_MS: u32 = 3000;
/// Cambió alguna ventana (frente, tamaño, minimizada…): se mira qué tapa cada monitor un
/// momento después, para juntar los avisos de un arrastre o una animación.
const TIMER_WINDOWS: usize = 4;
const WINDOWS_DELAY_MS: u32 = 200;
/// Cuánto se espera a que mpv abra un video antes de responder al daemon.
const VIDEO_LOAD_TIMEOUT: Duration = Duration::from_secs(8);
/// Cuánto se espera a que un mpv que se cierra suelte su ventana.
const VIDEO_CLOSE_TIMEOUT: Duration = Duration::from_secs(3);
/// Grafito del sistema de diseño (#1c1d21) en BGRX: fondo de la ventana mientras mpv abre.
const GRAPHITE_BGRX: [u8; 4] = [0x21, 0x1d, 0x1c, 0xff];
/// `0x052C`: pide a `Progman` que prepare la capa del fondo.
const SPAWN_WORKERW: u32 = 0x052C;

/// Manija del fondo de Windows. Se usa desde el hilo del daemon; las ventanas las maneja un
/// hilo propio.
pub struct WindowsSurface {
    commands: mpsc::Sender<Command>,
    thread_id: u32,
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
    form: DesktopForm,
}

impl WindowsSurface {
    /// Busca el escritorio, lee los monitores y arranca el hilo. `events` recibe los cambios
    /// de monitores que pasen después.
    pub fn connect(events: EventSink) -> Result<Self, ConnectError> {
        // Conciencia de DPI por monitor (v2): los tamaños llegan en píxeles reales. Si el
        // manifiesto del exe ya la puso, esto falla sin consecuencias.
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let shared = Arc::new(Mutex::new(Shared {
            outputs: Vec::new(),
            alive: true,
        }));
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("surface-windows".into())
            .spawn(move || run(rx, thread_shared, events, ready_tx))
            .map_err(|_| ConnectError::Unavailable)?;
        match ready_rx.recv() {
            Ok(Ok((thread_id, form))) => Ok(Self {
                commands: tx,
                thread_id,
                shared,
                thread: Some(thread),
                form,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err(ConnectError::Unavailable),
        }
    }

    /// Nombre para `status`: `windows_raised` o `windows_classic`.
    pub fn surface_name(&self) -> &'static str {
        self.form.surface_name()
    }

    fn send(&self, cmd: Command) -> Result<(), SurfaceError> {
        self.commands
            .send(cmd)
            .map_err(|_| SurfaceError::Disconnected)?;
        if unsafe { PostThreadMessageW(self.thread_id, WM_WAKE, 0, 0) } == 0 {
            return Err(SurfaceError::Disconnected);
        }
        Ok(())
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

impl Surface for WindowsSurface {
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
            Ok(Err("unknown_output")) => Err(SurfaceError::UnknownOutput),
            Ok(Err(code)) => Err(SurfaceError::Video(code)),
            Err(_) => Err(SurfaceError::Video("video_unplayable")),
        }
    }

    fn clear(&mut self, output: &str) -> Result<(), SurfaceError> {
        self.output(output)?;
        self.send(Command::Clear(output.to_string()))
    }

    /// mpv se pausa y deja de dibujar; queda el último cuadro.
    fn set_paused(&mut self, output: &str, paused: bool) -> Result<(), SurfaceError> {
        self.output(output)?;
        self.send(Command::SetPaused(output.to_string(), paused))
    }

    /// Velocidad, brillo y límite de cuadros: los hace mpv (el límite con el filtro `fps`,
    /// igual que en Sway).
    fn set_playback(&mut self, playback: &PlaybackSettings) -> Result<(), SurfaceError> {
        self.send(Command::SetPlayback(*playback))
    }
}

impl Drop for WindowsSurface {
    fn drop(&mut self) {
        let _ = self.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ---------------------------------------------------------------- hilo de ventanas

/// Un cuadro en el formato de GDI, listo para `StretchDIBits`.
struct Painted {
    size: Size,
    bgrx: Vec<u8>,
}

thread_local! {
    /// Lo que pinta cada ventana (clave: la ventana). Lo lee `WM_PAINT`; separado del resto
    /// del estado porque las llamadas a Windows pueden volver a entrar al procedimiento de
    /// ventana mientras el hilo está a medio cambiar algo.
    static PAINTED: RefCell<HashMap<isize, Rc<Painted>>> = RefCell::new(HashMap::new());
    /// Cambiaron los monitores o la escala (con un respiro para juntar varios avisos).
    static DISPLAY_DIRTY: Cell<bool> = const { Cell::new(false) };
    /// Explorer se reinició: hay que volver a colgar las ventanas.
    static EXPLORER_RESTARTED: Cell<bool> = const { Cell::new(false) };
    /// Pasó el tiempo del cambio de video: cerrar los reproductores anteriores.
    static VIDEO_SWAP_DUE: Cell<bool> = const { Cell::new(false) };
    /// Mensaje registrado `TaskbarCreated` (lo manda Explorer al arrancar).
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    /// La ventana oculta (para el temporizador de las ventanas, desde el aviso de Windows).
    static WATCHER: Cell<isize> = const { Cell::new(0) };
    /// Ya hay un temporizador de ventanas puesto / llegó su momento.
    static WINDOWS_PENDING: Cell<bool> = const { Cell::new(false) };
    static WINDOWS_DIRTY: Cell<bool> = const { Cell::new(false) };
    /// La pantalla se apagó (`Some(true)`) o se encendió, todavía sin avisar al daemon.
    static SCREEN_POWER: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Dónde se cuelgan las ventanas.
struct Host {
    form: DesktopForm,
    parent: HWND,
    /// Ventana de los íconos si es hermana nuestra: la nuestra va justo debajo.
    below: HWND,
    /// `WorkerW` del fondo en la forma elevada: tiene que quedar debajo de la nuestra.
    workerw: HWND,
}

/// Un video en un monitor. mpv dibuja solo (salida `gpu`, Direct3D 11) en una ventana suya
/// dentro de la nuestra (`wid`). Probado en Windows 11 25H2 (2026-11-01): se ve debajo de los
/// íconos y la barra de tareas; con un contexto OpenGL propio en la ventana, en cambio, el
/// video tapaba todo cuando no quedaba otra ventana encima.
struct Live {
    key: (PathBuf, FitMode),
    player: Player,
    /// El reproductor anterior sigue hasta que el nuevo muestra su primer cuadro.
    old: Option<Player>,
}

struct State {
    hinst: HINSTANCE,
    thread_id: u32,
    watcher: HWND,
    wall_class: Vec<u16>,
    spawn: SpawnArgs,
    monitors: Vec<Monitor>,
    /// Lo elegido por monitor (se conserva aunque Explorer se reinicie).
    frames: HashMap<String, Rc<Painted>>,
    windows: HashMap<String, HWND>,
    /// Video elegido por monitor (se conserva aunque Explorer se reinicie).
    videos: HashMap<String, (PathBuf, FitMode)>,
    players: HashMap<String, Live>,
    /// Reproductores cerrándose en otro hilo (cerrar mpv espera a su ventana, y su ventana
    /// le habla a este hilo: cerrarlo aquí mismo podría trabarse).
    closing: Vec<JoinHandle<()>>,
    host: Option<Host>,
    shared: Arc<Mutex<Shared>>,
    events: EventSink,
    // ---- Pausa y reproducción (fase 3.6)
    /// Monitores con el video en pausa (vale también para un video que se ponga después).
    paused: HashSet<String>,
    playback: PlaybackSettings,
    /// Lo último que se avisó de cada monitor tapado.
    covers: HashMap<String, Cover>,
    /// La pantalla está apagada (Windows lo avisa para todas a la vez).
    screen_off: bool,
}

type Ready = mpsc::Sender<Result<(u32, DesktopForm), ConnectError>>;

fn run(rx: mpsc::Receiver<Command>, shared: Arc<Mutex<Shared>>, events: EventSink, ready: Ready) {
    // SAFETY: llamadas a Windows desde el hilo dueño de las ventanas, con punteros válidos.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        // Crea la cola de mensajes del hilo antes de avisar que está listo.
        PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);

        let hinst = GetModuleHandleW(null());
        let slug = &miraishin_brand::get().slug;
        let wall_class = wide(&format!("{slug}-wallpaper"));
        let watch_class = wide(&format!("{slug}-watcher"));
        if !register_class(hinst, &wall_class, wall_proc)
            || !register_class(hinst, &watch_class, watch_proc)
        {
            tracing::error!(error = GetLastError(), "RegisterClassExW failed");
            let _ = ready.send(Err(ConnectError::Unavailable));
            return;
        }
        TASKBAR_CREATED.set(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()));
        // Ventana oculta de primer nivel: recibe los avisos que Windows manda a todas
        // (pantalla, escala, Explorer reiniciado). Una ventana solo de mensajes no los recibe.
        let watcher = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            watch_class.as_ptr(),
            null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if watcher.is_null() {
            tracing::error!(error = GetLastError(), "watcher window failed");
            let _ = ready.send(Err(ConnectError::Unavailable));
            return;
        }

        let progman = find_progman();
        if progman.is_null() {
            DestroyWindow(watcher);
            let _ = ready.send(Err(ConnectError::NoDesktop));
            return;
        }
        let form = form_of(progman);
        let env_key = format!("{}_WORKERW", slug.to_uppercase().replace('-', "_"));
        let spawn = SpawnArgs::parse(std::env::var(&env_key).ok().as_deref());
        tracing::info!(?form, ?spawn, "windows desktop found");

        let mut state = State {
            hinst,
            thread_id: GetCurrentThreadId(),
            watcher,
            wall_class,
            spawn,
            monitors: Vec::new(),
            frames: HashMap::new(),
            windows: HashMap::new(),
            videos: HashMap::new(),
            players: HashMap::new(),
            closing: Vec::new(),
            host: None,
            shared,
            events,
            paused: HashSet::new(),
            playback: PlaybackSettings::default(),
            covers: HashMap::new(),
            screen_off: false,
        };
        state.monitors = enumerate_monitors();
        state.publish_outputs();
        WATCHER.set(watcher as isize);
        // Pantalla apagada o encendida (Windows manda el estado actual al registrarse).
        if RegisterPowerSettingNotification(
            watcher,
            &GUID_SESSION_DISPLAY_STATUS,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        ) == 0
        {
            tracing::warn!(
                error = GetLastError(),
                "screen power notification not available"
            );
        }
        // Avisos de ventanas de los demás programas (las nuestras no): sin preguntar a cada
        // rato, Windows llama a `window_event` desde este mismo bucle.
        let mut hooks: Vec<HWINEVENTHOOK> = Vec::new();
        for (min, max) in [
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
            (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            (EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE),
            (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
            (EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED),
        ] {
            let h = SetWinEventHook(
                min,
                max,
                null_mut(),
                Some(window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if h.is_null() {
                tracing::warn!(min, "window events not available");
            } else {
                hooks.push(h);
            }
        }
        let _ = ready.send(Ok((GetCurrentThreadId(), form)));
        state.scan_windows();

        'outer: loop {
            if GetMessageW(&mut msg, null_mut(), 0, 0) <= 0 {
                break;
            }
            if msg.hwnd.is_null() && msg.message == WM_WAKE {
                while let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        Command::Show(name, frame) => state.show(name, frame),
                        Command::ShowVideo(name, path, mode, reply) => {
                            let _ = reply.send(state.show_video(name, path, mode));
                        }
                        Command::Clear(name) => state.clear(&name),
                        Command::SetPaused(name, paused) => state.set_paused(&name, paused),
                        Command::SetPlayback(p) => state.set_playback(p),
                        Command::Shutdown => break 'outer,
                    }
                }
                // El mismo mensaje lo manda mpv cuando tiene eventos.
                state.on_mpv();
            } else {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if EXPLORER_RESTARTED.replace(false) {
                DISPLAY_DIRTY.set(false);
                state.rescan();
                state.reattach();
            } else if DISPLAY_DIRTY.replace(false) {
                state.rescan();
            }
            if VIDEO_SWAP_DUE.replace(false) {
                state.close_old_players();
            }
            if WINDOWS_DIRTY.replace(false) {
                state.scan_windows();
            }
            if let Some(off) = SCREEN_POWER.take() {
                state.screen_power(off);
            }
        }

        for h in hooks {
            UnhookWinEvent(h);
        }

        for name in state.windows.keys().cloned().collect::<Vec<_>>() {
            state.destroy_window(&name);
        }
        state.wait_closing();
        if let Ok(mut s) = state.shared.lock() {
            s.alive = false;
        }
        DestroyWindow(watcher);
    }
}

impl State {
    fn publish_outputs(&self) -> Vec<OutputInfo> {
        let outputs: Vec<OutputInfo> = self.monitors.iter().map(output_info).collect();
        if let Ok(mut s) = self.shared.lock() {
            s.outputs = outputs.clone();
        }
        outputs
    }

    unsafe fn show(&mut self, name: String, frame: Frame) {
        let painted = Rc::new(Painted {
            size: frame.size,
            bgrx: rgba_to_bgrx(&frame.rgba),
        });
        drop(frame);
        self.frames.insert(name.clone(), painted.clone());
        // La imagen se pinta primero y recién después se cierra el video que la tapaba.
        self.videos.remove(&name);
        if let Some(&hwnd) = self.windows.get(&name) {
            if is_ours(hwnd) {
                PAINTED.with_borrow_mut(|p| p.insert(hwnd as isize, painted));
                InvalidateRect(hwnd, null(), 0);
                self.stop_video(&name);
                return;
            }
            self.windows.remove(&name);
        }
        self.stop_video(&name);
        self.create_window(&name);
    }

    unsafe fn clear(&mut self, name: &str) {
        self.frames.remove(name);
        self.videos.remove(name);
        self.paused.remove(name);
        self.destroy_window(name);
    }

    /// Pausa o reanuda el video de un monitor (y lo recuerda para un video nuevo).
    fn set_paused(&mut self, name: &str, paused: bool) {
        if paused {
            self.paused.insert(name.to_string());
        } else {
            self.paused.remove(name);
        }
        if let Some(live) = self.players.get(name) {
            if live.player.set_paused(paused).is_err() {
                tracing::warn!(output = name, "mpv did not take the pause");
            }
        }
    }

    /// Velocidad, brillo y límite para todos los reproductores, y los que vengan.
    fn set_playback(&mut self, p: PlaybackSettings) {
        if self.playback == p {
            return;
        }
        self.playback = p;
        for (name, live) in &self.players {
            if !live.player.set_playback(&p) {
                tracing::warn!(output = %name, "mpv did not take every playback setting");
            }
        }
        tracing::info!(
            speed = p.speed,
            brightness = p.brightness,
            fps_limit = p.fps_limit,
            "video playback settings"
        );
    }

    /// La pantalla se apagó o se encendió: Windows lo dice para todas a la vez.
    fn screen_power(&mut self, off: bool) {
        if self.screen_off == off {
            return;
        }
        self.screen_off = off;
        tracing::info!(off, "screen power");
        for m in &self.monitors {
            (self.events)(SurfaceEvent::ScreenPower {
                output: m.name.clone(),
                off,
            });
        }
    }

    /// Lee las ventanas de primer nivel y avisa los monitores cuyo fondo quedó tapado o
    /// destapado (pantalla completa o maximizada, regla en `layout::covered`).
    unsafe fn scan_windows(&mut self) {
        let mut list: Vec<TopWindow> = Vec::new();
        EnumWindows(Some(collect_window), &mut list as *mut _ as LPARAM);
        // Un juego en pantalla completa exclusiva (Direct3D): el monitor de la ventana del frente.
        let mut quns = 0;
        let d3d = if SHQueryUserNotificationState(&mut quns) == 0
            && quns == QUNS_RUNNING_D3D_FULL_SCREEN
        {
            let fg = GetForegroundWindow();
            window_rect(fg)
                .and_then(|r| monitor_of(r, &self.monitors))
                .map(|m| m.name.clone())
        } else {
            None
        };
        let now = covered(&list, &self.monitors, d3d.as_deref());
        for m in &self.monitors {
            let new = now.get(&m.name).copied().unwrap_or_default();
            let old = self.covers.get(&m.name).copied().unwrap_or_default();
            if new != old {
                tracing::debug!(output = %m.name, fullscreen = new.fullscreen,
                    maximized = new.maximized, "windows over the wallpaper");
                (self.events)(SurfaceEvent::Covered {
                    output: m.name.clone(),
                    fullscreen: new.fullscreen,
                    maximized: new.maximized,
                });
            }
        }
        self.covers = now;
    }

    unsafe fn destroy_window(&mut self, name: &str) {
        // mpv suelta su ventana (hija de la nuestra) antes de que la nuestra se destruya.
        if self.stop_video(name) {
            self.wait_closing();
        }
        if let Some(hwnd) = self.windows.remove(name) {
            if is_ours(hwnd) {
                DestroyWindow(hwnd);
            }
        }
    }

    /// Pone un video en un monitor. Responde cuando mpv ya abrió el archivo (o falló).
    unsafe fn show_video(
        &mut self,
        name: String,
        path: PathBuf,
        mode: FitMode,
    ) -> Result<(), &'static str> {
        if !self.monitors.iter().any(|m| m.name == name) {
            return Err("unknown_output");
        }
        let key = (path, mode);
        let alive = self.windows.get(&name).is_some_and(|&h| is_ours(h));
        if alive && self.players.get(&name).is_some_and(|l| l.key == key) {
            return Ok(());
        }
        self.videos.insert(name.clone(), key);
        self.start_video(&name)
    }

    /// Arranca el video elegido para `name` en su ventana (la crea si hace falta). El
    /// reproductor anterior, si lo hay, sigue hasta que el nuevo muestra su primer cuadro.
    unsafe fn start_video(&mut self, name: &str) -> Result<(), &'static str> {
        let Some(key) = self.videos.get(name).cloned() else {
            return Ok(());
        };
        // La ventana necesita algo que pintar debajo del video: lo que había, o grafito.
        if !self.frames.contains_key(name) {
            let graphite = Rc::new(Painted {
                size: Size::new(1, 1),
                bgrx: GRAPHITE_BGRX.to_vec(),
            });
            self.frames.insert(name.to_string(), graphite);
        }
        let hwnd = match self.windows.get(name) {
            Some(&h) if is_ours(h) => h,
            _ => {
                self.windows.remove(name);
                self.create_window(name);
                *self.windows.get(name).ok_or("video_no_window")?
            }
        };
        let thread = self.thread_id;
        let wakeup = Box::new(move || {
            // SAFETY: solo deja un mensaje en la cola del hilo de ventanas.
            unsafe { PostThreadMessageW(thread, WM_WAKE, 0, 0) };
        });
        let player = Player::in_window(key.1, hwnd as i64, wakeup).map_err(|e| e.as_str())?;
        if let Err(e) = player.load(&key.0, VIDEO_LOAD_TIMEOUT) {
            self.close_player(player);
            return Err(e.as_str());
        }
        if !self.playback.is_default() && !player.set_playback(&self.playback) {
            tracing::warn!(output = name, "mpv did not take every playback setting");
        }
        if self.paused.contains(name) {
            let _ = player.set_paused(true);
        }
        let old = match self.players.remove(name) {
            Some(prev) => {
                if let Some(older) = prev.old {
                    self.close_player(older);
                }
                Some(prev.player)
            }
            None => None,
        };
        if old.is_some() {
            SetTimer(self.watcher, TIMER_VIDEO_SWAP, VIDEO_SWAP_TIMEOUT_MS, None);
        }
        tracing::info!(output = name, "video started");
        self.players
            .insert(name.to_string(), Live { key, player, old });
        Ok(())
    }

    /// Cierra el video de un monitor (sin olvidar cuál era). `true` si había uno.
    fn stop_video(&mut self, name: &str) -> bool {
        let Some(live) = self.players.remove(name) else {
            return false;
        };
        self.close_player(live.player);
        if let Some(old) = live.old {
            self.close_player(old);
        }
        true
    }

    /// Cierra un reproductor en otro hilo (ver `closing`).
    fn close_player(&mut self, player: Player) {
        self.closing.retain(|h| !h.is_finished());
        if let Ok(h) = std::thread::Builder::new()
            .name("mpv-close".into())
            .spawn(move || drop(player))
        {
            self.closing.push(h);
        }
    }

    /// Espera a que terminen de cerrarse los reproductores, atendiendo los mensajes que
    /// manden sus ventanas. Los avisos `WM_WAKE` que lleguen mientras tanto se vuelven a
    /// mandar al final, para no perder órdenes.
    unsafe fn wait_closing(&mut self) {
        let end = Instant::now() + VIDEO_CLOSE_TIMEOUT;
        let mut woke = false;
        let mut msg: MSG = std::mem::zeroed();
        while self.closing.iter().any(|h| !h.is_finished()) && Instant::now() < end {
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.hwnd.is_null() && msg.message == WM_WAKE {
                    woke = true;
                } else {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.closing.iter().any(|h| !h.is_finished()) {
            tracing::warn!("mpv is taking long to close");
        }
        self.closing.retain(|h| !h.is_finished());
        if woke {
            PostThreadMessageW(self.thread_id, WM_WAKE, 0, 0);
        }
    }

    /// mpv avisó: eventos de cada reproductor. Cuando el nuevo ya se ve, se cierra el
    /// anterior.
    fn on_mpv(&mut self) {
        let mut started = Vec::new();
        for (name, live) in &self.players {
            let polled = live.player.poll();
            if let Some(code) = polled.failed {
                tracing::warn!(output = %name, code, "video stopped");
            }
            if polled.started && live.old.is_some() {
                started.push(name.clone());
            }
        }
        for name in started {
            if let Some(old) = self.players.get_mut(&name).and_then(|l| l.old.take()) {
                self.close_player(old);
            }
        }
    }

    fn close_old_players(&mut self) {
        let old: Vec<Player> = self
            .players
            .values_mut()
            .filter_map(|l| l.old.take())
            .collect();
        for p in old {
            self.close_player(p);
        }
    }

    /// Busca `Progman` y prepara la capa del fondo. Se hace la primera vez que hace falta y
    /// otra vez si Explorer se reinicia.
    unsafe fn ensure_host(&mut self) -> bool {
        if let Some(h) = &self.host {
            if IsWindow(h.parent) != 0 {
                return true;
            }
        }
        self.host = None;
        let progman = find_progman();
        if progman.is_null() {
            tracing::warn!("Progman not found");
            return false;
        }
        let form = form_of(progman);
        if let Some((w, l)) = self.spawn.resolve(form) {
            let mut result = 0usize;
            SendMessageTimeoutW(
                progman,
                SPAWN_WORKERW,
                w,
                l,
                SMTO_NORMAL | SMTO_ABORTIFHUNG,
                1000,
                &mut result,
            );
        }
        let defview_class = wide("SHELLDLL_DefView");
        let workerw_class = wide("WorkerW");
        let host = match form {
            DesktopForm::Raised => Host {
                form,
                parent: progman,
                below: FindWindowExW(progman, null_mut(), defview_class.as_ptr(), null()),
                workerw: FindWindowExW(progman, null_mut(), workerw_class.as_ptr(), null()),
            },
            DesktopForm::Classic => {
                // La ventana de primer nivel que tiene los íconos; la WorkerW del fondo es
                // la que le sigue.
                let mut found: (HWND, HWND) = (null_mut(), null_mut());
                EnumWindows(Some(find_icons_window), &mut found as *mut _ as LPARAM);
                if found.1.is_null() {
                    // Sin WorkerW: dentro de Progman, debajo de los íconos si están ahí.
                    Host {
                        form,
                        parent: progman,
                        below: FindWindowExW(progman, null_mut(), defview_class.as_ptr(), null()),
                        workerw: null_mut(),
                    }
                } else {
                    Host {
                        form,
                        parent: found.1,
                        below: null_mut(),
                        workerw: null_mut(),
                    }
                }
            }
        };
        tracing::info!(
            ?form,
            parent = host.parent as isize,
            icons = host.below as isize,
            workerw = host.workerw as isize,
            "wallpaper host ready"
        );
        self.host = Some(host);
        true
    }

    unsafe fn create_window(&mut self, name: &str) {
        let Some(monitor) = self.monitors.iter().find(|m| m.name == name).cloned() else {
            return;
        };
        let Some(painted) = self.frames.get(name).cloned() else {
            return;
        };
        if !self.ensure_host() {
            return;
        }
        let Some(host) = &self.host else { return };
        let raised = host.form == DesktopForm::Raised;
        let size = monitor.rect.size();
        // Sin estilos extendidos al crearla: con WS_EX_TRANSPARENT, TOOLWINDOW o NOACTIVATE
        // no se probó que se vea, y los clics ya pasan por WM_NCHITTEST.
        let hwnd = CreateWindowExW(
            0,
            self.wall_class.as_ptr(),
            null(),
            WS_POPUP,
            0,
            0,
            size.width as i32,
            size.height as i32,
            null_mut(),
            null_mut(),
            self.hinst,
            null(),
        );
        if hwnd.is_null() {
            tracing::error!(
                error = GetLastError(),
                output = name,
                "CreateWindowExW failed"
            );
            return;
        }
        PAINTED.with_borrow_mut(|p| p.insert(hwnd as isize, painted));
        // WS_CLIPCHILDREN: lo que pinta GDI no tapa la ventana de mpv (hija de la nuestra).
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN) as isize,
        );
        if SetParent(hwnd, host.parent).is_null() {
            tracing::error!(error = GetLastError(), output = name, "SetParent failed");
            DestroyWindow(hwnd);
            return;
        }
        let after = if host.below.is_null() {
            HWND_TOP
        } else {
            host.below
        };
        let (x, y, w, h) = child_rect(monitor.rect, origin_of(host.parent));
        SetWindowPos(
            hwnd,
            after,
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
        if raised {
            // El estilo layered va DESPUÉS de colgarla de Progman y mostrarla. Probado en
            // Windows 11 25H2 (2026-10-31): puesto antes (en la ventana oculta, antes de
            // SetParent) la ventana queda en el orden correcto pero no se ve nunca; puesto
            // con la ventana ya hija y visible, se ve debajo de los íconos.
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, WS_EX_LAYERED as isize);
            if SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) == 0 {
                tracing::warn!(error = GetLastError(), "SetLayeredWindowAttributes failed");
            }
            if !host.workerw.is_null() {
                SetWindowPos(
                    host.workerw,
                    hwnd,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            InvalidateRect(hwnd, null(), 0);
        }
        tracing::info!(output = name, x, y, w, h, "wallpaper window shown");
        self.windows.insert(name.to_string(), hwnd);
    }

    /// Lee los monitores de nuevo, avisa los cambios y acomoda las ventanas.
    unsafe fn rescan(&mut self) {
        let old: Vec<OutputInfo> = self.monitors.iter().map(output_info).collect();
        self.monitors = enumerate_monitors();
        let new = self.publish_outputs();
        for name in self.windows.keys().cloned().collect::<Vec<_>>() {
            if !self.monitors.iter().any(|m| m.name == name) {
                self.frames.remove(&name);
                self.videos.remove(&name);
                self.destroy_window(&name);
            }
        }
        if let Some(host) = &self.host {
            let origin = origin_of(host.parent);
            for (name, &hwnd) in &self.windows {
                if let Some(m) = self.monitors.iter().find(|m| &m.name == name) {
                    let (x, y, w, h) = child_rect(m.rect, origin);
                    SetWindowPos(hwnd, null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
                    InvalidateRect(hwnd, null(), 0);
                }
            }
        }
        for event in diff(&old, &new) {
            (self.events)(event);
        }
        self.covers
            .retain(|name, _| self.monitors.iter().any(|m| &m.name == name));
        self.scan_windows();
    }

    /// Explorer se reinició: sus ventanas (y las nuestras, hijas de ellas) ya no existen.
    unsafe fn reattach(&mut self) {
        tracing::info!("explorer restarted, attaching again");
        for name in self.windows.keys().cloned().collect::<Vec<_>>() {
            self.destroy_window(&name);
        }
        self.host = None;
        for name in self.frames.keys().cloned().collect::<Vec<_>>() {
            self.create_window(&name);
        }
        for name in self.videos.keys().cloned().collect::<Vec<_>>() {
            if let Err(code) = self.start_video(&name) {
                tracing::warn!(output = %name, code, "video could not start again");
            }
        }
    }
}

/// La ventana sigue siendo nuestra (no se destruyó con Explorer). Se mira la tabla de cuadros
/// y no solo `IsWindow`, porque Windows puede reutilizar el número de una ventana muerta.
fn is_ours(hwnd: HWND) -> bool {
    PAINTED.with_borrow(|p| p.contains_key(&(hwnd as isize))) && unsafe { IsWindow(hwnd) } != 0
}

unsafe fn find_progman() -> HWND {
    FindWindowW(wide("Progman").as_ptr(), null())
}

unsafe fn form_of(progman: HWND) -> DesktopForm {
    form_from_exstyle(GetWindowLongPtrW(progman, GWL_EXSTYLE) as u32)
}

/// Esquina del área de `parent` en coordenadas de pantalla.
unsafe fn origin_of(parent: HWND) -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    ClientToScreen(parent, &mut p);
    (p.x, p.y)
}

unsafe extern "system" fn find_icons_window(top: HWND, data: LPARAM) -> i32 {
    let defview = FindWindowExW(top, null_mut(), wide("SHELLDLL_DefView").as_ptr(), null());
    if defview.is_null() {
        return 1;
    }
    let found = &mut *(data as *mut (HWND, HWND));
    found.0 = top;
    found.1 = FindWindowExW(null_mut(), top, wide("WorkerW").as_ptr(), null());
    0
}

unsafe fn enumerate_monitors() -> Vec<Monitor> {
    let mut list: Vec<Monitor> = Vec::new();
    EnumDisplayMonitors(
        null_mut(),
        null(),
        Some(monitor_proc),
        &mut list as *mut _ as LPARAM,
    );
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

unsafe extern "system" fn monitor_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> i32 {
    let list = &mut *(data as *mut Vec<Monitor>);
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon, &mut info as *mut _ as *mut MONITORINFO) == 0 {
        return 1;
    }
    let len = info
        .szDevice
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(info.szDevice.len());
    let r = info.monitorInfo.rcMonitor;
    let (mut dpi, mut _dpi_y) = (0u32, 0u32);
    if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi, &mut _dpi_y) != 0 {
        dpi = 0;
    }
    list.push(Monitor {
        name: String::from_utf16_lossy(&info.szDevice[..len]),
        rect: PxRect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        },
        dpi,
    });
    1
}

unsafe fn register_class(
    hinst: HINSTANCE,
    name: &[u16],
    proc_: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> bool {
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(proc_),
        hInstance: hinst,
        lpszClassName: name.as_ptr(),
        ..std::mem::zeroed()
    };
    RegisterClassExW(&class) != 0
}

/// Procedimiento de las ventanas del fondo: pintar el cuadro y dejar pasar todo lo demás.
unsafe extern "system" fn wall_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let painted = PAINTED.with_borrow(|p| p.get(&(hwnd as isize)).cloned());
            if let Some(p) = painted {
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let mut bmi: BITMAPINFO = std::mem::zeroed();
                bmi.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: p.size.width as i32,
                    // Negativo: filas de arriba hacia abajo, como el cuadro.
                    biHeight: -(p.size.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                };
                SetStretchBltMode(hdc, HALFTONE);
                StretchDIBits(
                    hdc,
                    0,
                    0,
                    client.right - client.left,
                    client.bottom - client.top,
                    0,
                    0,
                    p.size.width as i32,
                    p.size.height as i32,
                    p.bgrx.as_ptr().cast(),
                    &bmi,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_NCDESTROY => {
            PAINTED.with_borrow_mut(|p| p.remove(&(hwnd as isize)));
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Procedimiento de la ventana oculta: junta los avisos con un temporizador corto y deja la
/// marca para el bucle principal (que hace el trabajo fuera de este procedimiento).
unsafe extern "system" fn watch_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
            SetTimer(hwnd, TIMER_DISPLAY, 500, None);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_TIMER => {
            KillTimer(hwnd, wp);
            match wp {
                TIMER_DISPLAY => DISPLAY_DIRTY.set(true),
                TIMER_EXPLORER => EXPLORER_RESTARTED.set(true),
                TIMER_VIDEO_SWAP => VIDEO_SWAP_DUE.set(true),
                TIMER_WINDOWS => {
                    WINDOWS_PENDING.set(false);
                    WINDOWS_DIRTY.set(true);
                }
                _ => {}
            }
            0
        }
        WM_POWERBROADCAST if wp as u32 == PBT_POWERSETTINGCHANGE && lp != 0 => {
            let setting = &*(lp as *const POWERBROADCAST_SETTING);
            if setting.DataLength >= 4
                && guid_eq(&setting.PowerSetting, &GUID_SESSION_DISPLAY_STATUS)
            {
                // 0 = apagada, 1 = encendida, 2 = atenuada (cuenta como encendida).
                let value = std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>());
                SCREEN_POWER.set(Some(value == 0));
            }
            1
        }
        m if m != 0 && m == TASKBAR_CREATED.get() => {
            SetTimer(hwnd, TIMER_EXPLORER, 1000, None);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Aviso de Windows de que cambió una ventana. Solo cuentan las ventanas de primer nivel;
/// se pone un temporizador corto (si no hay uno) y la lista se lee al cumplirse.
unsafe extern "system" fn window_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.is_null() || id_object != OBJID_WINDOW || id_child != CHILDID_SELF as i32 {
        return;
    }
    if GetAncestor(hwnd, GA_ROOT) != hwnd {
        return;
    }
    if !WINDOWS_PENDING.replace(true) {
        SetTimer(WATCHER.get() as HWND, TIMER_WINDOWS, WINDOWS_DELAY_MS, None);
    }
}

/// Clases que nunca tapan el fondo: el escritorio, la barra de tareas y sus partes.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

/// Junta las ventanas de primer nivel que cuentan para tapar el fondo.
unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> i32 {
    let list = &mut *(data as *mut Vec<TopWindow>);
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    // En otro escritorio virtual, o una app de la tienda suspendida.
    let mut cloaked: u32 = 0;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED as u32,
        &mut cloaked as *mut u32 as *mut _,
        4,
    ) == 0
        && cloaked != 0
    {
        return 1;
    }
    // Herramientas, capas transparentes que dejan pasar los clics (superposiciones de juegos
    // o de grabación) y ventanas que no se pueden activar.
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    if ex & (WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) != 0 {
        return 1;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == GetCurrentProcessId() {
        return 1;
    }
    let mut class = [0u16; 64];
    let n = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
    if n > 0 {
        let name = String::from_utf16_lossy(&class[..n as usize]);
        if SHELL_CLASSES.contains(&name.as_str()) {
            return 1;
        }
    }
    let Some(rect) = window_rect(hwnd) else {
        return 1;
    };
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return 1;
    }
    list.push(TopWindow {
        rect,
        maximized: IsZoomed(hwnd) != 0,
        minimized: IsIconic(hwnd) != 0,
    });
    1
}

/// Borde visible de una ventana (sin la sombra de Windows 10/11); si DWM no lo da, el de
/// `GetWindowRect`.
unsafe fn window_rect(hwnd: HWND) -> Option<PxRect> {
    if hwnd.is_null() {
        return None;
    }
    let mut r: RECT = std::mem::zeroed();
    let ok = DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS as u32,
        &mut r as *mut RECT as *mut _,
        std::mem::size_of::<RECT>() as u32,
    ) == 0
        || GetWindowRect(hwnd, &mut r) != 0;
    ok.then_some(PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
