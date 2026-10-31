//! Cliente: lo usan `ctl` y, más adelante, la app.

use crate::protocol::{Event, Request, Response, RpcError, MAX_LINE, PROTOCOL_VERSION};
use crate::transport::{self, Endpoint, Stream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::VecDeque;
use tokio::io::{ReadHalf, WriteHalf};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("no se pudo conectar: {0}")]
    Connect(std::io::Error),
    #[error("conexión: {0}")]
    Io(String),
    #[error("el daemon cerró la conexión")]
    Closed,
    #[error("respuesta ilegible")]
    BadResponse,
    #[error("error del daemon: {}", .0.code)]
    Rpc(RpcError),
}

pub struct Client {
    lines: FramedRead<ReadHalf<Stream>, LinesCodec>,
    out: FramedWrite<WriteHalf<Stream>, LinesCodec>,
    next_id: u64,
    pending_events: VecDeque<Event>,
}

impl Client {
    pub async fn connect(endpoint: &Endpoint) -> Result<Self, CallError> {
        let stream = transport::connect(endpoint)
            .await
            .map_err(CallError::Connect)?;
        let (read, write) = tokio::io::split(stream);
        Ok(Self {
            lines: FramedRead::new(read, LinesCodec::new_with_max_length(MAX_LINE)),
            out: FramedWrite::new(write, LinesCodec::new()),
            next_id: 1,
            pending_events: VecDeque::new(),
        })
    }

    /// Manda un pedido y espera su respuesta. Los eventos que lleguen mientras tanto
    /// se guardan para `next_event`.
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, CallError> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            v: PROTOCOL_VERSION,
            id,
            method: method.to_string(),
            params,
        };
        let text = serde_json::to_string(&req).map_err(|_| CallError::BadResponse)?;
        self.send_raw(&text).await?;
        loop {
            let value = self.read_value().await?;
            if value.get("event").is_some() {
                if let Ok(ev) = serde_json::from_value(value) {
                    self.pending_events.push_back(ev);
                }
                continue;
            }
            let resp: Response =
                serde_json::from_value(value).map_err(|_| CallError::BadResponse)?;
            if resp.id != Some(id) && resp.id.is_some() {
                continue;
            }
            return match (resp.result, resp.error) {
                (_, Some(err)) => Err(CallError::Rpc(err)),
                (Some(result), None) => Ok(result),
                (None, None) => Ok(Value::Null),
            };
        }
    }

    pub async fn next_event(&mut self) -> Result<Event, CallError> {
        if let Some(ev) = self.pending_events.pop_front() {
            return Ok(ev);
        }
        loop {
            let value = self.read_value().await?;
            if value.get("event").is_some() {
                return serde_json::from_value(value).map_err(|_| CallError::BadResponse);
            }
        }
    }

    /// Línea tal cual: para probar cómo responde el daemon a mensajes rotos.
    pub async fn send_raw(&mut self, line: &str) -> Result<(), CallError> {
        self.out
            .send(line.to_string())
            .await
            .map_err(|e| CallError::Io(e.to_string()))
    }

    pub async fn read_value(&mut self) -> Result<Value, CallError> {
        match self.lines.next().await {
            None => Err(CallError::Closed),
            Some(Err(e)) => Err(CallError::Io(e.to_string())),
            Some(Ok(line)) => serde_json::from_str(&line).map_err(|_| CallError::BadResponse),
        }
    }
}
