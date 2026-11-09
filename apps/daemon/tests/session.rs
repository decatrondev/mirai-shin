//! La lectura de logind contra un logind falso en un bus de D-Bus propio (no toca el del
//! equipo). Si no está `dbus-daemon`, la prueba no corre.
#![cfg(target_os = "linux")]

use miraishin_daemon::session::{logind, LockState, SessionEvent, SessionSink};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedObjectPath;

const SESSION_PATH: &str = "/org/freedesktop/login1/session/c7";

struct FakeManager;

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl FakeManager {
    fn get_session(&self, id: String) -> zbus::fdo::Result<OwnedObjectPath> {
        if id != "c7" {
            return Err(zbus::fdo::Error::Failed("no such session".into()));
        }
        Ok(OwnedObjectPath::try_from(SESSION_PATH).unwrap())
    }

    #[zbus(property)]
    fn preparing_for_sleep(&self) -> bool {
        false
    }

    #[zbus(signal)]
    async fn prepare_for_sleep(emitter: &SignalEmitter<'_>, start: bool) -> zbus::Result<()>;
}

struct FakeSession {
    locked: bool,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl FakeSession {
    #[zbus(property)]
    fn id(&self) -> String {
        "c7".into()
    }

    #[zbus(property)]
    fn locked_hint(&self) -> bool {
        self.locked
    }
}

struct Bus(Child);

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Un `dbus-daemon` propio; devuelve su dirección.
fn private_bus() -> Option<(Bus, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut line = String::new();
    BufReader::new(child.stdout.take()?)
        .read_line(&mut line)
        .ok()?;
    let address = line.trim().to_string();
    (!address.is_empty()).then(|| (Bus(child), address))
}

fn wait_for(events: &Mutex<Vec<SessionEvent>>, want: SessionEvent) {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if events.lock().unwrap().contains(&want) {
            events.lock().unwrap().clear();
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no llegó {want:?}: {:?}", events.lock().unwrap());
}

#[test]
fn lee_el_bloqueo_y_la_suspension() {
    let Some((_bus, address)) = private_bus() else {
        eprintln!("sin dbus-daemon: prueba omitida");
        return;
    };
    let events: Arc<Mutex<Vec<SessionEvent>>> = Arc::default();
    let keep = events.clone();
    let sink: SessionSink = Arc::new(move |e| keep.lock().unwrap().push(e));

    // Sin logind en el bus: no disponible.
    logind::start(Some(address.clone()), sink.clone());
    wait_for(&events, SessionEvent::Lock(LockState::Unavailable));

    // Con logind (sesión desbloqueada); los avisos llegan por la ruta propia de la sesión.
    let server = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .name("org.freedesktop.login1")
        .unwrap()
        .serve_at("/org/freedesktop/login1", FakeManager)
        .unwrap()
        .serve_at(
            "/org/freedesktop/login1/session/auto",
            FakeSession { locked: false },
        )
        .unwrap()
        .serve_at(SESSION_PATH, FakeSession { locked: false })
        .unwrap()
        .build()
        .unwrap();
    logind::start(Some(address), sink);
    wait_for(&events, SessionEvent::Lock(LockState::Unlocked));

    let session = server
        .object_server()
        .interface::<_, FakeSession>(SESSION_PATH)
        .unwrap();
    let set = |locked: bool| {
        session.get_mut().locked = locked;
        zbus::block_on(session.get().locked_hint_changed(session.signal_emitter())).unwrap();
    };
    set(true);
    wait_for(&events, SessionEvent::Lock(LockState::Locked));
    set(false);
    wait_for(&events, SessionEvent::Lock(LockState::Unlocked));

    let manager = server
        .object_server()
        .interface::<_, FakeManager>("/org/freedesktop/login1")
        .unwrap();
    // El hilo de la suspensión se suscribe un poco después de arrancar: se reintenta.
    let sleep = |start: bool| {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            zbus::block_on(FakeManager::prepare_for_sleep(
                manager.signal_emitter(),
                start,
            ))
            .unwrap();
            std::thread::sleep(Duration::from_millis(100));
            if events.lock().unwrap().contains(&SessionEvent::Sleep(start)) {
                events.lock().unwrap().clear();
                return;
            }
            assert!(Instant::now() < end, "no llegó Sleep({start})");
        }
    };
    sleep(true);
    sleep(false);
}
