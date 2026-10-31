//! Enlaces mínimos a libmpv, escritos a mano a partir de `client.h`, `render.h` y
//! `render_gl.h` (licencia ISC). Solo lo que usa `Player`. libmpv se carga una vez, al
//! primer uso; si no está, `api()` devuelve error y el resto del programa sigue.

use std::ffi::{c_char, c_double, c_int, c_void};
use std::sync::OnceLock;

/// `mpv_handle` y `mpv_render_context` son opacos.
#[repr(C)]
pub struct Handle {
    _p: [u8; 0],
}
#[repr(C)]
pub struct RenderContext {
    _p: [u8; 0],
}

/// `mpv_event` (solo lo que se lee).
#[repr(C)]
pub struct Event {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

/// Comienzo de `mpv_event_end_file`.
#[repr(C)]
pub struct EventEndFile {
    pub reason: c_int,
    pub error: c_int,
}

/// `mpv_event_log_message`.
#[repr(C)]
pub struct EventLogMessage {
    pub prefix: *const c_char,
    pub level: *const c_char,
    pub text: *const c_char,
    pub log_level: c_int,
}

#[repr(C)]
pub struct RenderParam {
    pub kind: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct OpenGlInitParams {
    pub get_proc_address: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
pub struct OpenGlFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

pub const EVENT_NONE: c_int = 0;
pub const EVENT_SHUTDOWN: c_int = 1;
pub const EVENT_LOG_MESSAGE: c_int = 2;
pub const EVENT_END_FILE: c_int = 7;
pub const EVENT_FILE_LOADED: c_int = 8;

pub const RENDER_PARAM_INVALID: c_int = 0;
pub const RENDER_PARAM_API_TYPE: c_int = 1;
pub const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub const RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub const RENDER_PARAM_FLIP_Y: c_int = 4;
pub const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
pub const RENDER_API_TYPE_OPENGL: &[u8] = b"opengl\0";
pub const RENDER_UPDATE_FRAME: u64 = 1;

type Callback = unsafe extern "C" fn(*mut c_void);

/// Funciones de libmpv. La biblioteca se queda cargada mientras viva el programa.
pub struct Api {
    _lib: libloading::Library,
    pub create: unsafe extern "C" fn() -> *mut Handle,
    pub initialize: unsafe extern "C" fn(*mut Handle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(*mut Handle),
    pub set_option_string: unsafe extern "C" fn(*mut Handle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut Handle, *mut *const c_char) -> c_int,
    pub wait_event: unsafe extern "C" fn(*mut Handle, c_double) -> *mut Event,
    pub request_log_messages: unsafe extern "C" fn(*mut Handle, *const c_char) -> c_int,
    pub set_wakeup_callback: unsafe extern "C" fn(*mut Handle, Option<Callback>, *mut c_void),
    pub render_context_create:
        unsafe extern "C" fn(*mut *mut RenderContext, *mut Handle, *mut RenderParam) -> c_int,
    pub render_context_set_update_callback:
        unsafe extern "C" fn(*mut RenderContext, Option<Callback>, *mut c_void),
    pub render_context_update: unsafe extern "C" fn(*mut RenderContext) -> u64,
    pub render_context_render: unsafe extern "C" fn(*mut RenderContext, *mut RenderParam) -> c_int,
    pub render_context_report_swap: unsafe extern "C" fn(*mut RenderContext),
    pub render_context_free: unsafe extern "C" fn(*mut RenderContext),
}

#[cfg(target_os = "linux")]
const NAMES: &[&str] = &["libmpv.so.2", "libmpv.so"];
#[cfg(windows)]
const NAMES: &[&str] = &["libmpv-2.dll", "mpv-2.dll"];
#[cfg(not(any(target_os = "linux", windows)))]
const NAMES: &[&str] = &["libmpv.2.dylib", "libmpv.dylib"];

static API: OnceLock<Option<Api>> = OnceLock::new();

/// libmpv cargada, o `None` si el sistema no la tiene (o le falta alguna función).
pub fn api() -> Option<&'static Api> {
    API.get_or_init(load).as_ref()
}

fn load() -> Option<Api> {
    let lib = NAMES
        .iter()
        // SAFETY: cargar libmpv corre sus constructores, que no tienen efectos fuera de la
        // propia biblioteca.
        .find_map(|n| unsafe { libloading::Library::new(n) }.ok())?;
    macro_rules! sym {
        ($name:literal) => {
            // SAFETY: el tipo es la firma de `client.h`/`render.h` para ese nombre.
            *unsafe { lib.get(concat!($name, "\0").as_bytes()) }.ok()?
        };
    }
    Some(Api {
        create: sym!("mpv_create"),
        initialize: sym!("mpv_initialize"),
        terminate_destroy: sym!("mpv_terminate_destroy"),
        set_option_string: sym!("mpv_set_option_string"),
        command: sym!("mpv_command"),
        wait_event: sym!("mpv_wait_event"),
        request_log_messages: sym!("mpv_request_log_messages"),
        set_wakeup_callback: sym!("mpv_set_wakeup_callback"),
        render_context_create: sym!("mpv_render_context_create"),
        render_context_set_update_callback: sym!("mpv_render_context_set_update_callback"),
        render_context_update: sym!("mpv_render_context_update"),
        render_context_render: sym!("mpv_render_context_render"),
        render_context_report_swap: sym!("mpv_render_context_report_swap"),
        render_context_free: sym!("mpv_render_context_free"),
        _lib: lib,
    })
}
