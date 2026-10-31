//! Fondo en KDE Plasma (fases 1.1c y 1.1d).
//!
//! En KDE una superficie de layer-shell tapa los íconos (prueba del 2026-10-30), así que el
//! fondo lo dibuja Plasma con un complemento de fondo propio (`packaging/kde-wallpaper`):
//! - El daemon instala el complemento en la carpeta de fondos de Plasma del usuario.
//! - Por cada monitor escribe el cuadro como PNG y le pasa la ruta al escritorio de esa
//!   pantalla por D-Bus (`org.kde.PlasmaShell.evaluateScript`). Plasma avisa al complemento
//!   del cambio: nadie revisa nada a cada rato.
//! - Guarda el tipo de fondo que había antes en cada pantalla, para devolverlo al quitar el
//!   nuestro.
//! - Los monitores se conocen por Wayland (`OutputWatcher`), igual que en Sway.

pub mod package;
pub mod script;

#[cfg(target_os = "linux")]
mod plasma;

#[cfg(target_os = "linux")]
pub use plasma::PlasmaSurface;
