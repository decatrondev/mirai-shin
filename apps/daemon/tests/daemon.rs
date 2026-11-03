//! El daemon entero con carpetas y socket temporales (nunca los datos reales).

use miraishin_daemon::{run, RunConfig, RunError};
use miraishin_ipc::{codes, events, methods, CallError, Client, Endpoint};
use miraishin_store::Paths;
use serde_json::{json, Value};
use std::time::Duration;

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
            r"\\.\pipe\miraishin-daemon-test-{}-{n}",
            std::process::id()
        ))
    }
}

/// Sin pantalla: el daemon funciona igual (biblioteca, IPC) y lo dice en `status`.
fn no_surface() -> miraishin_daemon::display::SurfaceFactory {
    Box::new(|_| Err("test_no_surface".into()))
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

fn wallpaper(dir: &std::path::Path, folder: &str, manifest: &str, file: Option<&str>) {
    let f = dir.join(folder);
    std::fs::create_dir_all(&f).unwrap();
    std::fs::write(f.join("manifest.json"), manifest).unwrap();
    if let Some(file) = file {
        std::fs::write(f.join(file), b"x").unwrap();
    }
}

#[tokio::test]
async fn ciclo_completo() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    let ep = endpoint(t.path());
    let wall = paths.default_wallpapers_dir();
    wallpaper(
        &wall,
        "lluvia",
        r#"{"schema_version":1,"id":"lluvia","type":"image","file":"a.png","title":"Lluvia"}"#,
        Some("a.png"),
    );

    let task = tokio::spawn(run(RunConfig {
        paths: paths.clone(),
        endpoint: ep.clone(),
        init_logging: false,
        console_logs: false,
        surface: Some(no_surface()),
        video_env: Some(test_video_env()),
        power: Some(miraishin_daemon::power::none()),
    }));
    let mut c = connect(&ep).await;

    let hello = c
        .call(methods::HELLO, json!({"protocol": 1, "client": "test"}))
        .await
        .unwrap();
    assert_eq!(hello["protocol"], json!(1));
    assert!(hello["methods"]
        .as_array()
        .unwrap()
        .contains(&json!("library.rescan")));

    let status = c.call(methods::STATUS, Value::Null).await.unwrap();
    assert_eq!(
        (status["valid"].clone(), status["invalid"].clone()),
        (json!(1), json!(0))
    );
    assert_eq!(status["license"], json!("not_required"));
    assert_eq!(status["wallpapers_dir_exists"], json!(true));

    // Uno inválido y un rescan: llega el evento y la lista lo muestra con su código.
    wallpaper(
        &wall,
        "roto",
        r#"{"schema_version":1,"type":"image"}"#,
        None,
    );
    let r = c.call(methods::LIBRARY_RESCAN, Value::Null).await.unwrap();
    assert_eq!(r, json!({"valid": 1, "invalid": 1, "removed": 0}));
    assert_eq!(c.next_event().await.unwrap().event, events::LIBRARY_CHANGED);
    let list = c.call(methods::LIBRARY_LIST, Value::Null).await.unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let roto = items.iter().find(|i| i["folder"] == json!("roto")).unwrap();
    assert_eq!(roto["valid"], json!(false));
    assert!(roto["issues"]
        .as_array()
        .unwrap()
        .contains(&json!("missing_field:id")));

    match c.call("no.existe", Value::Null).await {
        Err(CallError::Rpc(e)) => assert_eq!(e.code, codes::UNKNOWN_METHOD),
        other => panic!("{other:?}"),
    }
    match c.call(methods::HELLO, json!({"protocol": "uno"})).await {
        Err(CallError::Rpc(e)) => assert_eq!(e.code, codes::INVALID_PARAMS),
        other => panic!("{other:?}"),
    }

    // Segunda instancia: se niega sin tocar a la primera.
    let second = run(RunConfig {
        paths: paths.clone(),
        endpoint: ep.clone(),
        init_logging: false,
        console_logs: false,
        surface: Some(no_surface()),
        video_env: Some(test_video_env()),
        power: Some(miraishin_daemon::power::none()),
    })
    .await;
    assert!(matches!(second, Err(RunError::AlreadyRunning)));
    assert_eq!(
        c.call(methods::PING, Value::Null).await.unwrap(),
        json!("pong")
    );

    assert_eq!(
        c.call(methods::SHUTDOWN, Value::Null).await.unwrap(),
        json!({"stopping": true})
    );
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    #[cfg(unix)]
    {
        let Endpoint::Unix(p) = &ep;
        assert!(!p.exists(), "el socket se borra al salir");
    }
    assert!(paths.library_db().exists());
}

#[tokio::test]
async fn carpeta_elegida_que_no_existe() {
    let t = tempfile::tempdir().unwrap();
    let paths = Paths::under(t.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    let missing = t.path().join("disco-sin-montar");
    std::fs::write(
        paths.settings_file(),
        format!("wallpapers_dir = {:?}\n", missing.display().to_string()),
    )
    .unwrap();
    let ep = endpoint(t.path());
    let task = tokio::spawn(run(RunConfig {
        paths,
        endpoint: ep.clone(),
        init_logging: false,
        console_logs: false,
        surface: Some(no_surface()),
        video_env: Some(test_video_env()),
        power: Some(miraishin_daemon::power::none()),
    }));
    let mut c = connect(&ep).await;
    let status = c.call(methods::STATUS, Value::Null).await.unwrap();
    assert_eq!(status["wallpapers_dir_exists"], json!(false));
    assert!(
        !missing.exists(),
        "una carpeta elegida por la persona no se inventa"
    );
    match c.call(methods::LIBRARY_RESCAN, Value::Null).await {
        Err(CallError::Rpc(e)) => assert_eq!(e.code, codes::WALLPAPERS_DIR_MISSING),
        other => panic!("{other:?}"),
    }
    c.call(methods::SHUTDOWN, Value::Null).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

/// Sin GPU virtual, sin `ffmpeg` y sin revisiones: nada de copias salvo que la prueba lo pida.
fn test_video_env() -> miraishin_daemon::VideoEnv {
    miraishin_daemon::VideoEnv {
        no_real_gpu: false,
        ffmpeg: None,
        check_delays: Vec::new(),
    }
}
