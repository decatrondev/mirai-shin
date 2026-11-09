//! Avisos de Windows para la pausa (fase 3.6): batería y bloqueo de la sesión. Cada vigilante
//! tiene su hilo con una ventana oculta (Windows avisa estas cosas con mensajes a una
//! ventana). No preguntan nada a cada rato: esperan los avisos.
//!
//! - Energía: `RegisterPowerSettingNotification` con `GUID_ACDC_POWER_SOURCE` (cargador o
//!   batería) y `GUID_POWER_SAVING_STATUS` (ahorro de batería, cuenta como batería). Windows
//!   manda el valor actual apenas se registra.
//! - Bloqueo: `WTSRegisterSessionNotification` → `WM_WTSSESSION_CHANGE` con
//!   `WTS_SESSION_LOCK` / `WTS_SESSION_UNLOCK`. Al empezar se da por desbloqueada (el daemon
//!   arranca con la sesión abierta).
//!
//! La pantalla apagada la avisa la superficie (`GUID_SESSION_DISPLAY_STATUS`), porque es por
//! monitor en el contrato.

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
use windows_sys::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows_sys::Win32::System::SystemServices::{
    GUID_ACDC_POWER_SOURCE, GUID_POWER_SAVING_STATUS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
    TranslateMessage, DEVICE_NOTIFY_WINDOW_HANDLE, MSG, PBT_POWERSETTINGCHANGE, WM_POWERBROADCAST,
    WM_WTSSESSION_CHANGE, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP, WTS_SESSION_LOCK,
    WTS_SESSION_UNLOCK,
};

use crate::layout::on_battery;

/// Recibe `true` con batería (o ahorro de batería) / con la sesión bloqueada.
pub type Callback = Box<dyn Fn(bool) + Send>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Power,
    Lock,
}

thread_local! {
    static CALLBACK: RefCell<Option<Callback>> = const { RefCell::new(None) };
    /// Último `GUID_ACDC_POWER_SOURCE` y si el ahorro de batería está encendido.
    static AC_SOURCE: Cell<u32> = const { Cell::new(0) };
    static SAVER: Cell<bool> = const { Cell::new(false) };
}

/// Vigila cargador / batería. `false` si no se pudo arrancar (queda «no disponible»).
pub fn watch_power(callback: Callback) -> bool {
    start(Kind::Power, callback)
}

/// Vigila el bloqueo de la sesión. `false` si no se pudo arrancar.
pub fn watch_lock(callback: Callback) -> bool {
    start(Kind::Lock, callback)
}

fn start(kind: Kind, callback: Callback) -> bool {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let name = match kind {
        Kind::Power => "signals-power",
        Kind::Lock => "signals-lock",
    };
    let spawned = std::thread::Builder::new()
        .name(name.into())
        // SAFETY: todo lo de Windows se hace en este hilo, dueño de su ventana.
        .spawn(move || unsafe { run(kind, callback, ready_tx) });
    spawned.is_ok() && ready_rx.recv().unwrap_or(false)
}

unsafe fn run(kind: Kind, callback: Callback, ready: std::sync::mpsc::Sender<bool>) {
    let hinst = GetModuleHandleW(null());
    let class = wide(&format!("{}-signals", miraishin_brand::get().slug));
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(signals_proc),
        hInstance: hinst,
        lpszClassName: class.as_ptr(),
        ..std::mem::zeroed()
    };
    // La segunda vez falla porque ya existe (la usan los dos vigilantes): no importa.
    RegisterClassExW(&wc);
    // Ventana oculta de primer nivel (no solo de mensajes: esas no reciben todos los avisos).
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class.as_ptr(),
        null(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        hinst,
        null(),
    );
    if hwnd.is_null() {
        tracing::warn!(error = GetLastError(), "signals window failed");
        let _ = ready.send(false);
        return;
    }
    CALLBACK.with_borrow_mut(|c| *c = Some(callback));
    let ok = match kind {
        Kind::Power => {
            let a = RegisterPowerSettingNotification(
                hwnd,
                &GUID_ACDC_POWER_SOURCE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
            let b = RegisterPowerSettingNotification(
                hwnd,
                &GUID_POWER_SAVING_STATUS,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            );
            a != 0 || b != 0
        }
        Kind::Lock => {
            let ok = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) != 0;
            if ok {
                notify(false);
            }
            ok
        }
    };
    if !ok {
        tracing::warn!(
            error = GetLastError(),
            "windows did not take the notification"
        );
        let _ = ready.send(false);
        return;
    }
    let _ = ready.send(true);
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

fn notify(on: bool) {
    CALLBACK.with_borrow(|c| {
        if let Some(cb) = c {
            cb(on);
        }
    });
}

unsafe extern "system" fn signals_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_POWERBROADCAST if wp as u32 == PBT_POWERSETTINGCHANGE && lp != 0 => {
            let setting = &*(lp as *const POWERBROADCAST_SETTING);
            if setting.DataLength >= 4 {
                let value = std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>());
                if guid_eq(&setting.PowerSetting, &GUID_ACDC_POWER_SOURCE) {
                    AC_SOURCE.set(value);
                } else if guid_eq(&setting.PowerSetting, &GUID_POWER_SAVING_STATUS) {
                    SAVER.set(value != 0);
                }
                notify(on_battery(AC_SOURCE.get(), SAVER.get()));
            }
            1
        }
        WM_WTSSESSION_CHANGE => {
            match wp as u32 {
                WTS_SESSION_LOCK => notify(true),
                WTS_SESSION_UNLOCK => notify(false),
                _ => {}
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub(crate) fn guid_eq(a: &windows_sys::core::GUID, b: &windows_sys::core::GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
