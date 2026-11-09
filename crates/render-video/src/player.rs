//! `Player`: un archivo en bucle, sin sonido, dibujado con OpenGL donde diga la superficie.
//!
//! Hilos (reglas de `render.h`): todo `Player` se usa desde un solo hilo (el de la
//! superficie). libmpv avisa desde sus hilos con `wakeup`, que solo debe despertar a ese
//! hilo, nunca llamar a mpv.

use std::ffi::{c_char, c_void, CString};
use std::path::Path;
use std::time::{Duration, Instant};

use miraishin_core::fit::FitMode;
use miraishin_core::playback::{self, PlaybackSettings};

use crate::ffi::{self, Api};
use crate::options;

/// Por qué un video no se puede reproducir. Códigos estables para `outputs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoError {
    /// El sistema no tiene libmpv.
    MpvUnavailable,
    /// mpv no arrancó o no pudo abrir el archivo.
    Unplayable,
    /// No hay decodificador para el video (por ejemplo, falta H.264).
    NoDecoder,
    /// mpv no pudo usar el contexto OpenGL.
    GlFailed,
}

impl VideoError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MpvUnavailable => "mpv_unavailable",
            Self::Unplayable => "video_unplayable",
            Self::NoDecoder => "video_no_decoder",
            Self::GlFailed => "video_gl_failed",
        }
    }

    fn from_code(code: &str) -> Self {
        if code == "video_no_decoder" {
            Self::NoDecoder
        } else {
            Self::Unplayable
        }
    }
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for VideoError {}

type Wakeup = Box<dyn Fn() + Send + Sync>;

/// Lo que dejó `Player::poll`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Polled {
    /// Código si el video falló.
    pub failed: Option<&'static str>,
    /// El video empezó a reproducirse (ya se ve un cuadro).
    pub started: bool,
}

unsafe extern "C" fn trampoline(ctx: *mut c_void) {
    // SAFETY: `ctx` es el `Wakeup` en caja que vive dentro del `Player` hasta después de
    // destruir mpv (ver `Drop`).
    let f = unsafe { &*(ctx as *const Wakeup) };
    f();
}

/// `get_proc_address` de OpenGL que da la superficie (por ejemplo `eglGetProcAddress`).
pub type GetProcAddress = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;

pub struct Player {
    api: &'static Api,
    handle: *mut ffi::Handle,
    render: *mut ffi::RenderContext,
    /// Se le pasa a libmpv como puntero: tiene que vivir más que `handle` y `render`.
    wakeup: Box<Wakeup>,
}

// SAFETY: libmpv permite usar un `mpv_handle` desde cualquier hilo; el `Player` se mueve
// entero al hilo de la superficie y solo se usa ahí.
unsafe impl Send for Player {}

impl Player {
    /// Arranca mpv con el encuadre `mode`, sin archivo todavía. `wakeup` se llama desde
    /// hilos de mpv cuando hay eventos o un cuadro nuevo. Orden de uso: `new` → `attach_gl`
    /// → `load` (mpv necesita el contexto de dibujo antes de abrir el archivo; sin él da el
    /// video por imposible de reproducir).
    pub fn new(mode: FitMode, wakeup: Wakeup) -> Result<Self, VideoError> {
        Self::start(&options::mode(mode), wakeup)
    }

    /// mpv dibuja solo, con su salida `gpu`, dentro de la ventana nativa `wid` (en Windows,
    /// un `HWND`). Sin `attach_gl`: `load` ya se puede llamar. Es el camino B de la prueba
    /// del 2.8 (`tools/win-video-probe`).
    pub fn in_window(mode: FitMode, wid: i64, wakeup: Wakeup) -> Result<Self, VideoError> {
        let wid = wid.to_string();
        let mut extra: Vec<(&str, &str)> = options::mode(mode);
        extra.push(("vo", "gpu"));
        extra.push(("wid", &wid));
        // Que la ventana de mpv no tome el cursor ni el teclado.
        extra.push(("input-cursor", "no"));
        extra.push(("cursor-autohide", "no"));
        Self::start(&extra, wakeup)
    }

    fn start(extra: &[(&str, &str)], wakeup: Wakeup) -> Result<Self, VideoError> {
        let api = ffi::api().ok_or(VideoError::MpvUnavailable)?;
        // SAFETY: llamadas según `client.h`; `handle` se comprueba antes de usarlo.
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(VideoError::Unplayable);
        }
        let player = Self {
            api,
            handle,
            render: std::ptr::null_mut(),
            wakeup: Box::new(wakeup),
        };
        for (k, v) in options::BASE.iter().chain(extra.iter()) {
            player.set(k, v);
        }
        for attempt in options::background() {
            if attempt.iter().all(|(k, v)| player.set(k, v)) {
                break;
            }
        }
        // SAFETY: `handle` es válido.
        if unsafe { (api.initialize)(handle) } < 0 {
            return Err(VideoError::Unplayable);
        }
        // Mensajes de mpv al registro del daemon. Nivel con `MIRAISHIN_MPV_LOG` (por defecto
        // `warn`; `v` o `debug` para diagnosticar).
        let level = std::env::var("MIRAISHIN_MPV_LOG").unwrap_or_else(|_| "warn".into());
        if let Ok(level) = CString::new(level) {
            // SAFETY: `handle` válido, cadena terminada en 0.
            unsafe { (api.request_log_messages)(handle, level.as_ptr()) };
        }
        let ctx = &*player.wakeup as *const Wakeup as *mut c_void;
        // SAFETY: `ctx` vive tanto como el `Player` (ver `Drop`).
        unsafe { (api.set_wakeup_callback)(handle, Some(trampoline), ctx) };
        Ok(player)
    }

    /// Abre `path` y espera hasta `timeout` a que cargue, para poder contestar al daemon si
    /// el archivo no sirve.
    pub fn load(&self, path: &Path, timeout: Duration) -> Result<(), VideoError> {
        let file = path.to_str().ok_or(VideoError::Unplayable)?;
        self.command(&["loadfile", file])?;
        self.wait_loaded(timeout)
    }

    /// Comprueba que un archivo se puede abrir y decodificar, sin dibujar nada (mpv sin
    /// salida de video). Sirve para diagnóstico y pruebas.
    pub fn check(path: &Path, timeout: Duration) -> Result<(), VideoError> {
        let p = Self::start(&[("vo", "null")], Box::new(|| {}))?;
        p.load(path, timeout)
    }

    /// Pausa (`true`) o reanuda. En pausa mpv deja de decodificar y queda el último cuadro.
    pub fn set_paused(&self, paused: bool) -> Result<(), VideoError> {
        self.command(&["set", "pause", if paused { "yes" } else { "no" }])
    }

    /// Velocidad, brillo y límite de cuadros (fase 3.5). Se puede llamar con el video
    /// andando. Devuelve `false` si mpv no tomó alguno (sigue con lo que sí tomó).
    pub fn set_playback(&self, p: &PlaybackSettings) -> bool {
        let speed = self.command(&["set", "speed", &p.speed_text()]).is_ok();
        let brightness = self
            .command(&["set", "brightness", &p.brightness.to_string()])
            .is_ok();
        // Si mpv no sabe los cuadros del video, el filtro con `source_fps` no arranca: se
        // usa un ritmo fijo.
        let fps = self
            .command(&["set", "vf", &playback::mpv_fps_filter(p.fps_limit)])
            .is_ok()
            || self
                .command(&["set", "vf", &playback::mpv_fps_filter_plain(p.fps_limit)])
                .is_ok();
        speed && brightness && fps
    }

    fn set(&self, name: &str, value: &str) -> bool {
        let (Ok(n), Ok(v)) = (CString::new(name), CString::new(value)) else {
            return false;
        };
        // SAFETY: cadenas terminadas en 0 que viven durante la llamada.
        unsafe { (self.api.set_option_string)(self.handle, n.as_ptr(), v.as_ptr()) >= 0 }
    }

    fn command(&self, args: &[&str]) -> Result<(), VideoError> {
        let owned: Vec<CString> = args
            .iter()
            .map(|a| CString::new(*a).map_err(|_| VideoError::Unplayable))
            .collect::<Result<_, _>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: lista terminada en NULL de cadenas que viven durante la llamada.
        if unsafe { (self.api.command)(self.handle, ptrs.as_mut_ptr()) } < 0 {
            return Err(VideoError::Unplayable);
        }
        Ok(())
    }

    fn wait_loaded(&self, timeout: Duration) -> Result<(), VideoError> {
        let end = Instant::now() + timeout;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(VideoError::Unplayable);
            }
            // SAFETY: `handle` es válido; el evento vale hasta la próxima llamada.
            let ev = unsafe { &*(self.api.wait_event)(self.handle, left.as_secs_f64()) };
            match ev.event_id {
                ffi::EVENT_FILE_LOADED => return Ok(()),
                ffi::EVENT_END_FILE => {
                    if let Some(code) = end_file(ev) {
                        return Err(VideoError::from_code(code));
                    }
                }
                ffi::EVENT_SHUTDOWN => return Err(VideoError::Unplayable),
                ffi::EVENT_LOG_MESSAGE => log(ev),
                _ => {}
            }
        }
    }

    /// Lee los eventos pendientes sin esperar. Devuelve el código si el video falló.
    pub fn drain_events(&self) -> Option<&'static str> {
        self.poll().failed
    }

    /// Lee los eventos pendientes sin esperar: si el video falló y si ya empezó a verse
    /// (`playback-restart`, después de `load`).
    pub fn poll(&self) -> Polled {
        let mut out = Polled::default();
        loop {
            // SAFETY: como en `wait_loaded`, sin espera.
            let ev = unsafe { &*(self.api.wait_event)(self.handle, 0.0) };
            match ev.event_id {
                ffi::EVENT_NONE => return out,
                ffi::EVENT_END_FILE => out.failed = end_file(ev).or(out.failed),
                ffi::EVENT_PLAYBACK_RESTART => out.started = true,
                ffi::EVENT_LOG_MESSAGE => log(ev),
                _ => {}
            }
        }
    }

    /// Prepara el dibujo con el contexto OpenGL que está activo ahora.
    ///
    /// No se le pasa la conexión de Wayland (`MPV_RENDER_PARAM_WL_DISPLAY`): con ella mpv
    /// 0.37 se cayó (segfault) en Sway sin GPU (2026-10-31), y VA-API abre su propia
    /// conexión (Wayland o DRM) sin ella.
    ///
    /// # Safety
    /// El contexto OpenGL de `get_proc_address` debe estar activo en este hilo, ahora y en
    /// cada `render`.
    pub unsafe fn attach_gl(&mut self, get_proc_address: GetProcAddress) -> Result<(), VideoError> {
        let mut init = ffi::OpenGlInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        let mut params = vec![
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_API_TYPE,
                data: ffi::RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: &mut init as *mut _ as *mut c_void,
            },
        ];
        params.push(ffi::RenderParam {
            kind: ffi::RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        });
        let mut render = std::ptr::null_mut();
        // SAFETY: parámetros según `render.h`, terminados en INVALID; viven durante la llamada.
        let r = unsafe {
            (self.api.render_context_create)(&mut render, self.handle, params.as_mut_ptr())
        };
        if r < 0 || render.is_null() {
            return Err(VideoError::GlFailed);
        }
        self.render = render;
        let ctx = &*self.wakeup as *const Wakeup as *mut c_void;
        // SAFETY: `ctx` vive tanto como el `Player`.
        unsafe { (self.api.render_context_set_update_callback)(render, Some(trampoline), ctx) };
        Ok(())
    }

    /// Después de un `wakeup`: `true` si hay un cuadro nuevo para dibujar.
    pub fn needs_frame(&self) -> bool {
        if self.render.is_null() {
            return false;
        }
        // SAFETY: contexto válido, en el hilo de dibujo.
        unsafe { (self.api.render_context_update)(self.render) & ffi::RENDER_UPDATE_FRAME != 0 }
    }

    /// Dibuja el cuadro actual en la superficie por defecto (FBO 0) de `width`×`height`
    /// píxeles, sin esperar.
    ///
    /// # Safety
    /// El contexto de `attach_gl` debe estar activo, con la superficie de destino puesta.
    pub unsafe fn render(&self, width: i32, height: i32) {
        if self.render.is_null() {
            return;
        }
        let mut fbo = ffi::OpenGlFbo {
            fbo: 0,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip: i32 = 1;
        let mut block: i32 = 0;
        let mut params = [
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_FBO,
                data: &mut fbo as *mut _ as *mut c_void,
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_FLIP_Y,
                data: &mut flip as *mut _ as *mut c_void,
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: &mut block as *mut _ as *mut c_void,
            },
            ffi::RenderParam {
                kind: ffi::RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: según `render.h`; el contexto GL lo garantiza quien llama.
        unsafe { (self.api.render_context_render)(self.render, params.as_mut_ptr()) };
    }

    /// Avisa a mpv que el cuadro ya se mostró (ayuda a llevar el ritmo).
    pub fn report_swap(&self) {
        if !self.render.is_null() {
            // SAFETY: contexto válido.
            unsafe { (self.api.render_context_report_swap)(self.render) };
        }
    }
}

/// Pasa un mensaje de mpv al registro.
fn log(ev: &ffi::Event) {
    if ev.data.is_null() {
        return;
    }
    // SAFETY: para LOG_MESSAGE, `data` apunta a `mpv_event_log_message` con cadenas válidas
    // hasta el próximo evento.
    let m = unsafe { &*(ev.data as *const ffi::EventLogMessage) };
    let text = |p: *const c_char| {
        if p.is_null() {
            String::new()
        } else {
            // SAFETY: cadena terminada en 0 de mpv.
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .trim_end()
                .to_string()
        }
    };
    let (prefix, msg) = (text(m.prefix), text(m.text));
    // log_level: 10 fatal, 20 error, 30 warn, 40 info, 50+ más detalle. Los errores de mpv
    // van como aviso: buscar decodificación por hardware deja «errores» normales (CUDA que no
    // está, por ejemplo); lo que de verdad falla llega como código del video.
    match m.log_level {
        ..=30 => tracing::warn!(target: "mpv", "[{prefix}] {msg}"),
        31..=40 => tracing::info!(target: "mpv", "[{prefix}] {msg}"),
        _ => tracing::debug!(target: "mpv", "[{prefix}] {msg}"),
    }
}

fn end_file(ev: &ffi::Event) -> Option<&'static str> {
    if ev.data.is_null() {
        return None;
    }
    // SAFETY: para END_FILE, `data` apunta a `mpv_event_end_file`.
    let ef = unsafe { &*(ev.data as *const ffi::EventEndFile) };
    options::end_file_code(ef.reason, ef.error)
}

impl Drop for Player {
    fn drop(&mut self) {
        // SAFETY: primero el contexto de dibujo (con el contexto GL activo, lo asegura la
        // superficie), luego mpv. Recién después se suelta `wakeup`.
        unsafe {
            if !self.render.is_null() {
                (self.api.render_context_free)(self.render);
            }
            (self.api.terminate_destroy)(self.handle);
        }
    }
}
