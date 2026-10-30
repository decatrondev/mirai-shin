//! Límite de intentos de login fallidos, en memoria. Reglas puras: el reloj
//! entra por parámetro para poder probarlas sin esperar.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub const MAX_FAILURES: usize = 10;
pub const WINDOW: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct LoginLimiter {
    failures: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginLimiter {
    /// `true` si alguna de las claves (IP, correo) llegó al tope en la ventana.
    pub fn is_blocked(&self, keys: &[&str], now: Instant) -> bool {
        let mut map = self.failures.lock().unwrap();
        keys.iter().any(|k| {
            let list = map.entry((*k).to_string()).or_default();
            list.retain(|t| now.duration_since(*t) < WINDOW);
            list.len() >= MAX_FAILURES
        })
    }

    pub fn record_failure(&self, keys: &[&str], now: Instant) {
        let mut map = self.failures.lock().unwrap();
        for k in keys {
            map.entry((*k).to_string()).or_default().push(now);
        }
        // Que el mapa no crezca sin límite con IPs que no vuelven.
        if map.len() > 10_000 {
            map.retain(|_, v| v.iter().any(|t| now.duration_since(*t) < WINDOW));
        }
    }

    pub fn clear(&self, keys: &[&str]) {
        let mut map = self.failures.lock().unwrap();
        for k in keys {
            map.remove(*k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloquea_al_llegar_al_tope_y_libera_al_pasar_la_ventana() {
        let l = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES - 1 {
            l.record_failure(&["ip"], t0);
        }
        assert!(!l.is_blocked(&["ip"], t0));
        l.record_failure(&["ip"], t0);
        assert!(l.is_blocked(&["ip"], t0));
        assert!(!l.is_blocked(&["otra-ip"], t0));
        assert!(!l.is_blocked(&["ip"], t0 + WINDOW + Duration::from_secs(1)));
    }

    #[test]
    fn un_login_correcto_limpia_los_fallos() {
        let l = LoginLimiter::default();
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES {
            l.record_failure(&["ip", "correo"], t0);
        }
        assert!(l.is_blocked(&["correo"], t0));
        l.clear(&["ip", "correo"]);
        assert!(!l.is_blocked(&["ip", "correo"], t0));
    }
}
