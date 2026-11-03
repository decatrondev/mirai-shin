//! De dónde saca la energía el equipo (fase 3.2): con cargador o con batería.
//!
//! En Linux lo dice UPower por el bus de sistema (`OnBattery`, avisa sus cambios con
//! `PropertiesChanged`). En el Flatpak hace falta `--system-talk-name=org.freedesktop.UPower`.
//! Si no hay UPower o falta el permiso, no es un error: el estado queda «no disponible» y
//! el video no se pausa por batería. En Windows llega en el 3.6.

use std::sync::Arc;

/// Lo que se sabe de la energía del equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerState {
    /// No hay quién lo diga (sin UPower, sin permiso, o este sistema todavía no lo lee).
    #[default]
    Unavailable,
    Ac,
    Battery,
}

impl PowerState {
    /// Código para `status` (lo traduce `ctl`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Ac => "ac",
            Self::Battery => "battery",
        }
    }

    pub fn from_on_battery(on_battery: bool) -> Self {
        if on_battery {
            Self::Battery
        } else {
            Self::Ac
        }
    }
}

/// Recibe cada estado nuevo (puede repetirse: quien lo recibe descarta lo igual).
pub type PowerSink = Arc<dyn Fn(PowerState) + Send + Sync>;

/// Arranca lo que vigila la energía y le pasa los cambios a `sink`. En pruebas se pasa una
/// propia que guarda el `sink` para mandarle estados a mano.
pub type PowerFactory = Box<dyn FnOnce(PowerSink) + Send>;

/// La de este equipo: UPower en Linux, nada en los demás (queda «no disponible»).
pub fn system() -> PowerFactory {
    Box::new(|sink| {
        #[cfg(target_os = "linux")]
        upower::start(None, sink);
        #[cfg(not(target_os = "linux"))]
        sink(PowerState::Unavailable);
    })
}

/// Ninguna: para pruebas que no tienen que ver con la energía.
pub fn none() -> PowerFactory {
    Box::new(|sink| sink(PowerState::Unavailable))
}

#[cfg(target_os = "linux")]
pub mod upower {
    use super::{PowerSink, PowerState};
    use std::time::Duration;
    use zbus::blocking::{connection, Connection, Proxy};

    const NAME: &str = "org.freedesktop.UPower";
    const PATH: &str = "/org/freedesktop/UPower";
    /// Cada cuánto se vuelve a intentar si no hay UPower o se cortó la conexión.
    const RETRY: Duration = Duration::from_secs(60);

    /// Vigila UPower en un hilo propio. `address` es el bus a usar (`None` = el de sistema;
    /// las pruebas pasan uno propio con un UPower falso).
    pub fn start(address: Option<String>, sink: PowerSink) {
        let started = std::thread::Builder::new()
            .name("power".into())
            .spawn(move || {
                let mut logged = false;
                loop {
                    if let Err(e) = watch(address.as_deref(), &sink) {
                        // Solo la primera vez: sin permiso fallaría igual cada minuto.
                        if !logged {
                            tracing::info!(error = %e, "power source not available (UPower)");
                            logged = true;
                        }
                    }
                    sink(PowerState::Unavailable);
                    std::thread::sleep(RETRY);
                }
            });
        if started.is_err() {
            tracing::warn!("could not start the power thread");
        }
    }

    /// Lee `OnBattery` y se queda esperando sus cambios. Vuelve solo si algo falla.
    fn watch(address: Option<&str>, sink: &PowerSink) -> zbus::Result<()> {
        let conn = match address {
            Some(a) => connection::Builder::address(a)?.build()?,
            None => Connection::system()?,
        };
        let proxy = Proxy::new(&conn, NAME, PATH, NAME)?;
        // Primero el aviso de cambios, después el valor: así no se pierde un cambio en medio.
        let changes = proxy.receive_property_changed::<bool>("OnBattery");
        let on: bool = proxy.get_property("OnBattery")?;
        sink(PowerState::from_on_battery(on));
        for change in changes {
            sink(PowerState::from_on_battery(change.get()?));
        }
        Err(zbus::Error::Failure(
            "UPower stopped sending changes".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos() {
        assert_eq!(PowerState::from_on_battery(true).as_str(), "battery");
        assert_eq!(PowerState::from_on_battery(false).as_str(), "ac");
        assert_eq!(PowerState::default().as_str(), "unavailable");
    }
}
