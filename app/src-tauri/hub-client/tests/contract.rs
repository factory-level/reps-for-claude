//! Replays the vendored v1 contract transcript against HubClient: a mock
//! server walks the choreography, asserting every client-to-server frame
//! matches the transcript and pushing every server-to-client frame verbatim.
//! The canonical copy lives in usb-mcp-hub (apps/hubd/test/contracts/v1);
//! regenerate there, re-vendor here.

use std::net::TcpListener;
use std::time::Duration;

use hub_client::{EnableMetric, HubClient, VisionEvent, VisionHub};
use tungstenite::Message;

const TRANSCRIPT: &str = include_str!("contracts/v1/workout-session.json");

#[derive(serde::Deserialize)]
struct Transcript {
    steps: Vec<Step>,
}

#[derive(serde::Deserialize)]
struct Step {
    dir: String,
    msg: serde_json::Value,
}

#[test]
fn client_follows_the_v1_workout_transcript() {
    let transcript: Transcript = serde_json::from_str(TRANSCRIPT).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let mut last_id = serde_json::Value::Null;
        for step in &transcript.steps {
            match step.dir.as_str() {
                "s2c" => {
                    let frame = if let Some(reply) = step.msg.get("reply") {
                        serde_json::json!({"id": last_id, "result": reply})
                    } else {
                        step.msg.clone()
                    };
                    ws.send(Message::Text(frame.to_string().into())).unwrap();
                }
                "c2s" => loop {
                    match ws.read().unwrap() {
                        Message::Text(text) => {
                            let received: serde_json::Value =
                                serde_json::from_str(&text).unwrap();
                            last_id = received["id"].clone();
                            assert_eq!(
                                received["method"], step.msg["method"],
                                "wrong method order"
                            );
                            assert_eq!(
                                received["params"], step.msg["params"],
                                "params differ from transcript for {}",
                                step.msg["method"]
                            );
                            break;
                        }
                        Message::Close(_) => panic!("client closed early"),
                        _ => {}
                    }
                },
                other => panic!("bad transcript dir: {other}"),
            }
        }
        let _ = ws.close(None);
        let _ = ws.flush();
    });

    // Drive the client through the same choreography.
    let mut client = HubClient::connect(&format!("ws://127.0.0.1:{port}/")).unwrap();
    let rx = client.take_receiver().unwrap();
    client
        .enable_metric(&EnableMetric {
            metric_id: "workout".into(),
            plugin_id: "reps_vision".into(),
            cameras: None,
            config: serde_json::json!({
                "activity": "lift",
                "targetReps": 2,
                "exercise": {"name": "squat", "joints": ["hip", "knee", "ankle"],
                              "downBelow": 110, "upAbove": 160},
                "camera": {"source": "index", "value": 0, "id": "cam-desk"},
            }),
        })
        .unwrap();

    let mut events = vec![];
    for _ in 0..3 {
        events.push(rx.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    assert!(matches!(events[0], VisionEvent::Landmarks(_)));
    assert!(
        matches!(&events[1], VisionEvent::Progress { value, unit, satisfied }
            if *value == 1.0 && unit == "reps" && !satisfied)
    );
    assert!(
        matches!(&events[2], VisionEvent::Semantic { kind, .. } if kind == "rep_completed")
    );

    client.disable_metric("workout").unwrap();
    server.join().unwrap();
}

const TRANSCRIPT_V1_3: &str = include_str!("contracts/v1.3/camera-set.json");

/// v1.3 camera-set choreography: subscribe happens inside connect(), then
/// two add_camera calls, a camera-set enable, and a fused landmarks push.
#[test]
fn client_follows_the_v1_3_camera_set_transcript() {
    let transcript: Transcript = serde_json::from_str(TRANSCRIPT_V1_3).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let mut last_id = serde_json::Value::Null;
        for step in &transcript.steps {
            match step.dir.as_str() {
                "s2c" => {
                    let frame = if let Some(reply) = step.msg.get("reply") {
                        serde_json::json!({"id": last_id, "result": reply})
                    } else {
                        step.msg.clone()
                    };
                    ws.send(Message::Text(frame.to_string().into())).unwrap();
                }
                "c2s" => loop {
                    match ws.read().unwrap() {
                        Message::Text(text) => {
                            let received: serde_json::Value =
                                serde_json::from_str(&text).unwrap();
                            last_id = received["id"].clone();
                            assert_eq!(
                                received["method"], step.msg["method"],
                                "wrong method order"
                            );
                            assert_eq!(
                                received["params"], step.msg["params"],
                                "params differ from transcript for {}",
                                step.msg["method"]
                            );
                            break;
                        }
                        Message::Close(_) => panic!("client closed early"),
                        _ => {}
                    }
                },
                other => panic!("bad transcript dir: {other}"),
            }
        }
        let _ = ws.close(None);
        let _ = ws.flush();
    });

    let mut client = HubClient::connect(&format!("ws://127.0.0.1:{port}/")).unwrap();
    let rx = client.take_receiver().unwrap();
    client
        .add_camera(&serde_json::json!({
            "cameraId": "front", "kind": "usb", "source": "v4l2:///dev/video0"
        }))
        .unwrap();
    client
        .add_camera(&serde_json::json!({
            "cameraId": "side", "kind": "usb", "source": "v4l2:///dev/video1"
        }))
        .unwrap();
    client
        .enable_metric(&EnableMetric {
            metric_id: "workout".into(),
            plugin_id: "reps".into(),
            cameras: Some(vec!["front".into(), "side".into()]),
            config: serde_json::json!({
                "fusion": {"policy": "best", "scoreField": "visibility"},
            }),
        })
        .unwrap();

    let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event, VisionEvent::Landmarks(_)));

    server.join().unwrap();
}

const TRANSCRIPT_V1_4: &str = include_str!("contracts/v1.4/reps-application.json");

/// v1.4 FieldLab application choreography: subscribe inside connect(), then
/// register_application, publish_event, publish_command, report_action_result
/// — all with client-supplied ids so replies are deterministic.
#[test]
fn client_follows_the_v1_4_application_transcript() {
    let transcript: Transcript = serde_json::from_str(TRANSCRIPT_V1_4).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let mut last_id = serde_json::Value::Null;
        for step in &transcript.steps {
            match step.dir.as_str() {
                "s2c" => {
                    let frame = if let Some(reply) = step.msg.get("reply") {
                        serde_json::json!({"id": last_id, "result": reply})
                    } else {
                        step.msg.clone()
                    };
                    ws.send(Message::Text(frame.to_string().into())).unwrap();
                }
                "c2s" => loop {
                    match ws.read().unwrap() {
                        Message::Text(text) => {
                            let received: serde_json::Value =
                                serde_json::from_str(&text).unwrap();
                            last_id = received["id"].clone();
                            assert_eq!(
                                received["method"], step.msg["method"],
                                "wrong method order"
                            );
                            assert_eq!(
                                received["params"], step.msg["params"],
                                "params differ from transcript for {}",
                                step.msg["method"]
                            );
                            break;
                        }
                        Message::Close(_) => panic!("client closed early"),
                        _ => {}
                    }
                },
                other => panic!("bad transcript dir: {other}"),
            }
        }
        let _ = ws.close(None);
        let _ = ws.flush();
    });

    let mut client = HubClient::connect(&format!("ws://127.0.0.1:{port}/")).unwrap();
    client
        .register_application(
            "reps",
            "0.1.0",
            &serde_json::json!({
                "events": {
                    "rep_completed": {"schemaVersion": "1"},
                    "workout_completed": {"schemaVersion": "1"},
                },
                "commands": {
                    "desktop.lock_requested": {"schemaVersion": "1", "exposed": false},
                },
            }),
        )
        .unwrap();
    client
        .publish_event(&serde_json::json!({
            "appId": "reps", "type": "rep_completed", "id": "evt-1",
            "sessionId": "sess-1", "payload": {"count": 1},
        }))
        .unwrap();
    client
        .publish_command(&serde_json::json!({
            "appId": "reps", "type": "desktop.lock_requested", "id": "cmd-1",
            "sessionId": "sess-1", "payload": {"reason": "workout"},
        }))
        .unwrap();
    client
        .report_action_result(&serde_json::json!({
            "appId": "reps", "type": "desktop.lock_succeeded", "id": "res-1",
            "commandId": "cmd-1", "status": "succeeded", "sessionId": "sess-1",
            "payload": {},
        }))
        .unwrap();
    server.join().unwrap();
}
