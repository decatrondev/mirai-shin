//! Servidor: atiende cada conexión en su tarea, responde pedidos en orden y le reenvía
//! los eventos del daemon.

use crate::protocol::{codes, Event, Request, Response, RpcError, MAX_LINE, PROTOCOL_VERSION};
use crate::transport::{Listener, Stream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec, LinesCodecError};

pub type HandlerFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + 'a>>;

/// Lo que hace el daemon con cada pedido. Debe responder `unknown_method` para lo que no conoce.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, method: String, params: Value) -> HandlerFuture<'_>;
}

/// Atiende conexiones hasta que `shutdown` termina.
pub async fn serve(
    mut listener: Listener,
    handler: Arc<dyn Handler>,
    events: broadcast::Sender<Event>,
    shutdown: impl Future<Output = ()>,
) {
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let handler = handler.clone();
                    let rx = events.subscribe();
                    tokio::spawn(async move { connection(stream, handler, rx).await });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ipc accept failed");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            },
        }
    }
}

async fn connection(
    stream: Stream,
    handler: Arc<dyn Handler>,
    mut events: broadcast::Receiver<Event>,
) {
    let (read, write) = tokio::io::split(stream);
    let mut lines = FramedRead::new(read, LinesCodec::new_with_max_length(MAX_LINE));
    let mut out = FramedWrite::new(write, LinesCodec::new());

    loop {
        tokio::select! {
            line = lines.next() => {
                let response = match line {
                    None => break,
                    Some(Err(LinesCodecError::MaxLineLengthExceeded)) => {
                        let _ = out.send(encode(&error_response(None, codes::LINE_TOO_LONG))).await;
                        break;
                    }
                    Some(Err(_)) => break,
                    Some(Ok(text)) => answer(&text, handler.as_ref()).await,
                };
                if out.send(encode(&response)).await.is_err() {
                    break;
                }
            }
            event = events.recv() => match event {
                Ok(ev) => {
                    if out.send(encode(&ev)).await.is_err() {
                        break;
                    }
                }
                // Cliente lento: se pierden eventos viejos, no se corta.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
}

async fn answer(text: &str, handler: &dyn Handler) -> Response {
    let request: Request = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            // Si al menos se puede leer el id, se devuelve para que el cliente no quede esperando.
            let id = serde_json::from_str::<Value>(text)
                .ok()
                .and_then(|v| v.get("id").and_then(Value::as_u64));
            return error_response(id, codes::INVALID_REQUEST);
        }
    };
    match handler.handle(request.method, request.params).await {
        Ok(result) => Response {
            v: PROTOCOL_VERSION,
            id: Some(request.id),
            result: Some(result),
            error: None,
        },
        Err(err) => Response {
            v: PROTOCOL_VERSION,
            id: Some(request.id),
            result: None,
            error: Some(err),
        },
    }
}

fn error_response(id: Option<u64>, code: &str) -> Response {
    Response {
        v: PROTOCOL_VERSION,
        id,
        result: None,
        error: Some(RpcError::new(code)),
    }
}

fn encode<T: serde::Serialize>(msg: &T) -> String {
    // Los tipos del protocolo siempre se pueden serializar.
    serde_json::to_string(msg).unwrap_or_default()
}
