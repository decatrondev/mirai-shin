//! Video con libmpv (fase 2.1).
//!
//! - Enlaces propios a la API de mpv (`client.h`, `render.h`, `render_gl.h`, licencia ISC).
//!   libmpv **se carga al arrancar** (`dlopen`): si el sistema no la tiene, el daemon sigue
//!   funcionando con imágenes y responde `mpv_unavailable` solo a los videos.
//! - libmpv debe ser una compilación LGPL (`-Dgpl=false`); la del Flatpak lo es.
//! - Un `Player` reproduce un archivo en bucle, sin sonido, con el encuadre pedido, y dibuja
//!   con OpenGL en el contexto que le dé la superficie (EGL en Wayland).
//!
//! Las reglas puras (opciones de mpv por modo, códigos de error) están en `options`.

mod ffi;
pub mod options;
mod player;

pub use player::{Player, Polled, VideoError};
