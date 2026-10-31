//! Protocolo entre la app (o `ctl`) y el daemon.
//!
//! Una línea JSON por mensaje. Reglas de compatibilidad: se ignoran los campos
//! desconocidos y un método desconocido responde `unknown_method` sin cortar la conexión,
//! así una app nueva habla con un daemon viejo y al revés.

pub mod client;
pub mod protocol;
pub mod server;
pub mod transport;

pub use client::{CallError, Client};
pub use protocol::*;
pub use server::{serve, Handler, HandlerFuture};
pub use transport::{BindError, Endpoint, Listener};
