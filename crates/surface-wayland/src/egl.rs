//! EGL para el video (fase 2.1): un contexto OpenGL por reproductor y una superficie EGL por
//! monitor. libEGL se carga al primer video; sin ella (o sin GPU que la soporte) el video
//! responde `video_no_gpu` y las imágenes siguen por memoria compartida.

use std::ffi::{c_char, c_void, CStr};
use std::sync::OnceLock;

use khronos_egl as egl;

type Instance = egl::DynamicInstance<egl::EGL1_4>;

static INSTANCE: OnceLock<Option<Instance>> = OnceLock::new();

fn instance() -> Option<&'static Instance> {
    INSTANCE
        .get_or_init(|| {
            // SAFETY: cargar libEGL corre sus constructores; no tienen efectos fuera de ella.
            unsafe { Instance::load_required() }.ok()
        })
        .as_ref()
}

/// `get_proc_address` para libmpv.
pub unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    let Some(egl) = instance() else {
        return std::ptr::null_mut();
    };
    // SAFETY: libmpv pasa un nombre terminado en 0.
    let Ok(name) = unsafe { CStr::from_ptr(name) }.to_str() else {
        return std::ptr::null_mut();
    };
    egl.get_proc_address(name)
        .map_or(std::ptr::null_mut(), |f| f as *mut c_void)
}

pub struct Egl {
    egl: &'static Instance,
    display: egl::Display,
    config: egl::Config,
}

impl Egl {
    /// EGL sobre la conexión de Wayland. `wl_display`: puntero de libwayland.
    pub fn new(wl_display: *mut c_void) -> Option<Self> {
        let egl = instance()?;
        // SAFETY: `wl_display` es la conexión viva de este hilo.
        let display = unsafe { egl.get_display(wl_display as egl::NativeDisplayType) }?;
        egl.initialize(display).ok()?;
        egl.bind_api(egl::OPENGL_API).ok()?;
        let attrs = [
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::NONE,
        ];
        // Opaca (sin alfa) si se puede: el fondo no deja ver nada detrás y el compositor no
        // tiene que mezclarlo.
        let mut configs = Vec::with_capacity(64);
        egl.choose_config(display, &attrs, &mut configs).ok()?;
        let alpha = |c: &egl::Config| {
            egl.get_config_attrib(display, *c, egl::ALPHA_SIZE)
                .unwrap_or(8)
        };
        let config = configs
            .iter()
            .find(|c| alpha(c) == 0)
            .or(configs.first())
            .copied()?;
        Some(Self {
            egl,
            display,
            config,
        })
    }

    pub fn create_context(&self) -> Option<egl::Context> {
        self.egl
            .create_context(self.display, self.config, None, &[egl::NONE])
            .ok()
    }

    pub fn destroy_context(&self, ctx: egl::Context) {
        let _ = self.egl.destroy_context(self.display, ctx);
    }

    /// Superficie EGL sobre un `wl_egl_window`.
    pub fn create_surface(&self, window: *const c_void) -> Option<egl::Surface> {
        // SAFETY: `window` es un `wl_egl_window` vivo mientras viva la superficie.
        unsafe {
            self.egl.create_window_surface(
                self.display,
                self.config,
                window as egl::NativeWindowType,
                None,
            )
        }
        .ok()
    }

    pub fn destroy_surface(&self, surface: egl::Surface) {
        // Si es la actual, se suelta primero.
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_surface(self.display, surface);
    }

    /// Activa `ctx` dibujando en `surface` (o sin superficie, para liberar recursos).
    pub fn make_current(&self, surface: Option<egl::Surface>, ctx: egl::Context) -> bool {
        let ok = self
            .egl
            .make_current(self.display, surface, surface, Some(ctx))
            .is_ok();
        if ok && surface.is_some() {
            // Sin esperar al refresco: el ritmo lo llevan los avisos de cuadro del compositor.
            let _ = self.egl.swap_interval(self.display, 0);
        }
        ok
    }

    pub fn swap(&self, surface: egl::Surface) -> bool {
        self.egl.swap_buffers(self.display, surface).is_ok()
    }
}

impl Drop for Egl {
    fn drop(&mut self) {
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.terminate(self.display);
    }
}
