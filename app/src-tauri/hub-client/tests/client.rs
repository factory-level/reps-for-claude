//! HubClient against a scripted mock hub server (real WebSocket, no tokio).

use std::net::TcpListener;
use std::time::Duration;

use hub_client::{EnableMetric, HubClient, HubError, VisionEvent, VisionHub};
use tungstenite::Message;

/// Start a mock hub: sends `hello`, then answers every request with
/// `responder(method, params) -> result-or-error`, interleaving any pushes
/// queued by the responder via the returned sender.
fn mock_hub(
    hello_version: &str,
    responder: impl Fn(&str, &serde_json::Value) -> Result<serde_json::Value, String> + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hello = serde_json::json!({"type": "hello", "apiVersion": hello_version, "hubVersion": "mock"});
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        ws.send(Message::Text(hello.to_string().into())).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match ws.read() {
                Ok(Message::Text(text)) => {
                    let msg: serde_json::Value = serde_json::from_str(&text).unwrap();
                    let id = msg["id"].clone();
                    let method = msg["method"].as_str().unwrap_or("");
                    let reply = match responder(method, &msg["params"]) {
                        Ok(result) => serde_json::json!({"id": id, "result": result}),
                        Err(message) => {
                            serde_json::json!({"id": id, "error": {"message": message}})
                        }
                    };
                    ws.send(Message::Text(reply.to_string().into())).unwrap();
                    // reply, then close: simulates hubd going away mid-session
                    if method == "simulate" && msg["params"]["event"]["type"] == "bye" {
                        let _ = ws.close(None);
                        let _ = ws.flush();
                        return;
                    }
                    // after an enable, push one of each stream frame
                    if method == "enable_metric" {
                        for frame in [
                            serde_json::json!({"stream": "landmarks", "pluginId": "p", "metricId": "m", "cameraId": "cam", "seq": 1, "tsUs": 1, "data": {"poseDetected": true, "angle": 90.0}}),
                            serde_json::json!({"stream": "progress", "pluginId": "p", "metricId": "m", "cameraId": "cam", "seq": 2, "tsUs": 2, "data": {"value": 1.0, "unit": "reps", "satisfied": false}}),
                            serde_json::json!({"stream": "event", "pluginId": "p", "metricId": "m", "cameraId": "cam", "seq": 3, "tsUs": 3, "data": {"type": "rep_completed", "count": 1}}),
                        ] {
                            ws.send(Message::Text(frame.to_string().into())).unwrap();
                        }
                    }
                }
                Ok(Message::Close(_)) => return,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => return,
            }
        }
    });
    (format!("ws://127.0.0.1:{port}/v1/ws"), handle)
}

fn recv_event(rx: &std::sync::mpsc::Receiver<VisionEvent>) -> VisionEvent {
    rx.recv_timeout(Duration::from_secs(5)).expect("event")
}

#[test]
fn connects_and_verifies_api_version() {
    let (url, _handle) = mock_hub("1.0", |_m, _p| Ok(serde_json::json!({})));
    let client = HubClient::connect(&url).unwrap();
    assert_eq!(client.api_version, "1.0");
}

#[test]
fn rejects_unsupported_api_major() {
    let (url, _handle) = mock_hub("2.0", |_m, _p| Ok(serde_json::json!({})));
    match HubClient::connect(&url) {
        Err(HubError::VersionMismatch { hub, .. }) => assert_eq!(hub, "2.0"),
        Err(other) => panic!("expected VersionMismatch, got {other:?}"),
        Ok(_) => panic!("expected VersionMismatch, got a connection"),
    }
}

#[test]
fn enable_metric_round_trip_and_stream_events() {
    let (url, _handle) = mock_hub("1.0", |method, params| {
        if method == "enable_metric" {
            assert_eq!(params["metricId"], "workout");
            assert_eq!(params["pluginId"], "reps_vision");
            assert_eq!(params["config"]["targetReps"], 2);
        }
        Ok(serde_json::json!({"ok": true}))
    });
    let mut client = HubClient::connect(&url).unwrap();
    let rx = client.take_receiver().unwrap();
    client
        .enable_metric(&EnableMetric {
            metric_id: "workout".into(),
            plugin_id: "reps_vision".into(),
            cameras: None,
            config: serde_json::json!({"targetReps": 2}),
        })
        .unwrap();

    match recv_event(&rx) {
        VisionEvent::Landmarks(data) => assert_eq!(data["angle"], 90.0),
        other => panic!("expected landmarks, got {other:?}"),
    }
    match recv_event(&rx) {
        VisionEvent::Progress { value, unit, satisfied } => {
            assert_eq!(value, 1.0);
            assert_eq!(unit, "reps");
            assert!(!satisfied);
        }
        other => panic!("expected progress, got {other:?}"),
    }
    match recv_event(&rx) {
        VisionEvent::Semantic { kind, payload } => {
            assert_eq!(kind, "rep_completed");
            assert_eq!(payload["count"], 1);
        }
        other => panic!("expected semantic, got {other:?}"),
    }
}

#[test]
fn api_errors_surface_as_hub_error() {
    let (url, _handle) = mock_hub("1.0", |method, _p| {
        if method == "disable_metric" {
            Err("unknown metric: nope".into())
        } else {
            Ok(serde_json::json!({}))
        }
    });
    let mut client = HubClient::connect(&url).unwrap();
    match client.disable_metric("nope") {
        Err(HubError::Api(message)) => assert!(message.contains("unknown metric")),
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[test]
fn server_close_emits_connection_lost() {
    let (url, _handle) = mock_hub("1.0", |_m, _p| Ok(serde_json::json!({})));
    let mut client = HubClient::connect(&url).unwrap();
    let rx = client.take_receiver().unwrap();
    client
        .simulate("ignored", &serde_json::json!({"type": "bye"}))
        .unwrap();
    let mut saw_lost = false;
    while let Ok(event) = rx.recv_timeout(Duration::from_secs(5)) {
        if matches!(event, VisionEvent::ConnectionLost) {
            saw_lost = true;
            break;
        }
    }
    assert!(saw_lost, "expected ConnectionLost after server close");
}
