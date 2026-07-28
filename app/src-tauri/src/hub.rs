//! Vision-hub integration: reps-for-claude declares its model and workout
//! configs (resources/exercise_specs.json) and drives the bundled hub via
//! the `VisionHub` trait — enable metric on workout start, pump events into
//! the session, disable on completion. Honor-mode fallback when the hub is
//! down: never strand the user, never phantom reps.

use std::sync::Mutex;

use engine::clock::{Clock, SystemClock};
use engine::types::Progress;
use hub_client::{EnableMetric, HubSupervisor, HubSupervisorConfig, VisionEvent, VisionHub};
use tauri::{AppHandle, Emitter, Manager};

pub const EXERCISE_SPECS: &str = include_str!("../resources/exercise_specs.json");
pub const WORKOUT_METRIC: &str = "workout";

pub type SharedHub = Mutex<Option<Box<dyn VisionHub>>>;

/// Parse and sanity-check the shipped specs at startup: the model must be
/// declared and every exercise must name a known activity.
pub fn load_exercise_specs() -> Result<serde_json::Value, String> {
    let specs: serde_json::Value =
        serde_json::from_str(EXERCISE_SPECS).map_err(|e| format!("exercise_specs.json: {e}"))?;
    let model = specs
        .get("model")
        .and_then(|m| m.get("plugin"))
        .and_then(|p| p.as_str())
        .ok_or("exercise_specs.json: missing model.plugin declaration")?;
    if model.is_empty() {
        return Err("exercise_specs.json: empty model.plugin".into());
    }
    let exercises = specs
        .get("exercises")
        .and_then(|e| e.as_object())
        .ok_or("exercise_specs.json: missing exercises")?;
    for (name, entry) in exercises {
        let activity = entry.get("activity").and_then(|a| a.as_str()).unwrap_or("");
        if !matches!(activity, "lift" | "jumprope" | "stretch") {
            return Err(format!("exercise {name}: unknown activity {activity:?}"));
        }
    }
    Ok(specs)
}

/// The enable_metric config for a prescription: shipped exercise definition
/// + targets + camera selection.
pub fn metric_config_for(
    specs: &serde_json::Value,
    exercise: &str,
    target_reps: u32,
    target_seconds: f64,
) -> Option<(String, serde_json::Value)> {
    let plugin_id = specs["model"]["plugin"].as_str()?.to_string();
    let mut config = specs["exercises"].get(exercise)?.clone();
    let object = config.as_object_mut()?;
    if object.get("activity").and_then(|a| a.as_str()) == Some("lift") {
        object.insert("targetReps".into(), serde_json::json!(target_reps));
    } else if target_seconds > 0.0 {
        object.insert("targetSeconds".into(), serde_json::json!(target_seconds));
    }
    let camera_index: i64 = std::env::var("REPS_CAMERA_INDEX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    // The rig's webcam is mounted upside-down, and every exercise was tuned on
    // frames rotated 180° — so the app must rotate identically or the joint
    // angles are inverted and no rep crosses its thresholds. Overridable for
    // other setups via REPS_CAMERA_ROTATE (0/90/180/270).
    let camera_rotate: i64 = std::env::var("REPS_CAMERA_ROTATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(180);
    object.insert(
        "camera".into(),
        serde_json::json!({"source": "index", "value": camera_index, "id": "webcam", "rotate": camera_rotate}),
    );
    Some((plugin_id, config))
}

/// A camera-set declaration from exercise specs: registry entries to
/// register, the set to fuse across, and the fusion policy.
pub struct CameraSet {
    pub registry: Vec<serde_json::Value>,
    pub cameras: Vec<String>,
    /// Per-camera config deltas applied after enable (tuning overlays).
    pub overlays: Vec<(String, serde_json::Value)>,
}

/// When the specs declare a camera set, strip the injected single-camera
/// object, put the fusion policy into the config, and return the set.
pub fn apply_camera_set(
    specs: &serde_json::Value,
    config: &mut serde_json::Value,
) -> Option<CameraSet> {
    let block = specs.get("cameras")?;
    let cameras: Vec<String> = block
        .get("set")
        .and_then(|s| s.as_array())
        .map(|set| set.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if cameras.len() < 2 {
        eprintln!("hub: cameras.set needs >= 2 camera ids; falling back to single camera");
        return None;
    }
    let registry = block
        .get("registry")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let object = config.as_object_mut()?;
    object.remove("camera");
    object.insert(
        "fusion".into(),
        block
            .get("fusion")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"policy": "best", "scoreField": "visibility"})),
    );
    let overlays = block
        .get("overlays")
        .and_then(|o| o.as_object())
        .map(|map| {
            map.iter()
                .filter(|(camera_id, _)| cameras.contains(camera_id))
                .map(|(camera_id, delta)| (camera_id.clone(), delta.clone()))
                .collect()
        })
        .unwrap_or_default();
    Some(CameraSet {
        registry,
        cameras,
        overlays,
    })
}

/// Drive a hub through camera-set setup: register cameras, enable the
/// metric, then apply per-camera overlays. Extracted from the async enable
/// path so the ordering is unit-testable against the fake hub.
pub fn enable_on_hub(
    hub: &mut dyn hub_client::VisionHub,
    camera_set: &Option<CameraSet>,
    request: &EnableMetric,
) -> Result<(), hub_client::HubError> {
    if let Some(set) = camera_set {
        for camera in &set.registry {
            if let Err(err) = hub.add_camera(camera) {
                eprintln!("hub: add_camera failed: {err}");
            }
        }
    }
    hub.enable_metric(request)?;
    if let Some(set) = camera_set {
        for (camera_id, delta) in &set.overlays {
            if let Err(err) =
                hub.update_metric_config_for_camera(&request.metric_id, camera_id, delta)
            {
                eprintln!("hub: overlay for {camera_id} failed: {err}");
            }
        }
    }
    Ok(())
}

/// Start the supervisor in the background and pump its events into the
/// session. No-op when REPS_HUB_DISABLED is set (dev without a hub).
pub fn start(app: AppHandle) {
    if std::env::var("REPS_HUB_DISABLED").is_ok() {
        eprintln!("hub: disabled by REPS_HUB_DISABLED");
        return;
    }
    std::thread::Builder::new()
        .name("hub-start".into())
        .spawn(move || {
            let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .expect("repo root")
                .to_path_buf();
            let config = HubSupervisorConfig::dev(&repo_root);
            match HubSupervisor::start(config) {
                Ok(mut supervisor) => {
                    let receiver = supervisor.take_receiver();
                    *app.state::<SharedHub>().lock().unwrap() = Some(Box::new(supervisor));
                    let _ = app.emit("vision-event", serde_json::json!({"kind": "hub_up"}));
                    if let Some(rx) = receiver {
                        pump_events(app, rx);
                    }
                }
                Err(err) => {
                    eprintln!("hub: failed to start ({err}); honor mode only");
                    let _ = app.emit("vision-fallback", serde_json::json!({"reason": err.to_string()}));
                }
            }
        })
        .ok();
}

/// Print a terminal line when the camera gains/loses your pose (deduped — no
/// per-frame angle spam), so you can tell from the terminal whether you're in
/// frame while a set runs.
fn print_detect(data: &serde_json::Value) {
    static LAST: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);
    let pose = data.get("poseDetected").and_then(|v| v.as_bool()).unwrap_or(false);
    if let Ok(mut last) = LAST.lock() {
        if *last != Some(pose) {
            println!(
                "{}",
                if pose {
                    "[DETECT] 🟢 pose in frame — tracking"
                } else {
                    "[DETECT] 🔴 no pose in frame"
                }
            );
            *last = Some(pose);
        }
    }
}

fn pump_events(app: AppHandle, rx: std::sync::mpsc::Receiver<VisionEvent>) {
    for event in rx {
        match event {
            VisionEvent::Landmarks(data) => {
                print_detect(&data);
                let _ = app.emit("vision-landmarks", data);
            }
            VisionEvent::Progress { value, unit, satisfied } => {
                let core_state = app.state::<crate::SharedCore>();
                let mut core = core_state.lock().unwrap();
                core.session.report_progress(Progress { value, unit, satisfied });
                let snap = crate::persist_and_snapshot(&mut core);
                drop(core);
                crate::print_state(&snap);
                let _ = app.emit("snapshot", &snap);
                if satisfied {
                    disable_metric_async(&app);
                }
            }
            VisionEvent::Semantic { kind, payload } => {
                if kind == "rep_completed" || kind == "target_reached" {
                    println!("[DETECT] ✓ {}", kind);
                }
                let _ = app.emit(
                    "vision-event",
                    serde_json::json!({"kind": kind, "payload": payload}),
                );
            }
            VisionEvent::Health(health) => {
                let failed = health.vision_host == "down";
                let _ = app.emit(
                    "vision-event",
                    serde_json::json!({"kind": "health", "payload": {
                        "visionHost": health.vision_host,
                        "camera": health.camera,
                    }}),
                );
                if failed {
                    let _ = app.emit(
                        "vision-fallback",
                        serde_json::json!({"reason": "hub down after restart"}),
                    );
                }
            }
            VisionEvent::ConnectionLost => {
                let _ = app.emit(
                    "vision-event",
                    serde_json::json!({"kind": "connection_lost"}),
                );
            }
        }
    }
}

/// Enable the workout metric for the current prescription (fire-and-forget;
/// failures surface as vision-fallback so the UI offers honor mode).
pub fn enable_metric_async(app: &AppHandle, exercise: String, target_reps: u32, target_seconds: f64) {
    let app = app.clone();
    std::thread::spawn(move || {
        let specs = match load_exercise_specs() {
            Ok(specs) => specs,
            Err(err) => {
                eprintln!("hub: bad exercise specs: {err}");
                return;
            }
        };
        let Some((plugin_id, mut config)) =
            metric_config_for(&specs, &exercise, target_reps, target_seconds)
        else {
            eprintln!("hub: no spec for exercise {exercise}; honor mode");
            let _ = app.emit(
                "vision-fallback",
                serde_json::json!({"reason": format!("no spec for {exercise}")}),
            );
            return;
        };
        let camera_set = apply_camera_set(&specs, &mut config);
        let state = app.state::<SharedHub>();
        let mut guard = state.lock().unwrap();
        match guard.as_mut() {
            Some(hub) => {
                let request = EnableMetric {
                    metric_id: WORKOUT_METRIC.into(),
                    plugin_id,
                    cameras: camera_set.as_ref().map(|set| set.cameras.clone()),
                    config,
                };
                if let Err(err) = enable_on_hub(hub.as_mut(), &camera_set, &request) {
                    eprintln!("hub: enable_metric failed: {err}");
                    let _ = app.emit(
                        "vision-fallback",
                        serde_json::json!({"reason": err.to_string()}),
                    );
                }
            }
            None => {
                let _ = app.emit(
                    "vision-fallback",
                    serde_json::json!({"reason": "hub not running"}),
                );
            }
        }
    });
}

/// Disable the workout metric (camera released). Fire-and-forget.
pub fn disable_metric_async(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<SharedHub>();
        let mut guard = state.lock().unwrap();
        if let Some(hub) = guard.as_mut() {
            if let Err(err) = hub.disable_metric(WORKOUT_METRIC) {
                eprintln!("hub: disable_metric failed: {err}");
            }
        }
    });
}

/// Honor-mode completion: the camera path failed, the user pressed Done.
pub fn honor_complete(app: &AppHandle) -> engine::types::Snapshot {
    let clock = SystemClock;
    let core_state = app.state::<crate::SharedCore>();
    let mut core = core_state.lock().unwrap();
    core.session.mark_unverified();
    let (value, unit) = match core.session.snapshot(clock.now()).prescription {
        Some(rx) if rx.target_seconds > 0.0 => (rx.target_seconds, "seconds".to_string()),
        Some(rx) => (rx.target_reps as f64, "reps".to_string()),
        None => (0.0, "reps".to_string()),
    };
    core.session.report_progress(Progress { value, unit, satisfied: true });
    let snap = crate::persist_and_snapshot(&mut core);
    drop(core);
    let _ = app.emit("snapshot", &snap);
    disable_metric_async(app);
    snap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_specs_parse_and_declare_the_model() {
        let specs = load_exercise_specs().unwrap();
        assert_eq!(specs["model"]["plugin"], "reps_vision");
        assert_eq!(specs["model"]["name"], "mediapipe-pose-landmarker");
        assert!(specs["exercises"].get("squat").is_some());
    }

    #[test]
    fn metric_config_carries_exercise_target_and_camera() {
        let specs = load_exercise_specs().unwrap();
        let (plugin, config) = metric_config_for(&specs, "squat", 12, 0.0).unwrap();
        assert_eq!(plugin, "reps_vision");
        assert_eq!(config["activity"], "lift");
        // downBelow is the (camera-tuned) spec value, not a hardcoded constant.
        assert_eq!(
            config["exercise"]["downBelow"],
            specs["exercises"]["squat"]["exercise"]["downBelow"]
        );
        assert!(config["exercise"]["downBelow"].is_number());
        assert_eq!(config["targetReps"], 12);
        assert_eq!(config["camera"]["source"], "index");
        assert_eq!(config["camera"]["id"], "webcam");
        // The rig is mounted 180°; frames must be rotated to match the tuned
        // thresholds (env REPS_CAMERA_ROTATE overrides; default 180).
        let expected_rotate: i64 = std::env::var("REPS_CAMERA_ROTATE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(180);
        assert_eq!(config["camera"]["rotate"], expected_rotate);
    }

    #[test]
    fn camera_set_spec_overrides_single_camera_config() {
        let specs = serde_json::json!({
            "model": {"plugin": "reps_vision"},
            "cameras": {
                "registry": [
                    {"cameraId": "front", "kind": "usb", "source": "v4l2:///dev/video0"},
                    {"cameraId": "side", "kind": "usb", "source": "v4l2:///dev/video1"}
                ],
                "set": ["front", "side"],
                "fusion": {"policy": "best", "scoreField": "visibility"}
            },
            "exercises": {"squat": {"activity": "lift",
                "exercise": {"name": "squat", "joints": ["hip","knee","ankle"],
                              "downBelow": 110.0, "upAbove": 160.0}}}
        });
        let (_, mut config) = metric_config_for(&specs, "squat", 5, 0.0).unwrap();
        let set = apply_camera_set(&specs, &mut config).unwrap();
        assert_eq!(set.cameras, vec!["front", "side"]);
        assert_eq!(set.registry.len(), 2);
        // exclusive with the injected single-camera object
        assert!(config.get("camera").is_none());
        assert_eq!(config["fusion"]["scoreField"], "visibility");
    }

    #[test]
    fn enable_on_hub_registers_cameras_then_enables_then_applies_overlays() {
        let specs = serde_json::json!({
            "model": {"plugin": "reps_vision"},
            "cameras": {
                "registry": [
                    {"cameraId": "front", "kind": "usb", "source": "v4l2:///dev/video0"},
                    {"cameraId": "side", "kind": "usb", "source": "v4l2:///dev/video1"}
                ],
                "set": ["front", "side"],
                "fusion": {"policy": "best", "scoreField": "visibility"},
                "overlays": {"side": {"exercise": {"downBelow": 95.0}}}
            },
            "exercises": {"squat": {"activity": "lift",
                "exercise": {"name": "squat", "joints": ["hip","knee","ankle"],
                              "downBelow": 110.0, "upAbove": 160.0}}}
        });
        let (plugin_id, mut config) = metric_config_for(&specs, "squat", 5, 0.0).unwrap();
        let camera_set = apply_camera_set(&specs, &mut config);
        let mut hub = hub_client::fake::FakeHub::new(vec![]);
        enable_on_hub(
            &mut hub,
            &camera_set,
            &EnableMetric {
                metric_id: WORKOUT_METRIC.into(),
                plugin_id,
                cameras: camera_set.as_ref().map(|set| set.cameras.clone()),
                config,
            },
        )
        .unwrap();
        // registration precedes enable; overlays follow it
        assert_eq!(
            hub.calls,
            vec![
                "add_camera:front".to_string(),
                "add_camera:side".to_string(),
                format!("enable:{WORKOUT_METRIC}:reps_vision"),
                format!("update:{WORKOUT_METRIC}:side"),
            ]
        );
    }

    #[test]
    fn no_camera_set_leaves_single_camera_config_alone() {
        let specs = load_exercise_specs().unwrap();
        let (_, mut config) = metric_config_for(&specs, "squat", 5, 0.0).unwrap();
        assert!(apply_camera_set(&specs, &mut config).is_none());
        assert_eq!(config["camera"]["id"], "webcam");
    }

    #[test]
    fn metric_config_sets_duration_targets_for_continuous() {
        let specs = load_exercise_specs().unwrap();
        let (_, config) = metric_config_for(&specs, "jumprope", 0, 90.0).unwrap();
        assert_eq!(config["activity"], "jumprope");
        assert_eq!(config["targetSeconds"], 90.0);
    }

    #[test]
    fn unknown_exercise_yields_none() {
        let specs = load_exercise_specs().unwrap();
        assert!(metric_config_for(&specs, "wallsit", 5, 0.0).is_none());
    }
}
