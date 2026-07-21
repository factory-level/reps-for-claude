//! Blocking WebSocket client for hubd's /v1/ws — no tokio, one IO thread.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::{EnableMetric, HubError, HubHealth, VisionEvent, VisionHub, SUPPORTED_API_MAJOR};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const READ_POLL: Duration = Duration::from_millis(25);

struct Request {
    method: String,
    params: serde_json::Value,
    reply: mpsc::Sender<Result<serde_json::Value, HubError>>,
}

pub struct HubClient {
    req_tx: mpsc::Sender<Request>,
    events_rx: Option<mpsc::Receiver<VisionEvent>>,
    pub hub_version: String,
    pub api_version: String,
}

impl HubClient {
    /// Connect, read the hello frame, verify the API major version, and
    /// subscribe to all streams.
    pub fn connect(url: &str) -> Result<Self, HubError> {
        let (mut ws, _resp) =
            tungstenite::connect(url).map_err(|e| HubError::Io(e.to_string()))?;
        set_read_timeout(&mut ws, READ_POLL)?;

        let hello = read_hello(&mut ws)?;
        let api_version = hello
            .get("apiVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if api_version.split('.').next() != Some(SUPPORTED_API_MAJOR) {
            return Err(HubError::VersionMismatch {
                hub: api_version,
                supported: SUPPORTED_API_MAJOR.into(),
            });
        }
        let hub_version = hello
            .get("hubVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let (req_tx, req_rx) = mpsc::channel::<Request>();
        let (event_tx, events_rx) = mpsc::channel::<VisionEvent>();
        std::thread::Builder::new()
            .name("hub-client-io".into())
            .spawn(move || io_loop(ws, req_rx, event_tx))
            .map_err(|e| HubError::Io(e.to_string()))?;

        let client = HubClient {
            req_tx,
            events_rx: Some(events_rx),
            hub_version,
            api_version,
        };
        client.rpc(
            "subscribe",
            serde_json::json!({"streams": ["landmarks", "progress", "event", "health"]}),
        )?;
        Ok(client)
    }

    fn rpc(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, HubError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.req_tx
            .send(Request {
                method: method.to_string(),
                params,
                reply: reply_tx,
            })
            .map_err(|_| HubError::Down)?;
        reply_rx
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(|_| HubError::Timeout(method.to_string()))?
    }
}

impl HubClient {
    /// Register a camera in the hub's registry (API 1.1+).
    pub fn add_camera(&mut self, camera: &serde_json::Value) -> Result<(), HubError> {
        self.rpc("add_camera", serde_json::json!({"camera": camera}))
            .map(|_| ())
    }
}

impl VisionHub for HubClient {
    fn enable_metric(&mut self, req: &EnableMetric) -> Result<(), HubError> {
        let mut params = serde_json::json!({
            "metricId": req.metric_id,
            "pluginId": req.plugin_id,
            "config": req.config,
        });
        if let Some(cameras) = &req.cameras {
            params["cameras"] = serde_json::json!(cameras);
        }
        self.rpc("enable_metric", params).map(|_| ())
    }

    fn disable_metric(&mut self, metric_id: &str) -> Result<(), HubError> {
        self.rpc("disable_metric", serde_json::json!({"metricId": metric_id}))
            .map(|_| ())
    }

    fn update_metric_config(
        &mut self,
        metric_id: &str,
        config: &serde_json::Value,
    ) -> Result<(), HubError> {
        self.rpc(
            "update_metric_config",
            serde_json::json!({"metricId": metric_id, "config": config}),
        )
        .map(|_| ())
    }

    fn simulate(&mut self, metric_id: &str, event: &serde_json::Value) -> Result<(), HubError> {
        self.rpc(
            "simulate",
            serde_json::json!({"metricId": metric_id, "event": event}),
        )
        .map(|_| ())
    }

    fn health(&mut self) -> Result<HubHealth, HubError> {
        let result = self.rpc("health", serde_json::json!({}))?;
        serde_json::from_value(result).map_err(|e| HubError::Api(e.to_string()))
    }

    fn take_receiver(&mut self) -> Option<mpsc::Receiver<VisionEvent>> {
        self.events_rx.take()
    }
}

fn set_read_timeout(
    ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Duration,
) -> Result<(), HubError> {
    match ws.get_mut() {
        MaybeTlsStream::Plain(stream) => stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| HubError::Io(e.to_string())),
        _ => Ok(()),
    }
}

fn read_hello(
    ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
) -> Result<serde_json::Value, HubError> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    while Instant::now() < deadline {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(|e| HubError::Api(e.to_string()))?;
                if value.get("type").and_then(|t| t.as_str()) == Some("hello") {
                    return Ok(value);
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(err))
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(err) => return Err(HubError::Io(err.to_string())),
        }
    }
    Err(HubError::Timeout("hello frame".into()))
}

fn io_loop(
    mut ws: WebSocket<MaybeTlsStream<TcpStream>>,
    req_rx: mpsc::Receiver<Request>,
    event_tx: mpsc::Sender<VisionEvent>,
) {
    let mut next_id: u64 = 1;
    let mut pending: HashMap<u64, mpsc::Sender<Result<serde_json::Value, HubError>>> =
        HashMap::new();
    loop {
        while let Ok(req) = req_rx.try_recv() {
            let id = next_id;
            next_id += 1;
            let frame = serde_json::json!({"id": id, "method": req.method, "params": req.params});
            if ws.send(Message::Text(frame.to_string().into())).is_err() {
                let _ = req.reply.send(Err(HubError::Down));
                let _ = event_tx.send(VisionEvent::ConnectionLost);
                return;
            }
            pending.insert(id, req.reply);
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if value.get("stream").is_some() {
                    if let Some(event) = crate::event_from_frame(&value) {
                        if event_tx.send(event).is_err() {
                            return;
                        }
                    }
                } else if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                    if let Some(reply) = pending.remove(&id) {
                        let result = if let Some(err) = value.get("error") {
                            Err(HubError::Api(
                                err.get("message")
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("unknown")
                                    .to_string(),
                            ))
                        } else {
                            Ok(value.get("result").cloned().unwrap_or(serde_json::Value::Null))
                        };
                        let _ = reply.send(result);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                let _ = event_tx.send(VisionEvent::ConnectionLost);
                return;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(err))
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => {
                let _ = event_tx.send(VisionEvent::ConnectionLost);
                return;
            }
        }
    }
}
