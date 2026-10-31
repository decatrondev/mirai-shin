//! Servidor y cliente en el mismo proceso sobre un socket temporal.

use miraishin_ipc::{
    codes, events, methods, serve, CallError, Client, Endpoint, Event, Handler, HandlerFuture,
    Listener, RpcError,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot};

struct Echo {
    events: broadcast::Sender<Event>,
}

impl Handler for Echo {
    fn handle(&self, method: String, params: Value) -> HandlerFuture<'_> {
        Box::pin(async move {
            match method.as_str() {
                methods::PING => Ok(json!("pong")),
                "echo" => Ok(params),
                methods::LIBRARY_RESCAN => {
                    let _ = self
                        .events
                        .send(Event::new(events::LIBRARY_CHANGED, json!({"valid": 1})));
                    Ok(json!({"valid": 1}))
                }
                _ => Err(RpcError::new(codes::UNKNOWN_METHOD)),
            }
        })
    }
}

fn endpoint(dir: &tempfile::TempDir) -> Endpoint {
    #[cfg(unix)]
    {
        Endpoint::Unix(dir.path().join("run/daemon.sock"))
    }
    #[cfg(windows)]
    {
        let _ = dir;
        Endpoint::Pipe(format!(
            r"\\.\pipe\miraishin-test-{}-{}",
            std::process::id(),
            rand_suffix()
        ))
    }
}

#[cfg(windows)]
fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

async fn start(ep: &Endpoint) -> oneshot::Sender<()> {
    let listener = Listener::bind(ep).await.unwrap();
    let (tx, _) = broadcast::channel(16);
    let handler = Arc::new(Echo { events: tx.clone() });
    let (stop, rx) = oneshot::channel::<()>();
    tokio::spawn(serve(listener, handler, tx, async move {
        let _ = rx.await;
    }));
    stop
}

#[tokio::test]
async fn pedidos_errores_y_eventos() {
    let dir = tempfile::tempdir().unwrap();
    let ep = endpoint(&dir);
    let _stop = start(&ep).await;
    let mut c = Client::connect(&ep).await.unwrap();

    assert_eq!(
        c.call(methods::PING, Value::Null).await.unwrap(),
        json!("pong")
    );
    assert_eq!(
        c.call("echo", json!({"a": [1, 2]})).await.unwrap(),
        json!({"a": [1, 2]})
    );

    // Método desconocido: error con código, la conexión sigue viva.
    match c.call("no.existe", Value::Null).await {
        Err(CallError::Rpc(e)) => assert_eq!(e.code, codes::UNKNOWN_METHOD),
        other => panic!("{other:?}"),
    }
    // Campos desconocidos en el pedido: se ignoran.
    c.send_raw(r#"{"v":1,"id":50,"method":"ping","del_futuro":{"x":1}}"#)
        .await
        .unwrap();
    assert_eq!(c.read_value().await.unwrap()["result"], json!("pong"));
    // JSON roto: error, no caída; si se lee el id, vuelve.
    c.send_raw("{esto no es json").await.unwrap();
    assert_eq!(
        c.read_value().await.unwrap()["error"]["code"],
        json!(codes::INVALID_REQUEST)
    );
    c.send_raw(r#"{"v":1,"id":51}"#).await.unwrap();
    let v = c.read_value().await.unwrap();
    assert_eq!(
        (v["id"].clone(), v["error"]["code"].clone()),
        (json!(51), json!(codes::INVALID_REQUEST))
    );
    assert_eq!(
        c.call(methods::PING, Value::Null).await.unwrap(),
        json!("pong")
    );

    // Evento a todos los clientes conectados.
    let mut other = Client::connect(&ep).await.unwrap();
    other.call(methods::PING, Value::Null).await.unwrap();
    c.call(methods::LIBRARY_RESCAN, Value::Null).await.unwrap();
    assert_eq!(
        other.next_event().await.unwrap().event,
        events::LIBRARY_CHANGED
    );
    assert_eq!(c.next_event().await.unwrap().event, events::LIBRARY_CHANGED);
}

#[tokio::test]
async fn linea_demasiado_larga_corta_la_conexion() {
    let dir = tempfile::tempdir().unwrap();
    let ep = endpoint(&dir);
    let _stop = start(&ep).await;
    let mut c = Client::connect(&ep).await.unwrap();
    // El servidor corta a mitad de la escritura: el error de envío es esperable.
    let _ = c.send_raw(&"x".repeat(miraishin_ipc::MAX_LINE + 10)).await;
    assert_eq!(
        c.read_value().await.unwrap()["error"]["code"],
        json!(codes::LINE_TOO_LONG)
    );
    assert!(matches!(c.read_value().await, Err(CallError::Closed)));
}

#[tokio::test]
async fn una_sola_instancia() {
    let dir = tempfile::tempdir().unwrap();
    let ep = endpoint(&dir);
    let stop = start(&ep).await;
    assert!(matches!(
        Listener::bind(&ep).await,
        Err(miraishin_ipc::BindError::AlreadyRunning)
    ));
    drop(stop);
}

#[cfg(unix)]
#[tokio::test]
async fn socket_huerfano_se_reemplaza_y_queda_privado() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ep = endpoint(&dir);
    let Endpoint::Unix(path) = &ep;
    // Un socket que nadie atiende (daemon caído): se crea y se suelta sin borrar el archivo.
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let orphan = std::os::unix::net::UnixListener::bind(path).unwrap();
    drop(orphan);
    assert!(path.exists());

    let _stop = start(&ep).await;
    let mut c = Client::connect(&ep).await.unwrap();
    assert_eq!(
        c.call(methods::PING, Value::Null).await.unwrap(),
        json!("pong")
    );
    let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let dir_mode = std::fs::metadata(path.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);
}
