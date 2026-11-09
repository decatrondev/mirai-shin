//! Sesión bloqueada y suspensión del equipo (fase 3.4).
//!
//! En Linux lo dice logind por el bus de sistema: `Session.LockedHint` (avisa sus cambios con
//! `PropertiesChanged`) y la señal `Manager.PrepareForSleep(b)`. En el Flatpak hace falta
//! `--system-talk-name=org.freedesktop.login1`. KDE y GNOME ponen `LockedHint` al bloquear;
//! swaylock no lo hace solo, así que en Sway el bloqueo puede no llegar. Si no hay logind o
//! falta el permiso, el bloqueo queda «no disponible» y el video no se pausa por eso.

use std::sync::Arc;

/// Lo que se sabe del bloqueo de la sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LockState {
    /// No hay quién lo diga (sin logind, sin permiso, o este sistema todavía no lo lee).
    #[default]
    Unavailable,
    Unlocked,
    Locked,
}

impl LockState {
    /// Código para `status` (lo traduce `ctl`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unlocked => "unlocked",
            Self::Locked => "locked",
        }
    }

    pub fn from_locked(locked: bool) -> Self {
        if locked {
            Self::Locked
        } else {
            Self::Unlocked
        }
    }
}

/// Un cambio en la sesión (pueden repetirse: quien lo recibe descarta lo igual).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Lock(LockState),
    /// `true`: el equipo se va a suspender; `false`: despertó.
    Sleep(bool),
}

pub type SessionSink = Arc<dyn Fn(SessionEvent) + Send + Sync>;

/// Arranca lo que vigila la sesión y le pasa los cambios a `sink`. En pruebas se pasa una
/// propia que guarda el `sink` para mandarle cambios a mano.
pub type SessionFactory = Box<dyn FnOnce(SessionSink) + Send>;

/// La de este equipo: logind en Linux, nada en los demás (queda «no disponible»).
pub fn system() -> SessionFactory {
    Box::new(|sink| {
        #[cfg(target_os = "linux")]
        logind::start(None, sink);
        #[cfg(not(target_os = "linux"))]
        sink(SessionEvent::Lock(LockState::Unavailable));
    })
}

/// Ninguna: para pruebas que no tienen que ver con la sesión.
pub fn none() -> SessionFactory {
    Box::new(|sink| sink(SessionEvent::Lock(LockState::Unavailable)))
}

#[cfg(target_os = "linux")]
pub mod logind {
    use super::{LockState, SessionEvent, SessionSink};
    use std::time::Duration;
    use zbus::blocking::{connection, Connection, Proxy};
    use zbus::zvariant::OwnedObjectPath;

    const NAME: &str = "org.freedesktop.login1";
    const PATH: &str = "/org/freedesktop/login1";
    const MANAGER: &str = "org.freedesktop.login1.Manager";
    const SESSION: &str = "org.freedesktop.login1.Session";
    /// La sesión de quien pregunta. Sirve para leer, pero los avisos de cambios llegan por la
    /// ruta propia de la sesión: se pide con su `Id`.
    const AUTO: &str = "/org/freedesktop/login1/session/auto";
    /// Cada cuánto se vuelve a intentar si no hay logind o se cortó la conexión.
    const RETRY: Duration = Duration::from_secs(60);

    /// Vigila logind en dos hilos propios (bloqueo y suspensión). `address` es el bus a usar
    /// (`None` = el de sistema; las pruebas pasan uno propio con un logind falso).
    pub fn start(address: Option<String>, sink: SessionSink) {
        spawn("lock", address.clone(), sink.clone(), watch_lock, |sink| {
            sink(SessionEvent::Lock(LockState::Unavailable))
        });
        // Si se corta, se da por despierto: mejor reproducir que quedar en pausa sin motivo.
        spawn("sleep", address, sink, watch_sleep, |sink| {
            sink(SessionEvent::Sleep(false))
        });
    }

    fn spawn(
        what: &'static str,
        address: Option<String>,
        sink: SessionSink,
        watch: fn(Option<&str>, &SessionSink) -> zbus::Result<()>,
        on_fail: fn(&SessionSink),
    ) {
        let started = std::thread::Builder::new()
            .name(format!("session-{what}"))
            .spawn(move || {
                let mut logged = false;
                loop {
                    if let Err(e) = watch(address.as_deref(), &sink) {
                        // Solo la primera vez: sin permiso fallaría igual cada minuto.
                        if !logged {
                            tracing::info!(error = %e, what, "session state not available (logind)");
                            logged = true;
                        }
                    }
                    on_fail(&sink);
                    std::thread::sleep(RETRY);
                }
            });
        if started.is_err() {
            tracing::warn!(what, "could not start the session thread");
        }
    }

    fn connect(address: Option<&str>) -> zbus::Result<Connection> {
        match address {
            Some(a) => connection::Builder::address(a)?.build(),
            None => Connection::system(),
        }
    }

    /// Lee `LockedHint` de esta sesión y se queda esperando sus cambios.
    fn watch_lock(address: Option<&str>, sink: &SessionSink) -> zbus::Result<()> {
        let conn = connect(address)?;
        let auto = Proxy::new(&conn, NAME, AUTO, SESSION)?;
        let id: String = auto.get_property("Id")?;
        let manager = Proxy::new(&conn, NAME, PATH, MANAGER)?;
        let path: OwnedObjectPath = manager.call("GetSession", &(id,))?;
        let session = Proxy::new(&conn, NAME, path, SESSION)?;
        // Primero el aviso de cambios, después el valor: así no se pierde un cambio en medio.
        let changes = session.receive_property_changed::<bool>("LockedHint");
        let locked: bool = session.get_property("LockedHint")?;
        sink(SessionEvent::Lock(LockState::from_locked(locked)));
        for change in changes {
            sink(SessionEvent::Lock(LockState::from_locked(change.get()?)));
        }
        Err(zbus::Error::Failure(
            "logind stopped sending changes".into(),
        ))
    }

    /// Espera `PrepareForSleep`: `true` antes de suspender, `false` al despertar.
    fn watch_sleep(address: Option<&str>, sink: &SessionSink) -> zbus::Result<()> {
        let conn = connect(address)?;
        let manager = Proxy::new(&conn, NAME, PATH, MANAGER)?;
        let signals = manager.receive_signal("PrepareForSleep")?;
        // Comprueba que logind está: sin esto, un bus sin logind esperaría para siempre.
        let _: bool = manager.get_property("PreparingForSleep")?;
        for msg in signals {
            let (start,): (bool,) = msg.body().deserialize()?;
            sink(SessionEvent::Sleep(start));
        }
        Err(zbus::Error::Failure(
            "logind stopped sending signals".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos() {
        assert_eq!(LockState::from_locked(true).as_str(), "locked");
        assert_eq!(LockState::from_locked(false).as_str(), "unlocked");
        assert_eq!(LockState::default().as_str(), "unavailable");
    }
}
