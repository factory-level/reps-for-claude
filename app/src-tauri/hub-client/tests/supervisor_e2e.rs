//! Real-hub e2e for the supervisor (ignored by default: needs the sibling
//! usb-mcp-hub checkout, pnpm, uv, and free ports 8443/8081).
//! Run: cargo test -p hub-client -- --ignored

use std::time::Duration;

use hub_client::{EnableMetric, HubSupervisor, HubSupervisorConfig, VisionEvent, VisionHub};

#[test]
#[ignore]
fn supervisor_runs_a_real_workout_metric_over_a_fixture() {
    // CARGO_MANIFEST_DIR is app/src-tauri/hub-client; repo root is 3 up.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let fixture = repo_root.join("vision/tests/fixtures/videos/squat_demo.webm");
    assert!(fixture.exists(), "fixture missing: {}", fixture.display());

    let mut supervisor = HubSupervisor::start(HubSupervisorConfig::dev(&repo_root)).unwrap();
    let rx = supervisor.take_receiver().unwrap();

    let health = supervisor.health().unwrap();
    assert_eq!(health.vision_host, "up");

    supervisor
        .enable_metric(&EnableMetric {
            metric_id: "e2e".into(),
            plugin_id: "reps_vision".into(),
            config: serde_json::json!({
                "activity": "lift",
                "targetReps": 2,
                "exercise": {"name": "squat", "joints": ["hip", "knee", "ankle"],
                              "downBelow": 110, "upAbove": 160},
                "camera": {"source": "file", "value": fixture.display().to_string(), "id": "fixture"},
            }),
        })
        .unwrap();

    let mut saw_landmarks = false;
    let mut reps = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(VisionEvent::Landmarks(_)) => saw_landmarks = true,
            Ok(VisionEvent::Semantic { kind, .. }) => {
                if kind == "rep_completed" {
                    reps += 1;
                }
                if kind == "target_reached" || kind == "stream_ended" {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = supervisor.disable_metric("e2e");
    supervisor.stop();

    assert!(saw_landmarks, "no landmark events reached the Rust client");
    assert_eq!(reps, 2, "expected the fixture's 2 squats");
}
