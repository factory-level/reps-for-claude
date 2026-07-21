//! Client for the usb-mcp-hub vision SDK (client API v1).
//!
//! The engine crate stays hub-free: `lib.rs` in the app crate pumps
//! [`VisionEvent`]s from a [`VisionHub`] implementation into the session
//! state machine. `FakeHub` serves tests; `HubClient` speaks the real
//! WebSocket API; `HubSupervisor` owns the bundled hubd process.

use std::sync::mpsc;

pub mod client;
pub mod fake;
pub mod supervisor;

pub use client::HubClient;
pub use fake::FakeHub;
pub use supervisor::{HubSupervisor, HubSupervisorConfig};

pub const SUPPORTED_API_MAJOR: &str = "1";

#[derive(Debug, Clone, PartialEq)]
pub enum VisionEvent {
    /// Full landmark frame for the UI overlay (passed through untyped).
    Landmarks(serde_json::Value),
    /// Live activity progress — maps 1:1 onto `engine::types::Progress`.
    Progress {
        value: f64,
        unit: String,
        satisfied: bool,
    },
    /// Semantic events: rep_completed, target_reached, …
    Semantic {
        kind: String,
        payload: serde_json::Value,
    },
    Health(HubHealth),
    ConnectionLost,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct HubHealth {
    #[serde(rename = "visionHost")]
    pub vision_host: String,
    pub camera: String,
    #[serde(rename = "enabledMetrics")]
    pub enabled_metrics: Vec<String>,
}

impl HubHealth {
    pub fn failed() -> Self {
        HubHealth {
            vision_host: "down".into(),
            camera: "closed".into(),
            enabled_metrics: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnableMetric {
    pub metric_id: String,
    pub plugin_id: String,
    pub config: serde_json::Value,
}

#[derive(Debug)]
pub enum HubError {
    Io(String),
    Api(String),
    VersionMismatch { hub: String, supported: String },
    Timeout(String),
    Down,
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HubError::Io(msg) => write!(f, "hub io error: {msg}"),
            HubError::Api(msg) => write!(f, "hub api error: {msg}"),
            HubError::VersionMismatch { hub, supported } => {
                write!(f, "hub api version {hub} unsupported (need {supported}.x)")
            }
            HubError::Timeout(what) => write!(f, "hub timeout: {what}"),
            HubError::Down => write!(f, "hub is down"),
        }
    }
}

impl std::error::Error for HubError {}

/// The vision backend the reps core drives. Lock → `enable_metric`,
/// unlock/abort → `disable_metric`; events arrive on the receiver.
pub trait VisionHub: Send {
    fn enable_metric(&mut self, req: &EnableMetric) -> Result<(), HubError>;
    fn disable_metric(&mut self, metric_id: &str) -> Result<(), HubError>;
    fn update_metric_config(
        &mut self,
        metric_id: &str,
        config: &serde_json::Value,
    ) -> Result<(), HubError>;
    fn simulate(&mut self, metric_id: &str, event: &serde_json::Value) -> Result<(), HubError>;
    fn health(&mut self) -> Result<HubHealth, HubError>;
    /// The event stream; `None` after the first call.
    fn take_receiver(&mut self) -> Option<mpsc::Receiver<VisionEvent>>;
}

/// Convert a client-API push frame into a `VisionEvent`.
pub(crate) fn event_from_frame(frame: &serde_json::Value) -> Option<VisionEvent> {
    let stream = frame.get("stream")?.as_str()?;
    let data = frame.get("data").cloned().unwrap_or(serde_json::Value::Null);
    match stream {
        "landmarks" => Some(VisionEvent::Landmarks(data)),
        "progress" => Some(VisionEvent::Progress {
            value: data.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0),
            unit: data
                .get("unit")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            satisfied: data
                .get("satisfied")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        "event" => Some(VisionEvent::Semantic {
            kind: data
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            payload: data,
        }),
        "health" => serde_json::from_value(data).ok().map(VisionEvent::Health),
        _ => None,
    }
}
