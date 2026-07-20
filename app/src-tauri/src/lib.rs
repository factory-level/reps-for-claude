use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use engine::clock::{Clock, SystemClock};
use engine::session::Session;
use engine::store::{default_continuous_pool, Store};
use engine::timer::CodingTimer;
use engine::types::{Progress, Snapshot};
use engine::workout::WorkoutEngine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

struct Core {
    session: Session,
    store: Store,
}

type SharedCore = Mutex<Core>;

fn build_core() -> Core {
    let dir = dirs_next_data_dir();
    let store = Store::open(&dir.join("reps.sqlite")).expect("open sqlite");
    let work_minutes: f64 = store.setting("work_minutes", "6").parse().unwrap_or(6.0);
    let capacity: u32 = store.setting("max_weighted_sets", "20").parse().unwrap_or(20);
    let rotation = store.load_rotation().expect("load rotation");
    let mut workout = WorkoutEngine::new(rotation, default_continuous_pool(), capacity);
    let (pointer, used, date) = store.load_pointer_state().expect("load pointer");
    workout.restore(pointer, used, &date);
    let session = Session::new(CodingTimer::new(work_minutes * 60.0), workout);
    Core { session, store }
}

fn dirs_next_data_dir() -> std::path::PathBuf {
    std::env::var("REPS_APP_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            std::path::PathBuf::from(home).join(".local/share/reps-for-claude")
        })
}

fn persist_and_snapshot(core: &mut Core) -> Snapshot {
    let clock = SystemClock;
    if let Some(rec) = core.session.take_pending_record() {
        let _ = core.store.record_set(&rec);
    }
    let w = core.session.workout();
    let _ = core
        .store
        .save_pointer_state(w.pointer(), w.capacity_used(), w.capacity_date());
    core.session.snapshot(clock.now())
}

fn emit_snapshot(app: &AppHandle, snap: &Snapshot) {
    let _ = app.emit("snapshot", snap);
}

#[tauri::command]
fn get_snapshot(state: State<SharedCore>) -> Snapshot {
    let mut core = state.lock().unwrap();
    persist_and_snapshot(&mut core)
}

#[tauri::command]
fn start_session(app: AppHandle, state: State<SharedCore>) -> Snapshot {
    let clock = SystemClock;
    let mut core = state.lock().unwrap();
    core.session.start(clock.now(), &clock.today());
    let snap = persist_and_snapshot(&mut core);
    emit_snapshot(&app, &snap);
    snap
}

#[tauri::command]
fn begin_workout(app: AppHandle, state: State<SharedCore>) -> Snapshot {
    let mut core = state.lock().unwrap();
    core.session.begin_workout();
    let snap = persist_and_snapshot(&mut core);
    emit_snapshot(&app, &snap);
    snap
}

#[tauri::command]
fn simulate_progress(
    app: AppHandle,
    state: State<SharedCore>,
    value: f64,
    satisfied: bool,
) -> Snapshot {
    let mut core = state.lock().unwrap();
    let unit = "reps".to_string();
    core.session.report_progress(Progress { value, unit, satisfied });
    let snap = persist_and_snapshot(&mut core);
    emit_snapshot(&app, &snap);
    snap
}

#[tauri::command]
fn confirm_weight(app: AppHandle, state: State<SharedCore>, weight: f64) -> Snapshot {
    let clock = SystemClock;
    let mut core = state.lock().unwrap();
    if let Some(rec) = core.session.confirm_weight(weight, &clock.today()) {
        let _ = core.store.record_set(&rec);
        core.session.take_pending_record(); // already persisted
    }
    let snap = persist_and_snapshot(&mut core);
    emit_snapshot(&app, &snap);
    snap
}

#[tauri::command]
fn resume_coding(app: AppHandle, state: State<SharedCore>) -> Snapshot {
    let clock = SystemClock;
    let mut core = state.lock().unwrap();
    core.session.resume_coding(clock.now());
    let snap = persist_and_snapshot(&mut core);
    emit_snapshot(&app, &snap);
    snap
}

// ---------------------------------------------------------------------------
// Detection debug view (Task C): stream exercise fixture videos through the
// Python detector sidecar for live visual debugging.
// ---------------------------------------------------------------------------

/// Walk up from `start` until we find a directory containing a `vision`
/// subdirectory, returning that subdirectory.
///
/// `start` is meant to be `CARGO_MANIFEST_DIR` (compile-time, `app/src-tauri`)
/// rather than the process's runtime cwd: `tauri dev`, `cargo check`, and the
/// test binary can all be invoked from different working directories, but
/// `CARGO_MANIFEST_DIR` is stable for a given checkout. Two levels up from
/// `app/src-tauri` is the repo root, which contains `vision/`. This embeds
/// the build machine's source path, which is fine for this dev-only debug
/// feature but would not be appropriate for an end-user installer build.
fn find_vision_dir(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join("vision");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

fn vision_dir() -> Result<PathBuf, String> {
    find_vision_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .ok_or_else(|| "could not locate vision/ directory".to_string())
}

#[derive(Serialize)]
struct DebugVideo {
    exercise: String,
    path: String,
}

/// Tracked fixture + youtube-manifest entries, read at request time.
fn list_debug_videos(vision_dir: &Path) -> Vec<DebugVideo> {
    let fixtures = vision_dir.join("tests/fixtures/videos");
    let mut out = Vec::new();

    let squat_demo = fixtures.join("squat_demo.webm");
    out.push(DebugVideo {
        exercise: "squat".to_string(),
        path: squat_demo
            .canonicalize()
            .unwrap_or(squat_demo)
            .to_string_lossy()
            .to_string(),
    });

    let manifest_path = fixtures.join("youtube/youtube_manifest.json");
    if let Ok(text) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(map) = manifest.as_object() {
                let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
                entries.sort_by_key(|(exercise, _)| exercise.as_str());
                for (exercise, entry) in entries {
                    let Some(file) = entry.get("file").and_then(|f| f.as_str()) else {
                        continue;
                    };
                    let path = fixtures.join("youtube").join(file);
                    out.push(DebugVideo {
                        exercise: exercise.clone(),
                        path: path.canonicalize().unwrap_or(path).to_string_lossy().to_string(),
                    });
                }
            }
        }
    }
    out
}

#[tauri::command]
fn debug_videos() -> Result<Vec<DebugVideo>, String> {
    Ok(list_debug_videos(&vision_dir()?))
}

/// Managed state for the currently-running debug sidecar child, kept
/// separate from `SharedCore` since it guards an OS process handle rather
/// than app/session data. `generation` disambiguates a reader thread's own
/// (possibly stale) child from one belonging to a newer `debug_stream_start`
/// call, so a slow-to-exit old process can never be reaped/waited-on twice
/// or mistaken for the new one.
struct DebugProcess {
    generation: u64,
    child: Option<Child>,
}

type SharedDebugProcess = Mutex<DebugProcess>;

const DEBUG_STREAM_EVENT: &str = "debug-stream";
const DEBUG_JPEG_EVERY: &str = "5";

#[tauri::command]
fn debug_stream_start(
    app: AppHandle,
    state: State<SharedDebugProcess>,
    video: String,
    exercise: String,
) -> Result<(), String> {
    let vision_dir = vision_dir()?;

    let my_generation = {
        let mut guard = state.lock().unwrap();
        if let Some(child) = guard.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        guard.generation += 1;
        guard.child = None;
        guard.generation
    };

    let mut child = Command::new("uv")
        .args([
            "run",
            "python",
            "-m",
            "reps_vision.stream",
            "--video",
            &video,
            "--exercise",
            &exercise,
            "--jpeg-every",
            DEBUG_JPEG_EVERY,
        ])
        .current_dir(&vision_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn debug sidecar: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "debug sidecar has no stdout pipe".to_string())?;

    {
        let mut guard = state.lock().unwrap();
        guard.child = Some(child);
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(value) => {
                    let _ = handle.emit(DEBUG_STREAM_EVENT, value);
                }
                Err(_) => {
                    let _ = handle.emit(
                        DEBUG_STREAM_EVENT,
                        serde_json::json!({"event": "error", "message": trimmed}),
                    );
                }
            }
        }

        let debug_state = handle.state::<SharedDebugProcess>();
        let mut guard = debug_state.lock().unwrap();
        if guard.generation != my_generation {
            // Superseded by a newer debug_stream_start call; that stream owns
            // reaping/emitting now, so stay quiet to avoid a stale "exited".
            return;
        }
        let code = guard
            .child
            .as_mut()
            .and_then(|c| c.wait().ok())
            .and_then(|status| status.code());
        guard.child = None;
        drop(guard);
        let _ = handle.emit(DEBUG_STREAM_EVENT, serde_json::json!({"event": "exited", "code": code}));
    });

    Ok(())
}

#[tauri::command]
fn debug_stream_stop(state: State<SharedDebugProcess>) -> Result<(), String> {
    let mut guard = state.lock().unwrap();
    if let Some(child) = guard.child.as_mut() {
        child.kill().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(Mutex::new(build_core()) as SharedCore)
        .manage(Mutex::new(DebugProcess {
            generation: 0,
            child: None,
        }) as SharedDebugProcess)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            start_session,
            begin_workout,
            simulate_progress,
            confirm_weight,
            resume_coding,
            debug_videos,
            debug_stream_start,
            debug_stream_stop
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let clock = SystemClock;
                let state = handle.state::<SharedCore>();
                let mut core = state.lock().unwrap();
                core.session.tick(clock.now(), &clock.today());
                let snap = core.session.snapshot(clock.now());
                drop(core);
                let _ = handle.emit("snapshot", &snap);
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // Make sure the debug sidecar never survives the app: on shutdown,
        // kill any child still tracked in SharedDebugProcess.
        if let RunEvent::ExitRequested { .. } = event {
            let state = handle.state::<SharedDebugProcess>();
            let mut guard = state.lock().unwrap();
            if let Some(child) = guard.child.as_mut() {
                let _ = child.kill();
            }
        }
    });
}

#[cfg(test)]
mod debug_view_tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reps-debug-view-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn find_vision_dir_walks_up_to_repo_root() {
        let root = temp_dir("walk-up");
        let vision = root.join("vision");
        fs::create_dir_all(&vision).unwrap();
        let start = root.join("app").join("src-tauri");
        fs::create_dir_all(&start).unwrap();

        assert_eq!(find_vision_dir(&start), Some(vision));
    }

    #[test]
    fn find_vision_dir_returns_none_when_absent() {
        let root = temp_dir("absent");
        let start = root.join("app").join("src-tauri");
        fs::create_dir_all(&start).unwrap();

        assert_eq!(find_vision_dir(&start), None);
    }

    #[test]
    fn list_debug_videos_includes_tracked_fixture_and_manifest_entries() {
        let root = temp_dir("videos");
        let vision = root.join("vision");
        let fixtures = vision.join("tests/fixtures/videos");
        let youtube = fixtures.join("youtube");
        fs::create_dir_all(&youtube).unwrap();
        fs::write(fixtures.join("squat_demo.webm"), b"fake").unwrap();
        fs::write(youtube.join("bench.mp4"), b"fake").unwrap();
        fs::write(
            youtube.join("youtube_manifest.json"),
            r#"{"bench": {"file": "bench.mp4", "url": "u", "title": "t"}}"#,
        )
        .unwrap();

        let videos = list_debug_videos(&vision);
        let exercises: Vec<&str> = videos.iter().map(|v| v.exercise.as_str()).collect();
        assert_eq!(exercises, vec!["squat", "bench"]);
        assert!(videos[0].path.ends_with("squat_demo.webm"));
        assert!(videos[1].path.ends_with("bench.mp4"));
    }

    #[test]
    fn list_debug_videos_without_manifest_has_only_tracked_fixture() {
        let root = temp_dir("no-manifest");
        let vision = root.join("vision");
        let fixtures = vision.join("tests/fixtures/videos");
        fs::create_dir_all(&fixtures).unwrap();
        fs::write(fixtures.join("squat_demo.webm"), b"fake").unwrap();

        let videos = list_debug_videos(&vision);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].exercise, "squat");
    }
}
