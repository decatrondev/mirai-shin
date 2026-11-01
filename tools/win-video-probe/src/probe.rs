//! La prueba en sí (solo Windows). Todo corre en el hilo principal, con su bucle de mensajes;
//! mpv avisa desde sus hilos con un mensaje a este hilo (`WM_WAKE`) y nunca se le llama desde
//! otro lado.

use std::ffi::{c_char, c_void, CStr};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::time::Duration;

use miraishin_core::fit::FitMode;
use miraishin_render_video::Player;
use miraishin_surface_windows::layout::{child_rect, form_from_exstyle, DesktopForm, PxRect};
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetDC, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, ValidateRect, HDC,
    MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows_sys::Win32::Graphics::OpenGL::{
    glClear, glClearColor, glGetString, wglCreateContext, wglDeleteContext, wglGetProcAddress,
    wglMakeCurrent, ChoosePixelFormat, SetPixelFormat, SwapBuffers, GL_COLOR_BUFFER_BIT,
    GL_RENDERER, GL_VENDOR, GL_VERSION, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW,
    PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW,
    GetMessageW, GetWindowLongPtrW, KillTimer, PostThreadMessageW, RegisterClassExW,
    SendMessageTimeoutW, SetLayeredWindowAttributes, SetParent, SetTimer, SetWindowLongPtrW,
    SetWindowPos, TranslateMessage, CS_OWNDC, GWL_EXSTYLE, GWL_STYLE, HTTRANSPARENT, LWA_ALPHA,
    MA_NOACTIVATE, MSG, SMTO_ABORTIFHUNG, SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WM_APP, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED,
    WS_POPUP,
};

/// Mensaje que manda mpv (desde sus hilos) para despertar al bucle.
const WM_WAKE: u32 = WM_APP + 1;
/// Mensaje no documentado que le pide a `Progman` preparar el fondo (como en `surface-windows`).
const SPAWN_WORKERW: u32 = 0x052C;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Way {
    /// OpenGL propio + API de render.
    A,
    /// Ventana de mpv (`wid`).
    B,
}

pub fn run() -> i32 {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args: Vec<String> = std::env::args().collect();
    let way = match args.get(1).map(String::as_str) {
        Some("a") | Some("A") => Way::A,
        Some("b") | Some("B") => Way::B,
        _ => return usage(),
    };
    let Some(video) = args.get(2).map(PathBuf::from) else {
        return usage();
    };
    let seconds: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);
    if !video.is_file() {
        eprintln!("No encuentro el video: {}", video.display());
        return 2;
    }
    // SAFETY: llamadas a la API de Windows desde un solo hilo, con valores comprobados.
    unsafe { probe(way, &video, seconds) }
}

fn usage() -> i32 {
    eprintln!("Uso: miraishin-win-video-probe a|b <video> [segundos]");
    2
}

unsafe fn probe(way: Way, video: &std::path::Path, seconds: u32) -> i32 {
    let progman = FindWindowW(wide("Progman").as_ptr(), null());
    if progman.is_null() {
        eprintln!("No hay escritorio de Explorer (Progman no existe).");
        return 3;
    }
    let form = form_from_exstyle(GetWindowLongPtrW(progman, GWL_EXSTYLE) as u32);
    if form != DesktopForm::Raised {
        eprintln!(
            "Este Windows tiene el escritorio clásico; la prueba es para el elevado (24H2+)."
        );
        return 3;
    }
    let mut result = 0usize;
    SendMessageTimeoutW(
        progman,
        SPAWN_WORKERW,
        0xD,
        1,
        SMTO_NORMAL | SMTO_ABORTIFHUNG,
        1000,
        &mut result,
    );
    let defview = FindWindowExW(
        progman,
        null_mut(),
        wide("SHELLDLL_DefView").as_ptr(),
        null(),
    );
    let workerw = FindWindowExW(progman, null_mut(), wide("WorkerW").as_ptr(), null());

    // Monitor principal, en píxeles reales (el manifiesto pide DPI por monitor).
    let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..std::mem::zeroed()
    };
    GetMonitorInfoW(monitor, &mut info);
    let r = info.rcMonitor;
    let rect = PxRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    };

    let hinst = GetModuleHandleW(null());
    let class = wide("MiraiShinVideoProbe");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        // Propio de la ventana: el contexto OpenGL se queda con el mismo HDC todo el tiempo.
        style: CS_OWNDC,
        lpfnWndProc: Some(probe_proc),
        hInstance: hinst,
        lpszClassName: class.as_ptr(),
        ..std::mem::zeroed()
    };
    if RegisterClassExW(&wc) == 0 {
        eprintln!("RegisterClassExW falló ({})", GetLastError());
        return 4;
    }
    let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        null(),
        WS_POPUP,
        0,
        0,
        w,
        h,
        null_mut(),
        null_mut(),
        hinst,
        null(),
    );
    if hwnd.is_null() {
        eprintln!("CreateWindowExW falló ({})", GetLastError());
        return 4;
    }
    // Igual que `surface-windows`: hija de Progman, debajo de los íconos, y el estilo layered
    // recién con la ventana ya hija y visible. En B, mpv crea su propia ventana dentro.
    let mut style = WS_CHILD | WS_CLIPSIBLINGS;
    if way == Way::B {
        style |= WS_CLIPCHILDREN;
    }
    SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
    if SetParent(hwnd, progman).is_null() {
        eprintln!("SetParent falló ({})", GetLastError());
        DestroyWindow(hwnd);
        return 4;
    }
    let mut origin = POINT { x: 0, y: 0 };
    ClientToScreen(progman, &mut origin);
    let (x, y, cw, ch) = child_rect(rect, (origin.x, origin.y));
    SetWindowPos(
        hwnd,
        defview,
        x,
        y,
        cw,
        ch,
        SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
    );
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, WS_EX_LAYERED as isize);
    if SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) == 0 {
        eprintln!("SetLayeredWindowAttributes falló ({})", GetLastError());
    }
    if !workerw.is_null() {
        SetWindowPos(
            workerw,
            hwnd,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    println!(
        "Ventana lista: {cw}×{ch} en ({x}, {y}), íconos={} WorkerW={}",
        defview as isize, workerw as isize
    );

    let thread = GetCurrentThreadId();
    let wakeup = Box::new(move || {
        // SAFETY: solo deja un mensaje en la cola del hilo principal.
        unsafe { PostThreadMessageW(thread, WM_WAKE, 0, 0) };
    });

    let code = match way {
        Way::A => way_a(hwnd, w, h, video, seconds, wakeup),
        Way::B => way_b(hwnd, video, seconds, wakeup),
    };
    DestroyWindow(hwnd);
    println!("Listo: la ventana se cerró (vuelve el fondo de Windows).");
    code
}

type Wakeup = Box<dyn Fn() + Send + Sync>;

unsafe fn way_a(
    hwnd: HWND,
    w: i32,
    h: i32,
    video: &std::path::Path,
    seconds: u32,
    wakeup: Wakeup,
) -> i32 {
    let hdc = GetDC(hwnd);
    let Some(glrc) = gl_context(hdc) else {
        ReleaseDC(hwnd, hdc);
        return 5;
    };
    println!(
        "OpenGL: {} / {} / {}",
        gl_string(GL_VENDOR),
        gl_string(GL_RENDERER),
        gl_string(GL_VERSION)
    );
    // Primero un color (acento), para distinguir «no se ve nada» de «se ve OpenGL pero no
    // el video».
    glClearColor(0.27, 0.42, 0.85, 1.0);
    glClear(GL_COLOR_BUFFER_BIT);
    SwapBuffers(hdc);

    let code = 'play: {
        let mut player = match Player::new(FitMode::Fill, wakeup) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("mpv no arrancó: {e} (¿está libmpv-2.dll junto al exe?)");
                break 'play 6;
            }
        };
        if let Err(e) = player.attach_gl(gl_proc) {
            eprintln!("mpv no pudo usar OpenGL: {e}");
            break 'play 6;
        }
        if let Err(e) = player.load(video, Duration::from_secs(10)) {
            eprintln!("mpv no pudo abrir el video: {e}");
            break 'play 6;
        }
        println!("Camino A (OpenGL propio): reproduciendo {seconds} s…");
        let mut frames = 0u32;
        let code = pump(seconds, |tick| {
            if tick {
                println!("  cuadros dibujados en 5 s: {frames}");
                frames = 0;
                return;
            }
            if let Some(code) = player.drain_events() {
                eprintln!("  el video falló: {code}");
            }
            if player.needs_frame() {
                player.render(w, h);
                SwapBuffers(hdc);
                player.report_swap();
                frames += 1;
            }
        });
        // `player` se suelta acá, con el contexto todavía activo.
        drop(player);
        code
    };
    wglMakeCurrent(null_mut(), null_mut());
    wglDeleteContext(glrc);
    ReleaseDC(hwnd, hdc);
    code
}

unsafe fn way_b(hwnd: HWND, video: &std::path::Path, seconds: u32, wakeup: Wakeup) -> i32 {
    let player = match Player::in_window(FitMode::Fill, hwnd as i64, wakeup) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("mpv no arrancó: {e} (¿está libmpv-2.dll junto al exe?)");
            return 6;
        }
    };
    if let Err(e) = player.load(video, Duration::from_secs(10)) {
        eprintln!("mpv no pudo abrir el video: {e}");
        return 6;
    }
    println!("Camino B (ventana de mpv): reproduciendo {seconds} s…");
    pump(seconds, |tick| {
        if !tick {
            if let Some(code) = player.drain_events() {
                eprintln!("  el video falló: {code}");
            }
        }
    })
}

/// Bucle de mensajes durante `seconds` (redondeado a 5 s). `on(false)` en cada aviso de mpv,
/// `on(true)` cada 5 s.
unsafe fn pump(seconds: u32, mut on: impl FnMut(bool)) -> i32 {
    // Temporizador sin ventana: Windows elige el id y lo devuelve.
    let timer = SetTimer(null_mut(), 0, 5000, None);
    let ticks_to_end = seconds.div_ceil(5).max(1);
    let mut ticks = 0;
    let mut msg: MSG = std::mem::zeroed();
    loop {
        let r = GetMessageW(&mut msg, null_mut(), 0, 0);
        if r == 0 || r == -1 {
            return 0;
        }
        match msg.message {
            WM_WAKE => on(false),
            WM_TIMER if msg.hwnd.is_null() && msg.wParam == timer => {
                on(true);
                ticks += 1;
                if ticks >= ticks_to_end {
                    KillTimer(null_mut(), timer);
                    return 0;
                }
            }
            _ => {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// Crea el contexto OpenGL (el clásico de WGL: perfil de compatibilidad, la versión más alta
/// que dé el controlador) y lo deja activo.
unsafe fn gl_context(hdc: HDC) -> Option<HGLRC> {
    let pfd = PIXELFORMATDESCRIPTOR {
        nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
        nVersion: 1,
        dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
        iPixelType: PFD_TYPE_RGBA,
        cColorBits: 32,
        cAlphaBits: 8,
        iLayerType: PFD_MAIN_PLANE as u8,
        ..std::mem::zeroed()
    };
    let format = ChoosePixelFormat(hdc, &pfd);
    if format == 0 {
        eprintln!("ChoosePixelFormat falló ({})", GetLastError());
        return None;
    }
    if SetPixelFormat(hdc, format, &pfd) == 0 {
        eprintln!("SetPixelFormat falló ({})", GetLastError());
        return None;
    }
    let glrc = wglCreateContext(hdc);
    if glrc.is_null() {
        eprintln!("wglCreateContext falló ({})", GetLastError());
        return None;
    }
    if wglMakeCurrent(hdc, glrc) == 0 {
        eprintln!("wglMakeCurrent falló ({})", GetLastError());
        wglDeleteContext(glrc);
        return None;
    }
    Some(glrc)
}

unsafe fn gl_string(name: u32) -> String {
    let p = glGetString(name);
    if p.is_null() {
        return "?".into();
    }
    CStr::from_ptr(p as *const c_char)
        .to_string_lossy()
        .into_owned()
}

/// `get_proc_address` para mpv: `wglGetProcAddress` solo da las funciones nuevas; las de
/// OpenGL 1.1 salen de `opengl32.dll` (y `wglGetProcAddress` devuelve 0, 1, 2, 3 o -1).
unsafe extern "C" fn gl_proc(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let p = wglGetProcAddress(name as *const u8).map_or(0, |f| f as usize);
    if p > 3 && p != usize::MAX {
        return p as *mut c_void;
    }
    let module = GetModuleHandleW(wide("opengl32.dll").as_ptr());
    if module.is_null() {
        return null_mut();
    }
    GetProcAddress(module, name as *const u8).map_or(null_mut(), |f| f as *mut c_void)
}

/// Ventana de la prueba: no pinta con GDI (dibuja OpenGL o mpv) y deja pasar los clics.
unsafe extern "system" fn probe_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            ValidateRect(hwnd, null());
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
