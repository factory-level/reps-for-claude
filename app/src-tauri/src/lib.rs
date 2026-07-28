use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use engine::clock::{Clock, SystemClock};
use engine::plan::DailyPlan;
use engine::session::Session;
use engine::store::{default_continuous_pool, Store};
use engine::timer::CodingTimer;
use engine::types::{ExerciseKind, Phase, Progress, Snapshot};
use engine::workout::WorkoutEngine;
use serde::{Deserialize, Serialize};

/// The daily routine, bundled at build time (like exercise_specs.json).
const ROUTINE_JSON: &str = include_str!("../resources/routine.json");

/// Persisted per-item completion so the day survives app restarts.
#[derive(Serialize, Deserialize, Default)]
struct PlanState {
    date: String,
    done: Vec<(String, u32)>,
}
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};

mod hub;

pub(crate) struct Core {
    pub(crate) session: Session,
    store: Store,
}

pub(crate) type SharedCore = Mutex<Core>;

fn build_core() -> Core {
    let dir = dirs_next_data_dir();
    let store = Store::open(&dir.join("reps.sqlite")).expect("open sqlite");
    // REPS_WORK_MINUTES overrides the coding timer for testing (e.g. 0.15 = a
    // ~9s countdown so you can reach a locked set immediately); otherwise the
    // persisted setting (default 6 min).
    let work_minutes: f64 = std::env::var("REPS_WORK_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| store.setting("work_minutes", "6").parse().unwrap_or(6.0));
    let capacity: u32 = store.setting("max_weighted_sets", "20").parse().unwrap_or(20);
    let rotation = store.load_rotation().expect("load rotation");
    let mut workout = WorkoutEngine::new(rotation, default_continuous_pool(), capacity);
    let (pointer, used, date) = store.load_pointer_state().expect("load pointer");
    workout.restore(pointer, used, &date);
    let mut session = Session::new(CodingTimer::new(work_minutes * 60.0), workout);

    // Drive prescription from the routine.json daily plan (falls back to the
    // rotation if the routine fails to parse). Restore today's completion.
    let today = SystemClock.today();
    match DailyPlan::from_routine_json(ROUTINE_JSON, &today) {
        Ok(mut plan) => {
            if let Ok(ps) = serde_json::from_str::<PlanState>(&store.setting("plan_state", "")) {
                if !ps.date.is_empty() {
                    plan.restore(&ps.date, &ps.done);
                    plan.roll_date(&today); // clears completion if it's a new day
                }
            }
            session.set_plan(plan);
        }
        Err(e) => eprintln!("routine.json failed to load, using rotation: {e}"),
    }

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

pub(crate) fn persist_and_snapshot(core: &mut Core) -> Snapshot {
    let clock = SystemClock;
    if let Some(rec) = core.session.take_pending_record() {
        match core.store.record_set(&rec) {
            Ok(()) => {}
            Err(e) => eprintln!("failed to record set: {e}"),
        }
    }
    let w = core.session.workout();
    match core
        .store
        .save_pointer_state(w.pointer(), w.capacity_used(), w.capacity_date())
    {
        Ok(()) => {}
        Err(e) => eprintln!("failed to save pointer state: {e}"),
    }
    // Persist the daily-plan completion so today's progress survives a restart.
    if let Some(plan) = core.session.plan() {
        let ps = PlanState { date: plan.date().to_string(), done: plan.done_state() };
        if let Ok(json) = serde_json::to_string(&ps) {
            if let Err(e) = core.store.set_setting("plan_state", &json) {
                eprintln!("failed to save plan state: {e}");
            }
        }
    }
    core.session.snapshot(clock.now())
}

/// Print a concise, human-readable line of the current session state to the
/// terminal (stdout) whenever it changes — so you can read your state while the
/// app runs without watching the webview.
pub(crate) fn print_state(snap: &Snapshot) {
    static LAST: Mutex<String> = Mutex::new(String::new());
    let rx = snap.prescription.as_ref();
    let ex = rx.map(|r| r.exercise.as_str()).unwrap_or("—");
    let target = rx
        .map(|r| match r.kind {
            ExerciseKind::Continuous => format!("{:.0}s", r.target_seconds),
            ExerciseKind::Rep => format!("{} reps", r.target_reps),
        })
        .unwrap_or_default();
    let value = snap.progress.as_ref().map(|p| p.value.round() as i64).unwrap_or(0);
    let line = match snap.phase {
        Phase::Coding => {
            let rem = snap.remaining_seconds.max(0.0) as u64;
            let next = snap.rotation.get(snap.pointer).map(|s| s.as_str()).unwrap_or("—");
            format!("[STATE] ⌨️  CODING · {}:{:02} left · next: {}", rem / 60, rem % 60, next)
        }
        Phase::ExerciseRequired => format!("[STATE] 🔒 LOCKED · do {} · {}", ex, target),
        Phase::WorkoutActive => format!("[STATE] 🏋️  ACTIVE · {} · {} / {}", ex, value, target),
        Phase::WeightConfirmation => format!(
            "[STATE] ⚖️  CONFIRM WEIGHT · {} · {:.0} lbs",
            ex,
            rx.map(|r| r.default_weight).unwrap_or(0.0)
        ),
        Phase::Unlocked => "[STATE] ✅ UNLOCKED — set complete".to_string(),
    };
    if let Ok(mut last) = LAST.lock() {
        if *last != line {
            println!("{line}");
            *last = line;
        }
    }
}

pub(crate) fn emit_snapshot(app: &AppHandle, snap: &Snapshot) {
    print_state(snap);
    // Durable history: phase changes publish their reps.* events through the
    // hub (fire-and-forget; queues while the hub is down).
    hub::publish_phase_transition(snap);
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
    drop(core);
    // Camera on only while working out: enable the metric for this
    // prescription (fire-and-forget; failure surfaces honor mode).
    if let Some(rx) = snap.prescription.as_ref() {
        hub::enable_metric_async(&app, rx.exercise.clone(), rx.target_reps, rx.target_seconds);
    }
    emit_snapshot(&app, &snap);
    snap
}

/// Honor-mode completion: camera path failed, the user attests the set.
#[tauri::command]
fn honor_complete(app: AppHandle) -> Snapshot {
    hub::honor_complete(&app)
}

/// DEBUG: force the session between "coding" and "workout" without waiting out
/// the coding timer, and flip the camera to match. Lets you exercise live
/// detection on demand from the debug toggle.
#[tauri::command]
fn debug_mode(app: AppHandle, state: State<SharedCore>, mode: String) -> Snapshot {
    let clock = SystemClock;
    let mut core = state.lock().unwrap();
    let workout = mode == "workout";
    if workout {
        core.session.debug_force_workout(clock.now(), &clock.today());
    } else {
        core.session.debug_force_coding(clock.now());
    }
    let snap = persist_and_snapshot(&mut core);
    drop(core);
    if workout {
        if let Some(rx) = snap.prescription.as_ref() {
            hub::enable_metric_async(&app, rx.exercise.clone(), rx.target_reps, rx.target_seconds);
        }
    } else {
        hub::disable_metric_async(&app);
    }
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
        match core.store.record_set(&rec) {
            Ok(()) => {}
            Err(e) => eprintln!("failed to record set: {e}"),
        }
        core.session.take_pending_record(); // already persisted
        hub::queue_event(
            "weight_logged",
            serde_json::json!({"exercise": rec.exercise, "weight": weight}),
        );
    }
    let snap = persist_and_snapshot(&mut core);
    drop(core);
    // Camera gating: confirming the weight ENDS the workout, so it must
    // release the camera like every other completion path. Found live: a
    // simulated/early completion left the detector running (and counting)
    // after desktop_unlocked because only the hub-satisfied path disabled.
    hub::disable_metric_async(&app);
    emit_snapshot(&app, &snap);
    snap
}

#[tauri::command]
fn resume_coding(app: AppHandle, state: State<SharedCore>) -> Snapshot {
    let clock = SystemClock;
    let mut core = state.lock().unwrap();
    core.session.resume_coding(clock.now());
    let snap = persist_and_snapshot(&mut core);
    drop(core);
    // Belt and braces: the metric is disabled on satisfaction already, but
    // an aborted workout must also release the camera.
    hub::disable_metric_async(&app);
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
const DEBUG_TARGET: &str = "10"; // matches WorkoutEngine's default_reps

/// Decide whether a just-spawned child belonging to `my_generation` should be
/// stored in `DebugProcess`, given the guard's generation at the moment the
/// spawning call re-acquires the lock after `Command::spawn()` returns.
///
/// Spawning happens without the lock held (it can block), so a second, later
/// `debug_stream_start` call can race ahead, kill/replace the tracked child,
/// and bump the generation before the first call gets back to storing its
/// own child. If that happened, `current_generation` will have moved past
/// `my_generation` and the newly spawned child is already orphaned: it must
/// be killed rather than stored, or it would silently replace (and leak) the
/// newer call's child handle.
fn should_store_spawn(current_generation: u64, my_generation: u64) -> bool {
    current_generation == my_generation
}

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
            "--extra",
            "cv",
            "python",
            "-m",
            "reps_vision.stream",
            "--video",
            &video,
            "--exercise",
            &exercise,
            "--jpeg-every",
            DEBUG_JPEG_EVERY,
            "--target",
            DEBUG_TARGET,
        ])
        .current_dir(&vision_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn debug sidecar: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "debug sidecar has no stdout pipe".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "debug sidecar has no stderr pipe".to_string())?;

    {
        let mut guard = state.lock().unwrap();
        if should_store_spawn(guard.generation, my_generation) {
            guard.child = Some(child);
        } else {
            // Superseded by a newer debug_stream_start call that raced ahead
            // while spawn() was blocking. That newer call already owns
            // `guard.child`; kill and reap this now-orphaned child instead of
            // overwriting the newer handle or leaking the process.
            drop(guard);
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    // Last ~20 stderr lines from the sidecar, shared between the stderr
    // reader thread (below) and the stdout reader thread's exit handling, so
    // a sidecar crash (e.g. missing mediapipe/opencv) surfaces its actual
    // error instead of just a bare exit code. We report the tail bundled
    // into the "exited" event (`{"event":"exited","code":...,"stderrTail":[...]}`)
    // rather than forwarding a live stderr stream, since stderr chatter is
    // only actionable once the process has stopped.
    const STDERR_TAIL_LEN: usize = 20;
    let stderr_tail: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_LEN)));

    let stderr_tail_writer = stderr_tail.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let mut tail = stderr_tail_writer.lock().unwrap();
            if tail.len() >= STDERR_TAIL_LEN {
                tail.pop_front();
            }
            tail.push_back(line);
        }
    });

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
        let tail: Vec<String> = stderr_tail.lock().unwrap().iter().cloned().collect();
        let _ = handle.emit(
            DEBUG_STREAM_EVENT,
            serde_json::json!({"event": "exited", "code": code, "stderrTail": tail}),
        );
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
        .manage(Mutex::new(None) as hub::SharedHub)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            start_session,
            begin_workout,
            simulate_progress,
            confirm_weight,
            resume_coding,
            honor_complete,
            debug_mode,
            debug_videos,
            debug_stream_start,
            debug_stream_stop
        ])
        .setup(|app| {
            hub::start(app.handle().clone());
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let clock = SystemClock;
                let state = handle.state::<SharedCore>();
                let mut core = state.lock().unwrap();
                core.session.tick(clock.now(), &clock.today());
                let snap = core.session.snapshot(clock.now());
                drop(core);
                emit_snapshot(&handle, &snap);
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

    #[test]
    fn should_store_spawn_when_generation_still_current() {
        // No newer debug_stream_start call raced ahead: store the child.
        assert!(should_store_spawn(3, 3));
    }

    #[test]
    fn should_store_spawn_false_when_superseded_by_newer_generation() {
        // A newer debug_stream_start call bumped the generation while our
        // spawn() was blocking: our child is orphaned and must be killed,
        // never stored (it would clobber the newer call's handle).
        assert!(!should_store_spawn(4, 3));
    }
}
