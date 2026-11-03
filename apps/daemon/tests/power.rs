//! La lectura de UPower contra un UPower falso en un bus de D-Bus propio (no toca el del
//! equipo). Si no está `dbus-daemon`, la prueba no corre.
#![cfg(target_os = "linux")]

use miraishin_daemon::power::{upower, PowerSink, PowerState};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct FakeUPower {
    on_battery: bool,
}

#[zbus::interface(name = "org.freedesktop.UPower")]
impl FakeUPower {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.on_battery
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

fn wait_for(states: &Mutex<Vec<PowerState>>, want: PowerState) {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if states.lock().unwrap().last() == Some(&want) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no llegó {want:?}: {:?}", states.lock().unwrap());
}

#[test]
fn lee_la_bateria_y_sus_cambios() {
    let Some((_bus, address)) = private_bus() else {
        eprintln!("sin dbus-daemon: prueba omitida");
        return;
    };
    let states: Arc<Mutex<Vec<PowerState>>> = Arc::default();
    let keep = states.clone();
    let sink: PowerSink = Arc::new(move |s| keep.lock().unwrap().push(s));

    // Sin UPower en el bus: no disponible.
    upower::start(Some(address.clone()), sink.clone());
    wait_for(&states, PowerState::Unavailable);

    // Con UPower (empieza con cargador) y después a batería y de vuelta.
    let server = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .name("org.freedesktop.UPower")
        .unwrap()
        .serve_at("/org/freedesktop/UPower", FakeUPower { on_battery: false })
        .unwrap()
        .build()
        .unwrap();
    upower::start(Some(address), sink);
    wait_for(&states, PowerState::Ac);

    let iface = server
        .object_server()
        .interface::<_, FakeUPower>("/org/freedesktop/UPower")
        .unwrap();
    let set = |on: bool| {
        iface.get_mut().on_battery = on;
        zbus::block_on(iface.get().on_battery_changed(iface.signal_emitter())).unwrap();
    };
    set(true);
    wait_for(&states, PowerState::Battery);
    set(false);
    wait_for(&states, PowerState::Ac);
}
