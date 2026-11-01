//! Prueba mínima de la fase 2.8: un video con libmpv en una ventana colgada del escritorio
//! elevado de Windows 11 (24H2+), hija layered de `Progman` debajo de los íconos, igual que
//! la de `surface-windows`. Sirve para saber cuál de los dos caminos se ve:
//! - `a`: contexto OpenGL propio (WGL) en la ventana y la API de render de mpv (como en Sway).
//! - `b`: mpv dibuja solo en la ventana (`wid`, salida `gpu`, Direct3D 11).
//!
//! Herramienta interna: no se distribuye. `libmpv-2.dll` va junto al exe.
//!
//! Uso: miraishin-win-video-probe a|b <video> [segundos]

#[cfg(windows)]
mod probe;

#[cfg(windows)]
fn main() {
    std::process::exit(probe::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Esta prueba es solo para Windows.");
}
