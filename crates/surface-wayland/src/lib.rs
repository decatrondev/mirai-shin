//! Superficie de fondo para compositores con `wlr-layer-shell` (Sway, Hyprland).
//! En KDE no se usa: ahí el fondo va por el complemento de Plasma (ver el plan de la fase 1).
//!
//! Una superficie en la capa `background` por monitor, anclada a los cuatro bordes, sin zona
//! exclusiva, sin teclado y con región de entrada vacía (los clics pasan al escritorio). Se
//! dibuja por memoria compartida (`wl_shm`) una vez por imagen; el compositor la conserva.
//! La conexión con Wayland corre en su propio hilo.

pub mod pixels;

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "linux")]
pub use wayland::{ConnectError, WaylandSurface};
