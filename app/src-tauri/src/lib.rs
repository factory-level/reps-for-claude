use std::sync::Mutex;
use std::time::Duration;

use engine::clock::{Clock, SystemClock};
use engine::session::Session;
use engine::store::{default_continuous_pool, Store};
use engine::timer::CodingTimer;
use engine::types::{Progress, Snapshot};
use engine::workout::WorkoutEngine;
use tauri::{AppHandle, Emitter, Manager, State};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(build_core()) as SharedCore)
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            start_session,
            begin_workout,
            simulate_progress,
            confirm_weight,
            resume_coding
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
