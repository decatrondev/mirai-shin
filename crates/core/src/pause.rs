//! Cuándo se pausa el video de un monitor (fase 3). Reglas puras: las señales y los ajustes
//! entran por parámetro, y sale el motivo de la pausa o nada (se reproduce).
//!
//! Las imágenes no se pausan: después de dibujarse no gastan.

/// Por qué está en pausa un monitor. El código (`as_str`) lo traduce `ctl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauseReason {
    /// La persona lo pausó (`ctl pause`).
    Manual,
    /// El equipo se va a suspender (fase 3.4). Siempre vale, no tiene ajuste.
    Sleep,
    /// La sesión está bloqueada.
    Locked,
    /// La pantalla de ese monitor está apagada.
    ScreenOff,
    /// El equipo funciona con batería.
    Battery,
    /// Hay una app en pantalla completa en ese monitor.
    Fullscreen,
    /// Hay una ventana maximizada en ese monitor.
    Maximized,
}

impl PauseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Sleep => "sleep",
            Self::Locked => "locked",
            Self::ScreenOff => "screen_off",
            Self::Battery => "battery",
            Self::Fullscreen => "fullscreen",
            Self::Maximized => "maximized",
        }
    }
}

/// Ajustes `pause_on_*` de `settings.toml`. La pausa a mano no tiene ajuste: siempre vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PauseSettings {
    pub on_fullscreen: bool,
    pub on_maximized: bool,
    pub on_battery: bool,
    pub on_lock: bool,
    pub on_screen_off: bool,
}

impl Default for PauseSettings {
    /// Decidido con Anthony el 2026-11-05: se pausa en los cinco casos, también con una ventana
    /// maximizada (casi no se ve el fondo). Una ventana normal encima no pausa: el fondo se
    /// sigue viendo y se notaría congelado.
    fn default() -> Self {
        Self {
            on_fullscreen: true,
            on_maximized: true,
            on_battery: true,
            on_lock: true,
            on_screen_off: true,
        }
    }
}

/// Lo que vale para todo el equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemSignals {
    pub locked: bool,
    pub on_battery: bool,
    /// El equipo se va a suspender (logind `PrepareForSleep`, fase 3.4).
    pub asleep: bool,
    /// `ctl pause` sin monitor.
    pub manual_all: bool,
}

/// Lo de un monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputSignals {
    /// `ctl pause --monitor`.
    pub manual: bool,
    pub screen_off: bool,
    pub fullscreen: bool,
    pub maximized: bool,
}

/// Motivo de la pausa de un monitor, o `None` si el video se reproduce. Si hay varios, va el
/// que la persona entiende primero: lo que hizo ella, después lo que no se ve, después el
/// ahorro.
pub fn decide(
    settings: &PauseSettings,
    system: &SystemSignals,
    output: &OutputSignals,
) -> Option<PauseReason> {
    let checks = [
        (system.manual_all || output.manual, PauseReason::Manual),
        (system.asleep, PauseReason::Sleep),
        (settings.on_lock && system.locked, PauseReason::Locked),
        (
            settings.on_screen_off && output.screen_off,
            PauseReason::ScreenOff,
        ),
        (
            settings.on_battery && system.on_battery,
            PauseReason::Battery,
        ),
        (
            settings.on_fullscreen && output.fullscreen,
            PauseReason::Fullscreen,
        ),
        (
            settings.on_maximized && output.maximized,
            PauseReason::Maximized,
        ),
    ];
    checks.into_iter().find(|(on, _)| *on).map(|(_, r)| r)
}

/// Una ventana, como la informa el compositor (Sway/Hyprland, `wlr-foreign-toplevel`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Window {
    /// Monitores donde está (normalmente uno).
    pub outputs: Vec<String>,
    pub fullscreen: bool,
    pub maximized: bool,
    pub minimized: bool,
    /// Es la que tiene el foco.
    pub activated: bool,
}

/// Qué tapa el fondo de un monitor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cover {
    pub fullscreen: bool,
    pub maximized: bool,
}

/// Qué monitores tienen el fondo tapado, con la regla que salió de la prueba 3.0 en Sway:
/// una ventana en pantalla completa (o maximizada) tapa su monitor si **es la activa**, o si
/// la activa está **en otro monitor** (el juego del monitor A sigue tapándolo aunque se haga
/// clic en el B). Así no cuenta una ventana que quedó en un escritorio virtual oculto (Sway
/// la sigue informando en pantalla completa en ese monitor, pero ya no es la activa), y con
/// un escritorio vacío (ninguna activa) no se pausa nada. Las minimizadas no tapan.
/// Devuelve solo los monitores tapados.
pub fn covered(windows: &[Window]) -> std::collections::HashMap<String, Cover> {
    let active: Vec<&String> = windows
        .iter()
        .filter(|w| w.activated)
        .flat_map(|w| &w.outputs)
        .collect();
    let mut out: std::collections::HashMap<String, Cover> = std::collections::HashMap::new();
    for w in windows {
        if w.minimized || !(w.fullscreen || w.maximized) {
            continue;
        }
        for o in &w.outputs {
            let visible = w.activated || (!active.is_empty() && !active.contains(&o));
            if !visible {
                continue;
            }
            let c = out.entry(o.clone()).or_default();
            c.fullscreen |= w.fullscreen;
            c.maximized |= w.maximized;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(output: &str, f: impl FnOnce(&mut Window)) -> Window {
        let mut w = Window {
            outputs: vec![output.into()],
            ..Default::default()
        };
        f(&mut w);
        w
    }

    #[test]
    fn ventanas_que_tapan() {
        let full = |o: &str, active: bool| {
            win(o, |w| {
                w.fullscreen = true;
                w.activated = active;
            })
        };
        // Activa en pantalla completa: tapa su monitor, no el otro.
        let c = covered(&[full("A", true)]);
        assert!(c["A"].fullscreen);
        assert!(!c.contains_key("B"));
        // Clic en una ventana del monitor B: el juego de A sigue tapando A.
        let c = covered(&[full("A", false), win("B", |w| w.activated = true)]);
        assert!(c["A"].fullscreen);
        assert!(!c.contains_key("B"));
        // Otra ventana activa en el mismo monitor: la de pantalla completa quedó en un
        // escritorio oculto (Sway la sigue informando): no tapa.
        let c = covered(&[full("A", false), win("A", |w| w.activated = true)]);
        assert!(c.is_empty());
        // Escritorio vacío: ninguna activa, no se pausa.
        assert!(covered(&[full("A", false)]).is_empty());
        // Minimizada: no tapa.
        let c = covered(&[win("A", |w| {
            w.fullscreen = true;
            w.activated = true;
            w.minimized = true;
        })]);
        assert!(c.is_empty());
        // Maximizada activa.
        let c = covered(&[win("A", |w| {
            w.maximized = true;
            w.activated = true;
        })]);
        assert_eq!(
            c["A"],
            Cover {
                fullscreen: false,
                maximized: true
            }
        );
        // Una normal activa no tapa nada.
        assert!(covered(&[win("A", |w| w.activated = true)]).is_empty());
    }

    fn out(f: impl FnOnce(&mut OutputSignals)) -> OutputSignals {
        let mut o = OutputSignals::default();
        f(&mut o);
        o
    }

    #[test]
    fn sin_senales_se_reproduce() {
        let s = PauseSettings::default();
        assert_eq!(
            decide(&s, &SystemSignals::default(), &OutputSignals::default()),
            None
        );
    }

    #[test]
    fn cada_senal_con_su_ajuste() {
        let s = PauseSettings::default();
        let sys = SystemSignals::default();
        assert_eq!(
            decide(&s, &sys, &out(|o| o.fullscreen = true)),
            Some(PauseReason::Fullscreen)
        );
        assert_eq!(
            decide(&s, &sys, &out(|o| o.screen_off = true)),
            Some(PauseReason::ScreenOff)
        );
        // Maximizada: pausa por defecto, y se puede apagar.
        assert_eq!(
            decide(&s, &sys, &out(|o| o.maximized = true)),
            Some(PauseReason::Maximized)
        );
        let sin_max = PauseSettings {
            on_maximized: false,
            ..s
        };
        assert_eq!(decide(&sin_max, &sys, &out(|o| o.maximized = true)), None);
        let bat = SystemSignals {
            on_battery: true,
            ..sys
        };
        assert_eq!(
            decide(&s, &bat, &OutputSignals::default()),
            Some(PauseReason::Battery)
        );
        let sin_bat = PauseSettings {
            on_battery: false,
            ..s
        };
        assert_eq!(decide(&sin_bat, &bat, &OutputSignals::default()), None);
    }

    #[test]
    fn la_pausa_a_mano_no_se_apaga_con_ajustes() {
        let nada = PauseSettings {
            on_fullscreen: false,
            on_maximized: false,
            on_battery: false,
            on_lock: false,
            on_screen_off: false,
        };
        let all = SystemSignals {
            manual_all: true,
            ..Default::default()
        };
        assert_eq!(
            decide(&nada, &all, &OutputSignals::default()),
            Some(PauseReason::Manual)
        );
        assert_eq!(
            decide(&nada, &SystemSignals::default(), &out(|o| o.manual = true)),
            Some(PauseReason::Manual)
        );
        // Al suspender tampoco hay ajuste que lo apague; la pausa a mano va primero.
        let dormido = SystemSignals {
            asleep: true,
            ..Default::default()
        };
        assert_eq!(
            decide(&nada, &dormido, &OutputSignals::default()),
            Some(PauseReason::Sleep)
        );
        assert_eq!(
            decide(&nada, &dormido, &out(|o| o.manual = true)),
            Some(PauseReason::Manual)
        );
    }

    #[test]
    fn con_varios_motivos_va_el_primero() {
        let s = PauseSettings::default();
        let sys = SystemSignals {
            locked: true,
            on_battery: true,
            asleep: false,
            manual_all: false,
        };
        let o = out(|o| {
            o.fullscreen = true;
            o.screen_off = true;
        });
        assert_eq!(decide(&s, &sys, &o), Some(PauseReason::Locked));
        let o2 = out(|o| o.fullscreen = true);
        assert_eq!(
            decide(
                &s,
                &SystemSignals {
                    locked: false,
                    ..sys
                },
                &o2
            ),
            Some(PauseReason::Battery)
        );
        assert_eq!(
            decide(
                &s,
                &SystemSignals {
                    manual_all: true,
                    ..sys
                },
                &o
            ),
            Some(PauseReason::Manual)
        );
    }

    #[test]
    fn codigos() {
        assert_eq!(PauseReason::ScreenOff.as_str(), "screen_off");
        assert_eq!(PauseReason::Manual.as_str(), "manual");
    }
}
