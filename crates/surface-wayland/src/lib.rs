//! Superficie de fondo para compositores con `wlr-layer-shell` (Sway, Hyprland).
//! En KDE no se usa: ahí el fondo va por el complemento de Plasma (ver el plan de la fase 1).
//!
//! Una superficie en la capa `background` por monitor, anclada a los cuatro bordes, sin zona
//! exclusiva, sin teclado y con región de entrada vacía (los clics pasan al escritorio). Se
//! dibuja por memoria compartida (`wl_shm`) una vez por imagen; el compositor la conserva.
//! Los videos (fase 2) se dibujan con libmpv sobre EGL, al ritmo de la pantalla, con un solo
//! reproductor para todos los monitores que muestran el mismo video con el mismo encuadre.
//! La conexión con Wayland corre en su propio hilo.

pub mod pixels;

#[cfg(target_os = "linux")]
mod egl;
#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "linux")]
pub use wayland::{ConnectError, OutputWatcher, WaylandSurface};
