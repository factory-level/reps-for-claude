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
