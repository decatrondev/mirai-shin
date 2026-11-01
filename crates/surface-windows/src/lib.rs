//! Superficie de fondo para Windows (fase 1.8).
//!
//! Una ventana por monitor colgada del escritorio, debajo de los íconos. Windows tiene dos
//! formas del escritorio y se detecta cuál hay (no se supone por versión):
//! - **Clásica** (Windows 10, 11 hasta 23H2): `0x052C` a `Progman` crea una `WorkerW` detrás
//!   de los íconos y la ventana se pone como hija de ella.
//! - **Elevada** (Windows 11 24H2 en adelante): la ventana es hija de `Progman`, con estilo
//!   layered, debajo de los íconos (`SHELLDLL_DefView`) y encima de la `WorkerW` del fondo.
//!   El estilo layered se pone con la ventana ya hija y visible (probado en 25H2).
//!
//! Las imágenes se dibujan con GDI en `WM_PAINT`; el cuadro se guarda una vez. Los videos
//! (fase 2.8) los dibuja mpv con Direct3D 11 en una ventana suya dentro de la nuestra.
//! El trabajo con ventanas corre en un hilo propio con su bucle de mensajes.

pub mod layout;

#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::{ConnectError, WindowsSurface};
