//! Cuándo se pausa el video de un monitor (fase 3). Reglas puras: las señales y los ajustes
//! entran por parámetro, y sale el motivo de la pausa o nada (se reproduce).
//!
//! Las imágenes no se pausan: después de dibujarse no gastan.

/// Por qué está en pausa un monitor. El código (`as_str`) lo traduce `ctl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauseReason {
    /// La persona lo pausó (`ctl pause`).
    Manual,
    /// La sesión está bloqueada.
    Locked,
    /// La pantalla de ese monitor está apagada.
    ScreenOff,
    /// El equipo funciona con batería.
    Battery,
    /// Hay una app en pantalla completa en ese monitor.
    Fullscreen,
    /// Hay una ventana maximizada en ese monitor (apagado por defecto).
    Maximized,
}

impl PauseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
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
    /// Decidido con Anthony el 2026-11-01: se pausa en los cuatro casos; con una ventana
    /// maximizada no (el fondo todavía se ve en parte por los bordes y la barra).
    fn default() -> Self {
        Self {
            on_fullscreen: true,
            on_maximized: false,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        // Maximizada: apagado por defecto.
        assert_eq!(decide(&s, &sys, &out(|o| o.maximized = true)), None);
        let con_max = PauseSettings {
            on_maximized: true,
            ..s
        };
        assert_eq!(
            decide(&con_max, &sys, &out(|o| o.maximized = true)),
            Some(PauseReason::Maximized)
        );
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
    }

    #[test]
    fn con_varios_motivos_va_el_primero() {
        let s = PauseSettings::default();
        let sys = SystemSignals {
            locked: true,
            on_battery: true,
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
