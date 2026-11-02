//! Fondos por monitor con una superficie falsa en memoria (sin pantalla ni Wayland).

use miraishin_core::fit::FitMode;
use miraishin_core::surface::{
    check_frame, EventSink, Frame, OutputInfo, Scale, Size, Surface, SurfaceError, SurfaceEvent,
};
use miraishin_daemon::display::{SurfaceFactory, FALLBACK};
use miraishin_daemon::{run, RunConfig};
use miraishin_ipc::{codes, events, methods, CallError, Client, Endpoint};
use miraishin_store::Paths;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Screen {
    outputs: Vec<OutputInfo>,
    frames: HashMap<String, Frame>,
    /// Videos en reproducción (solo si `plays_video`).
    videos: HashMap<String, (std::path::PathBuf, FitMode)>,
    plays_video: bool,
    /// Lo que la superficie sabe después de `show_video` (en KDE lo informa Plasma).
    video_problems: HashMap<String, String>,
    /// Todo lo que se mandó a reproducir, en orden.
    history: Vec<String>,
    /// Sabe pausar (fase 3) y monitores con el video en pausa.
    pauses: bool,
    paused: HashSet<String>,
    sink: Option<EventSink>,
}

type Shared = Arc<Mutex<Screen>>;

struct FakeSurface(Shared);

impl Surface for FakeSurface {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.0.lock().unwrap().outputs.clone()
    }
    fn show(&mut self, output: &str, frame: Frame) -> Result<(), SurfaceError> {
        let mut s = self.0.lock().unwrap();
        let o = s
            .outputs
            .iter()
            .find(|o| o.name == output)
            .cloned()
            .ok_or(SurfaceError::UnknownOutput)?;
        check_frame(&o, &frame)?;
        s.videos.remove(output);
        s.frames.insert(output.into(), frame);
        Ok(())
    }
    fn show_video(
        &mut self,
        output: &str,
        video: &std::path::Path,
        mode: FitMode,
    ) -> Result<(), SurfaceError> {
        let mut s = self.0.lock().unwrap();
        if !s.plays_video {
            return Err(SurfaceError::VideoUnsupported);
        }
        if !s.outputs.iter().any(|o| o.name == output) {
            return Err(SurfaceError::UnknownOutput);
        }
        s.frames.remove(output);
        // Un video recién puesto arranca reproduciéndose.
        s.paused.remove(output);
        s.history.push(video.to_string_lossy().into_owned());
        s.videos.insert(output.into(), (video.to_path_buf(), mode));
        Ok(())
    }
    fn video_problem(&self, output: &str) -> Option<String> {
        self.0.lock().unwrap().video_problems.get(output).cloned()
    }
    fn set_paused(&mut self, output: &str, paused: bool) -> Result<(), SurfaceError> {
        let mut s = self.0.lock().unwrap();
        if !s.pauses {
            return Err(SurfaceError::PauseUnsupported);
        }
        if paused {
            s.paused.insert(output.into());
        } else {
            s.paused.remove(output);
        }
        Ok(())
    }
    fn clear(&mut self, output: &str) -> Result<(), SurfaceError> {
        let mut s = self.0.lock().unwrap();
        s.frames.remove(output);
        s.videos.remove(output);
        Ok(())
    }
}

fn out(name: &str, w: u32, h: u32, scale: u32) -> OutputInfo {
    OutputInfo {
        name: name.into(),
        description: format!("Monitor {name}"),
        logical: Size::new(w, h),
        scale: Scale(scale),
    }
}

fn factory(screen: Shared) -> SurfaceFactory {
    Box::new(move |sink| {
        screen.lock().unwrap().sink = Some(sink);
        Ok((Box::new(FakeSurface(screen)) as Box<dyn Surface>, "fake"))
    })
}

fn endpoint(dir: &std::path::Path) -> Endpoint {
    #[cfg(unix)]
    {
        Endpoint::Unix(dir.join("run/daemon.sock"))
    }
    #[cfg(windows)]
    {
        let _ = dir;
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Endpoint::Pipe(format!(
            r"\\.\pipe\miraishin-display-test-{}-{n}",
            std::process::id()
        ))
    }
}

async fn connect(ep: &Endpoint) -> Client {
    for _ in 0..100 {
        if let Ok(c) = Client::connect(ep).await {
            return c;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("el daemon no arrancó");
}

fn image_wallpaper(dir: &std::path::Path, folder: &str, rgba: [u8; 4]) {
    let f = dir.join(folder);
    std::fs::create_dir_all(&f).unwrap();
    image::RgbaImage::from_pixel(20, 10, image::Rgba(rgba))
        .save(f.join("a.png"))
        .unwrap();
    std::fs::write(
        f.join("manifest.json"),
        format!(r#"{{"schema_version":1,"id":"{folder}","type":"image","file":"a.png","title":"{folder}"}}"#),
    )
    .unwrap();
}

fn video_wallpaper(dir: &std::path::Path, folder: &str) {
    let f = dir.join(folder);
    std::fs::create_dir_all(&f).unwrap();
    std::fs::write(f.join("v.mp4"), b"x").unwrap();
    std::fs::write(
        f.join("manifest.json"),
        format!(r#"{{"schema_version":1,"id":"{folder}","type":"video","file":"v.mp4","title":"{folder}"}}"#),
    )
    .unwrap();
}

fn color(screen: &Shared, output: &str) -> Option<[u8; 4]> {
    let s = screen.lock().unwrap();
    let f = s.frames.get(output)?;
    // El centro: con «ajustar» las orillas son del color de fondo.
    let i = (((f.size.height / 2) * f.size.width + f.size.width / 2) * 4) as usize;
    Some(f.rgba[i..i + 4].try_into().unwrap())
}

fn code(r: Result<Value, CallError>) -> String {
    match r {
        Err(CallError::Rpc(e)) => e.code,
        other => panic!("se esperaba un error: {other:?}"),
    }
}

async fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no pasó a tiempo");
}

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn start(paths: &Paths, ep: &Endpoint, screen: &Shared) -> tokio::task::JoinHandle<()> {
    start_with(paths, ep, screen, test_video_env())
}

fn start_with(
    paths: &Paths,
    ep: &Endpoint,
    screen: &Shared,
    env: miraishin_daemon::VideoEnv,
) -> tokio::task::JoinHandle<()> {
    let config = RunConfig {
        paths: paths.clone(),
        endpoint: ep.clone(),
        init_logging: false,
        console_logs: false,
        surface: Some(factory(screen.clone())),
        video_env: Some(env),
    };
    tokio::spawn(async move {
        run(config).await.unwrap();
    })
}

async fn stop(mut c: Client, task: tokio::task::JoinHandle<()>) {
    c.call(methods::SHUTDOWN, Value::Null).await.unwrap();
    // En Windows el nombre del pipe sigue tomado mientras un cliente lo tenga abierto: un
    // daemon nuevo daría `AlreadyRunning`. Se cierra antes de esperar al viejo.
    drop(c);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn fondos_por_monitor() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    let wall = paths.default_wallpapers_dir();
    image_wallpaper(&wall, "rojo", RED);
    image_wallpaper(&wall, "azul", BLUE);
    let roto = wall.join("roto");
    std::fs::create_dir_all(&roto).unwrap();
    std::fs::write(roto.join("manifest.json"), r#"{"schema_version":1}"#).unwrap();

    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 100, 50, 120), out("HDMI-A-1", 40, 40, 240)],
        ..Screen::default()
    }));
    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;

    let status = c.call(methods::STATUS, Value::Null).await.unwrap();
    assert_eq!(status["surface"], json!("fake"));
    assert_eq!(status["surface_available"], json!(true));

    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert_eq!(list["items"][1]["pixel_width"], json!(80));
    assert_eq!(list["items"][1]["scale_percent"], json!(200));
    assert_eq!(list["items"][0]["wallpaper"], Value::Null);

    // Uno para todos.
    let r = c
        .call(methods::WALLPAPER_SET, json!({"folder": "rojo"}))
        .await
        .unwrap();
    assert_eq!(r["all"], json!("rojo"));
    assert_eq!(color(&screen, "DP-1"), Some(RED));
    assert_eq!(color(&screen, "HDMI-A-1"), Some(RED));
    {
        let s = screen.lock().unwrap();
        assert_eq!(s.frames["HDMI-A-1"].size, Size::new(80, 80));
    }
    assert_eq!(c.next_event().await.unwrap().event, events::OUTPUTS_CHANGED);

    // Uno distinto en un monitor.
    let r = c
        .call(
            methods::WALLPAPER_SET,
            json!({"folder": "azul", "output": "HDMI-A-1", "mode": "fit"}),
        )
        .await
        .unwrap();
    assert_eq!(r["items"][1]["wallpaper"], json!("azul"));
    assert_eq!(r["items"][1]["mode"], json!("fit"));
    assert_eq!(color(&screen, "HDMI-A-1"), Some(BLUE));
    assert_eq!(color(&screen, "DP-1"), Some(RED));

    // Errores con código, sin cambiar lo que se ve.
    let set = |p: Value| (methods::WALLPAPER_SET, p);
    for (params, expected) in [
        (json!({"folder": "nada"}), codes::WALLPAPER_NOT_FOUND),
        (json!({"folder": "roto"}), codes::WALLPAPER_INVALID),
        (
            json!({"folder": "rojo", "output": "DP-9"}),
            codes::UNKNOWN_OUTPUT,
        ),
        (
            json!({"folder": "rojo", "mode": "zoom"}),
            codes::INVALID_MODE,
        ),
        (json!({"carpeta": "rojo"}), codes::INVALID_PARAMS),
    ] {
        let (m, p) = set(params);
        assert_eq!(code(c.call(m, p).await), expected);
    }
    assert_eq!(color(&screen, "HDMI-A-1"), Some(BLUE));

    // Quitar en un monitor: ese queda sin fondo, el otro sigue.
    c.call(methods::WALLPAPER_CLEAR, json!({"output": "HDMI-A-1"}))
        .await
        .unwrap();
    assert_eq!(color(&screen, "HDMI-A-1"), None);
    assert_eq!(color(&screen, "DP-1"), Some(RED));

    // Un monitor nuevo recibe el de «todos».
    {
        let mut s = screen.lock().unwrap();
        let o = out("DP-2", 30, 20, 120);
        s.outputs.push(o.clone());
        (s.sink.as_mut().unwrap())(SurfaceEvent::OutputAdded(o));
    }
    wait_for(|| color(&screen, "DP-2") == Some(RED)).await;

    // El archivo desaparece: rescan → ese fondo queda inválido → color de respaldo y código.
    std::fs::remove_file(wall.join("rojo/a.png")).unwrap();
    c.call(methods::LIBRARY_RESCAN, Value::Null).await.unwrap();
    wait_for(|| color(&screen, "DP-1") == Some(FALLBACK)).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], json!("wallpaper_invalid"));

    stop(c, task).await;
}

#[tokio::test]
async fn lo_elegido_se_restaura_al_arrancar() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    image_wallpaper(&paths.default_wallpapers_dir(), "azul", BLUE);
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        ..Screen::default()
    }));

    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;
    c.call(methods::WALLPAPER_SET, json!({"folder": "azul"}))
        .await
        .unwrap();
    stop(c, task).await;

    screen.lock().unwrap().frames.clear();
    let task = start(&paths, &ep, &screen);
    let c = connect(&ep).await;
    wait_for(|| color(&screen, "DP-1") == Some(BLUE)).await;
    stop(c, task).await;
}

#[tokio::test]
async fn sin_superficie_se_guarda_igual() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    image_wallpaper(&paths.default_wallpapers_dir(), "azul", BLUE);
    let config = RunConfig {
        paths: paths.clone(),
        endpoint: ep.clone(),
        init_logging: false,
        console_logs: false,
        surface: Some(Box::new(|_| Err("kde_plugin_needs_restart".into()))),
        video_env: Some(test_video_env()),
    };
    let task = tokio::spawn(async move { run(config).await.unwrap() });
    let mut c = connect(&ep).await;
    let status = c.call(methods::STATUS, Value::Null).await.unwrap();
    assert_eq!(status["surface"], json!("kde_plugin_needs_restart"));
    assert_eq!(status["surface_available"], json!(false));
    // Un monitor que no se puede comprobar se acepta: se verá cuando haya superficie.
    let r = c
        .call(
            methods::WALLPAPER_SET,
            json!({"folder": "azul", "output": "Virtual-1"}),
        )
        .await
        .unwrap();
    assert_eq!(r["surface_available"], json!(false));
    assert_eq!(r["items"], json!([]));
    stop(c, task).await;
}

#[tokio::test]
async fn videos() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    let wall = paths.default_wallpapers_dir();
    image_wallpaper(&wall, "azul", BLUE);
    video_wallpaper(&wall, "olas");
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120), out("DP-2", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;

    // Para todos: la superficie recibe el archivo y el modo, no cuadros.
    let r = c
        .call(
            methods::WALLPAPER_SET,
            json!({"folder": "olas", "mode": "fit"}),
        )
        .await
        .unwrap();
    assert_eq!(r["items"][0]["wallpaper"], json!("olas"));
    assert_eq!(r["items"][0]["problem"], Value::Null);
    {
        let s = screen.lock().unwrap();
        let (path, mode) = &s.videos["DP-2"];
        assert_eq!(path, &wall.join("olas/v.mp4"));
        assert_eq!(*mode, FitMode::Fit);
        assert!(s.frames.is_empty());
    }

    // La superficie aceptó el video pero después supo que no se ve (Plasma sin el
    // decodificador): `outputs` lo dice, sin volver a mandarlo.
    screen
        .lock()
        .unwrap()
        .video_problems
        .insert("DP-2".into(), "video_no_decoder".into());
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], Value::Null);
    assert_eq!(list["items"][1]["problem"], json!("video_no_decoder"));
    assert_eq!(list["items"][1]["wallpaper"], json!("olas"));

    // Imagen en un monitor: ese deja el video.
    c.call(
        methods::WALLPAPER_SET,
        json!({"folder": "azul", "output": "DP-1"}),
    )
    .await
    .unwrap();
    assert_eq!(color(&screen, "DP-1"), Some(BLUE));
    assert!(!screen.lock().unwrap().videos.contains_key("DP-1"));
    // Con una imagen no se le pregunta por el video (aunque la superficie dijera algo).
    screen
        .lock()
        .unwrap()
        .video_problems
        .insert("DP-1".into(), "video_too_slow".into());
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], Value::Null);
    screen.lock().unwrap().video_problems.clear();
    assert!(screen.lock().unwrap().videos.contains_key("DP-2"));

    // El archivo desaparece sin rescan: respaldo con su código.
    std::fs::remove_file(wall.join("olas/v.mp4")).unwrap();
    c.call(methods::WALLPAPER_CLEAR, json!({"output": "DP-2"}))
        .await
        .unwrap();
    c.call(
        methods::WALLPAPER_SET,
        json!({"folder": "olas", "output": "DP-2"}),
    )
    .await
    .unwrap();
    assert_eq!(color(&screen, "DP-2"), Some(FALLBACK));
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][1]["problem"], json!("video_unreadable"));
    stop(c, task).await;

    // Una superficie que todavía no reproduce video: respaldo y `video_unsupported`.
    video_wallpaper(&wall, "olas");
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        ..Screen::default()
    }));
    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;
    c.call(methods::LIBRARY_RESCAN, Value::Null).await.unwrap();
    c.call(methods::WALLPAPER_SET, json!({"folder": "olas"}))
        .await
        .unwrap();
    wait_for(|| color(&screen, "DP-1") == Some(FALLBACK)).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["wallpaper"], json!("olas"));
    assert_eq!(list["items"][0]["problem"], json!("video_unsupported"));
    stop(c, task).await;
}

/// Sin GPU virtual, sin `ffmpeg` y sin revisiones: nada de copias salvo que la prueba lo pida.
fn test_video_env() -> miraishin_daemon::VideoEnv {
    miraishin_daemon::VideoEnv {
        no_real_gpu: false,
        ffmpeg: None,
        check_delays: Vec::new(),
    }
}

/// `ffmpeg` falso: escribe la duración y el avance como el de verdad y crea el archivo de
/// salida (el último argumento). Con `falla` sale con error. Sin `-progress` (leer la
/// cabecera) escribe el flujo de un video 1080p30 H.264.
#[cfg(unix)]
fn fake_ffmpeg(dir: &std::path::Path, falla: bool) -> std::path::PathBuf {
    fake_ffmpeg_with(dir, falla, "1920x1080", "h264")
}

#[cfg(unix)]
fn fake_ffmpeg_with(
    dir: &std::path::Path,
    falla: bool,
    size: &str,
    codec: &str,
) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(format!(
        "ffmpeg-{}-{size}-{codec}",
        if falla { "malo" } else { "bueno" }
    ));
    let probe = format!(
        "case \"$*\" in *-progress*) ;; *) echo '  Stream #0:0: Video: {codec} (High), \
         yuv420p(progressive), {size} [SAR 1:1 DAR 16:9], 30 fps, 30 tbr, 15360 tbn' >&2; \
         exit 1;; esac\n"
    );
    let body = if falla {
        "echo 'Invalid data found' >&2\nexit 1\n".to_string()
    } else {
        "echo '  Duration: 00:00:02.00, start: 0.000000' >&2\necho out_time_us=1000000\n\
         for a in \"$@\"; do last=\"$a\"; done\nprintf webm > \"$last\"\necho progress=end\n"
            .to_string()
    };
    std::fs::write(&p, format!("#!/bin/sh\n{probe}{body}")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[cfg(unix)]
fn playing(screen: &Shared, output: &str) -> Option<String> {
    let s = screen.lock().unwrap();
    s.videos
        .get(output)
        .map(|(p, _)| p.to_string_lossy().into_owned())
}

#[cfg(unix)]
#[tokio::test]
async fn copias_compatibles() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    let wall = paths.default_wallpapers_dir();
    video_wallpaper(&wall, "olas");
    let env = |no_real_gpu, ffmpeg| miraishin_daemon::VideoEnv {
        no_real_gpu,
        ffmpeg: Some(ffmpeg),
        check_delays: vec![Duration::from_millis(50)],
    };

    // 1. Equipo con GPU: primero el original. La superficie dice que no lo puede decodificar
    //    → copia del tamaño de la pantalla (10×10 px); sigue sin verse → copia de ahorro.
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start_with(
        &paths,
        &ep,
        &screen,
        env(false, fake_ffmpeg(t.path(), false)),
    );
    let mut c = connect(&ep).await;
    c.call(methods::WALLPAPER_SET, json!({"folder": "olas"}))
        .await
        .unwrap();
    assert_eq!(
        playing(&screen, "DP-1"),
        Some(wall.join("olas/v.mp4").to_string_lossy().into_owned())
    );
    screen
        .lock()
        .unwrap()
        .video_problems
        .insert("DP-1".into(), "video_no_decoder".into());
    wait_for(|| playing(&screen, "DP-1").is_some_and(|p| p.contains("1280x720-30-"))).await;
    {
        let h = &screen.lock().unwrap().history;
        assert_eq!(h.len(), 3, "{h:?}");
        assert!(h[0].ends_with("olas/v.mp4"));
        assert!(
            h[1].contains("video-copies") && h[1].contains("/10x10-60-"),
            "{h:?}"
        );
        assert!(h[2].contains("/1280x720-30-"), "{h:?}");
    }
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["copy"], json!("1280x720"));
    assert_eq!(list["items"][0]["copy_progress"], Value::Null);
    // La copia de pantalla se borró al pasar a ahorro: queda una sola.
    let dir = paths.video_copies_dir();
    let copies: Vec<_> = std::fs::read_dir(
        std::fs::read_dir(&dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap()
    .flatten()
    .filter(|e| e.path().extension().is_some_and(|x| x == "webm"))
    .collect();
    assert_eq!(copies.len(), 1);
    // Ya en el último nivel, el problema se informa en vez de subir más.
    tokio::time::sleep(Duration::from_millis(150)).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], json!("video_no_decoder"));
    stop(c, task).await;

    // 2. Sin GPU real: directo a la copia de ahorro, que ya existe de antes (no se rehace).
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start_with(&paths, &ep, &screen, env(true, t.path().join("no-existe")));
    let c = connect(&ep).await;
    wait_for(|| playing(&screen, "DP-1").is_some_and(|p| p.contains("1280x720-30-"))).await;
    assert_eq!(
        screen.lock().unwrap().history.len(),
        1,
        "sin pasar por el original"
    );
    stop(c, task).await;

    // 3. La copia falla: color de respaldo y el código.
    std::fs::remove_dir_all(&dir).unwrap();
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start_with(&paths, &ep, &screen, env(true, fake_ffmpeg(t.path(), true)));
    let mut c = connect(&ep).await;
    wait_for(|| color(&screen, "DP-1") == Some(FALLBACK)).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], json!("copy_failed"));
    assert_eq!(playing(&screen, "DP-1"), None);
    stop(c, task).await;

    // 4. Sin GPU real, pero el video ya es de ahorro (720p30 H.264): el original, sin copia.
    //    Si la superficie dice que no se ve, directo a la copia de ahorro.
    std::fs::remove_dir_all(&dir).ok();
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let ffmpeg = fake_ffmpeg_with(t.path(), false, "1280x720", "h264");
    let task = start_with(&paths, &ep, &screen, env(true, ffmpeg));
    let mut c = connect(&ep).await;
    let original = wall.join("olas/v.mp4").to_string_lossy().into_owned();
    wait_for(|| playing(&screen, "DP-1").as_deref() == Some(original.as_str())).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["copy"], Value::Null);
    assert!(!dir.exists(), "no se creó ninguna copia");
    screen
        .lock()
        .unwrap()
        .video_problems
        .insert("DP-1".into(), "video_too_slow".into());
    wait_for(|| playing(&screen, "DP-1").is_some_and(|p| p.contains("1280x720-30-"))).await;
    {
        let h = &screen.lock().unwrap().history;
        assert_eq!(h.len(), 2, "sin pasar por la copia de pantalla: {h:?}");
    }
    stop(c, task).await;

    // 5. Sin GPU real y 720p en HEVC: se copia igual (Plasma de algunas distros no lo trae).
    std::fs::remove_dir_all(&dir).ok();
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let ffmpeg = fake_ffmpeg_with(t.path(), false, "1280x720", "hevc");
    let task = start_with(&paths, &ep, &screen, env(true, ffmpeg));
    let c = connect(&ep).await;
    wait_for(|| playing(&screen, "DP-1").is_some_and(|p| p.contains("1280x720-30-"))).await;
    assert_eq!(screen.lock().unwrap().history.len(), 1);
    stop(c, task).await;

    // 6. Archivo cortado (ffmpeg no lo abre): `video_damaged`, no `copy_failed`.
    std::fs::remove_dir_all(&dir).ok();
    let roto = t.path().join("ffmpeg-roto");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            &roto,
            "#!/bin/sh\necho '[mov,mp4 @ 0x1] moov atom not found' >&2\n\
             echo 'Error opening input files: Invalid data found when processing input' >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&roto, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 10, 10, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start_with(&paths, &ep, &screen, env(true, roto));
    let mut c = connect(&ep).await;
    wait_for(|| color(&screen, "DP-1") == Some(FALLBACK)).await;
    let list = c.call(methods::OUTPUTS_LIST, Value::Null).await.unwrap();
    assert_eq!(list["items"][0]["problem"], json!("video_damaged"));
    stop(c, task).await;
}

fn paused(list: &Value, output: &str) -> Value {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == json!(output))
        .unwrap()["paused"]
        .clone()
}

#[tokio::test]
async fn pausa_a_mano() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    let wall = paths.default_wallpapers_dir();
    video_wallpaper(&wall, "olas");
    video_wallpaper(&wall, "nubes");
    image_wallpaper(&wall, "rojo", RED);
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![
            out("DP-1", 100, 50, 120),
            out("HDMI-A-1", 40, 40, 120),
            out("eDP-1", 40, 40, 120),
        ],
        plays_video: true,
        pauses: true,
        ..Screen::default()
    }));
    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;
    let set = |folder: &str, output: Option<&str>| json!({"folder": folder, "output": output});
    c.call(methods::WALLPAPER_SET, set("olas", None))
        .await
        .unwrap();
    c.call(methods::WALLPAPER_SET, set("rojo", Some("eDP-1")))
        .await
        .unwrap();
    let paused_set = |s: &Shared| {
        let mut v: Vec<String> = s.lock().unwrap().paused.iter().cloned().collect();
        v.sort();
        v
    };

    // Todos: los dos con video quedan en pausa; el de la imagen no tiene nada que pausar.
    let r = c.call(methods::PLAYBACK_PAUSE, Value::Null).await.unwrap();
    assert_eq!(paused_set(&screen), vec!["DP-1", "HDMI-A-1"]);
    assert_eq!(paused(&r, "DP-1"), json!("manual"));
    assert_eq!(paused(&r, "eDP-1"), Value::Null);

    // Un video nuevo mientras está en pausa: también queda en pausa.
    c.call(methods::WALLPAPER_SET, set("nubes", Some("DP-1")))
        .await
        .unwrap();
    assert_eq!(paused_set(&screen), vec!["DP-1", "HDMI-A-1"]);

    // Quitar la pausa de uno deja al otro en pausa.
    let r = c
        .call(methods::PLAYBACK_RESUME, json!({"output": "DP-1"}))
        .await
        .unwrap();
    assert_eq!(paused_set(&screen), vec!["HDMI-A-1"]);
    assert_eq!(paused(&r, "DP-1"), Value::Null);
    assert_eq!(paused(&r, "HDMI-A-1"), json!("manual"));

    // Uno solo, y quitar todas.
    c.call(methods::PLAYBACK_PAUSE, json!({"output": "DP-1"}))
        .await
        .unwrap();
    assert_eq!(paused_set(&screen), vec!["DP-1", "HDMI-A-1"]);
    let r = c.call(methods::PLAYBACK_RESUME, Value::Null).await.unwrap();
    assert!(paused_set(&screen).is_empty());
    assert_eq!(paused(&r, "HDMI-A-1"), Value::Null);

    // Monitor que no existe.
    assert_eq!(
        code(
            c.call(methods::PLAYBACK_PAUSE, json!({"output": "VGA-9"}))
                .await
        ),
        codes::UNKNOWN_OUTPUT
    );
    stop(c, task).await;
}

#[tokio::test]
async fn superficie_que_no_sabe_pausar() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    video_wallpaper(&paths.default_wallpapers_dir(), "olas");
    let screen: Shared = Arc::new(Mutex::new(Screen {
        outputs: vec![out("DP-1", 100, 50, 120)],
        plays_video: true,
        ..Screen::default()
    }));
    let task = start(&paths, &ep, &screen);
    let mut c = connect(&ep).await;
    c.call(methods::WALLPAPER_SET, json!({"folder": "olas"}))
        .await
        .unwrap();
    let r = c.call(methods::PLAYBACK_PAUSE, Value::Null).await.unwrap();
    assert_eq!(paused(&r, "DP-1"), Value::Null);
    assert_eq!(r["items"][0]["pause_unsupported"], json!(true));
    let r = c.call(methods::PLAYBACK_RESUME, Value::Null).await.unwrap();
    assert_eq!(r["items"][0]["pause_unsupported"], json!(false));
    stop(c, task).await;
}
