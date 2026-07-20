# Tauri Rewrite — Milestone 1 (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the repo into `app/` (Tauri + React + TS) and `vision/` (Python detection sidecar package), delete the legacy Python, and build the headless Rust core — state machine, Timer/Workout engines, SQLite store — surfaced in a minimal Workstation view.

**Architecture:** A `engine` Rust subcrate holds all logic (Tauri-free, fully unit-tested with a fake clock); the `src-tauri` crate wraps it with managed state, commands, a 1 Hz tick loop, and `snapshot` events. React renders snapshots and sends intents. `vision/` is a standalone uv package (`reps_vision`) carrying the proven detection stack + tests; its sidecar CLI comes in Milestone 2.

**Tech Stack:** Rust (stable, rustup), Tauri v2, React 18 + TypeScript + Vite, vitest + Testing Library, rusqlite (bundled SQLite), serde, chrono; Python ≥3.12 + pytest (vision only).

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-19-tauri-rewrite-design.md`. Target is **this machine only** (Linux Mint, X11). No cross-platform hedging.
- **State machine phases** (spec + V1 §20), exact names: `CODING → EXERCISE_REQUIRED → WORKOUT_ACTIVE → WEIGHT_CONFIRMATION → UNLOCKED → CODING`.
- **No lock, no camera in Milestone 1.** Progress is driven by a dev-mode simulate command. `xsecurelock` and the sidecar driver are Milestone 2–3.
- Engine crate must **not depend on tauri** — `cargo test -p engine` must pass without webkit system libs.
- Snapshot JSON is `camelCase` (serde `rename_all`); it is the single UI contract.
- Vision package: Python ≥3.12, modules start with `from __future__ import annotations`, hermetic tests via `REPS_HOME`, `cvvideo` marker excluded by default. No new Python dependencies.
- Every git commit message ends with this trailer (repo convention):
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```
- Run Python tests with `uv run pytest` from `vision/`; Rust tests with `cargo test -p engine` from `app/src-tauri/`; JS tests with `npm test` from `app/`.

---

### Task 1: Toolchain prerequisites

**Files:** none (system setup). **This task needs the user for one sudo command.**

**Interfaces:**
- Consumes: nothing.
- Produces: working `cargo`, and webkit2gtk system libs so later `cargo check` of the tauri crate succeeds.

- [ ] **Step 1: Install rustup (user-level, no sudo)**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
cargo --version
```

Expected: `cargo 1.8x.x`.

- [ ] **Step 2: System libraries (USER ACTION — needs sudo password)**

Ask the user to run this themselves (in Claude Code, typing `! <command>` runs it in-session):

```bash
sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Verify afterwards:

```bash
pkg-config --exists webkit2gtk-4.1 && echo OK
```

Expected: `OK`. **Do not proceed to Task 4 until this prints OK.** (Tasks 2–3 are Python-only and may proceed.)

- [ ] **Step 3: No commit** (nothing in the repo changed).

---

### Task 2: Extract the `vision/` package

**Files:**
- Create: `vision/pyproject.toml`, `vision/src/reps_vision/paths.py`, `vision/tests/conftest.py`
- Move (git mv): `src/reps_for_claude/{angles,exercises,detector,pose,video,visualize}.py` → `vision/src/reps_vision/`; `src/reps_for_claude/activities/` → `vision/src/reps_vision/activities/`; `tests/{test_angles,test_exercises,test_detector,test_activities_lift,test_activities_jumprope,test_activities_stretch,test_video,test_video_integration,test_visualize}.py` → `vision/tests/`; `tests/fixtures/` → `vision/tests/fixtures/`; `scripts/fetch_fixtures.py` → `vision/scripts/fetch_fixtures.py`
- Modify: `vision/src/reps_vision/pose.py` (one import), all moved files (package rename), `vision/tests/test_video_integration.py` (drop one legacy test)

**Interfaces:**
- Consumes: nothing.
- Produces: importable package `reps_vision` with modules `angles`, `exercises`, `detector`, `pose`, `video`, `visualize`, `activities.{base,lift,jumprope,stretch}`, `paths.cache_dir() -> Path`. Milestone 2's sidecar CLI builds on exactly these.

- [ ] **Step 1: Create the project skeleton and move the files**

```bash
mkdir -p vision/src/reps_vision vision/tests vision/scripts
git mv src/reps_for_claude/angles.py vision/src/reps_vision/angles.py
git mv src/reps_for_claude/exercises.py vision/src/reps_vision/exercises.py
git mv src/reps_for_claude/detector.py vision/src/reps_vision/detector.py
git mv src/reps_for_claude/pose.py vision/src/reps_vision/pose.py
git mv src/reps_for_claude/video.py vision/src/reps_vision/video.py
git mv src/reps_for_claude/visualize.py vision/src/reps_vision/visualize.py
git mv src/reps_for_claude/activities vision/src/reps_vision/activities
for t in angles exercises detector activities_lift activities_jumprope activities_stretch video video_integration visualize; do
  git mv "tests/test_${t}.py" "vision/tests/test_${t}.py"
done
git mv tests/fixtures vision/tests/fixtures
git mv scripts/fetch_fixtures.py vision/scripts/fetch_fixtures.py
```

- [ ] **Step 2: Write `vision/pyproject.toml`**

```toml
[project]
name = "reps-vision"
version = "0.1.0"
description = "Pose detection and rep counting sidecar for Reps for Claude"
requires-python = ">=3.12"
dependencies = []

[project.optional-dependencies]
cv = ["mediapipe>=0.10.14", "opencv-python>=4.9"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/reps_vision"]

[dependency-groups]
dev = ["pytest>=8"]

[tool.pytest.ini_options]
testpaths = ["tests"]
addopts = "-m 'not cvvideo'"
markers = [
    "cvvideo: integration tests running real pose estimation over video fixtures",
]
```

- [ ] **Step 3: Write `vision/src/reps_vision/__init__.py` and `paths.py`**

`vision/src/reps_vision/__init__.py`:

```python
"""reps_vision: pose detection and rep counting for Reps for Claude."""

__version__ = "0.1.0"
```

`vision/src/reps_vision/paths.py` (replaces the deleted `config.cache_dir`):

```python
"""Filesystem locations. REPS_HOME relocates everything (tests use this)."""

from __future__ import annotations

import os
from pathlib import Path


def cache_dir() -> Path:
    home = os.environ.get("REPS_HOME")
    if home:
        return Path(home) / "cache"
    return Path.home() / ".cache" / "reps-for-claude"
```

- [ ] **Step 4: Rewrite package references**

```bash
cd vision
grep -rl "reps_for_claude" src tests scripts | xargs sed -i "s/reps_for_claude/reps_vision/g"
sed -i "s/from .config import cache_dir/from .paths import cache_dir/" src/reps_vision/pose.py
```

Then check nothing else references the old config:

```bash
grep -rn "\.config\b\|import config" src tests scripts || echo clean
```

Expected: `clean`. If `fetch_fixtures.py` referenced other legacy modules, trim it to stdlib + fixture paths only.

- [ ] **Step 5: Write `vision/tests/conftest.py`**

The old conftest's `cfg`/`ledger` fixtures belong to deleted legacy modules; only `reps_home` is needed here:

```python
from pathlib import Path

import pytest


@pytest.fixture
def reps_home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point REPS_HOME at a temp dir so model-cache paths are hermetic."""
    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))
    return home
```

- [ ] **Step 6: Drop the legacy CLI test**

In `vision/tests/test_video_integration.py`, delete the entire `test_analyze_does_not_touch_ledger` function (it exercises the legacy `reps analyze` CLI + ledger, both deleted in Task 3). Keep every other test.

- [ ] **Step 7: Run the vision suite**

```bash
cd vision && uv sync && uv run pytest -v
```

Expected: PASS (angles, exercises, detector, activities, video, visualize; cvvideo deselected). If an import error surfaces a missed rename, fix and re-run.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "refactor: extract detection stack into vision/ (reps_vision)"
```

---

### Task 3: Delete the legacy Python package

**Files:**
- Delete: `src/`, `tests/`, `pyproject.toml`, `uv.lock`, `scripts/`
- Modify: `README.md` (two-line status note)

**Interfaces:**
- Consumes: Task 2 (vision/ is standalone).
- Produces: a repo with no root Python project; `vision/` is the only Python.

- [ ] **Step 1: Delete**

```bash
git rm -r src tests pyproject.toml uv.lock
git rm -r scripts 2>/dev/null || true
```

- [ ] **Step 2: Verify vision is unaffected and nothing dangles**

```bash
cd vision && uv run pytest -q && cd ..
grep -rn "reps_for_claude" --include="*.py" --include="*.toml" . | grep -v "^./docs\|^./_docs" || echo clean
```

Expected: tests PASS, then `clean`.

- [ ] **Step 3: Update `README.md`** — replace the Install/Use/Configuration/Development sections with:

```markdown
## Status

Ground-up rewrite in progress per
`docs/superpowers/specs/2026-07-19-tauri-rewrite-design.md`:
`app/` is the Tauri + React desktop app; `vision/` is the Python
pose-detection sidecar (`cd vision && uv run pytest`).
```

Keep the title, intro, mermaid loop diagram, and `_docs/` links.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor: delete legacy Python app (superseded by Tauri rewrite)"
```

---

### Task 4: Scaffold the Tauri app + engine subcrate

**Files:**
- Create: `app/` (via create-tauri-app), `app/src-tauri/engine/Cargo.toml`, `app/src-tauri/engine/src/lib.rs`
- Modify: `app/src-tauri/Cargo.toml` (workspace + dependency), `.gitignore`

**Interfaces:**
- Consumes: Task 1 (cargo + webkit libs).
- Produces: `app/` builds (`cargo check`); an empty `engine` crate that Tasks 5–9 fill; `cargo test -p engine` works.

- [ ] **Step 1: Scaffold**

```bash
source "$HOME/.cargo/env"
npm create tauri-app@latest app -- --template react-ts --manager npm --yes
cd app && npm install
```

Expected: `app/` contains `src/` (React) and `src-tauri/` (Rust). If the CLI prompts despite `--yes`, answer: name `app`, identifier `dev.calvin.reps-for-claude`, frontend `TypeScript / React`, manager `npm`.

- [ ] **Step 2: Create the engine subcrate**

`app/src-tauri/engine/Cargo.toml`:

```toml
[package]
name = "engine"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled"] }
chrono = { version = "0.4", default-features = false, features = ["clock"] }

[dev-dependencies]
tempfile = "3"
```

`app/src-tauri/engine/src/lib.rs`:

```rust
//! Headless core: state machine, timers, workout engine, storage.
//! No tauri dependency — everything here tests with `cargo test -p engine`.

pub mod clock;
pub mod session;
pub mod store;
pub mod timer;
pub mod types;
pub mod workout;
```

Create empty module files so it compiles: `clock.rs`, `session.rs`, `store.rs`, `timer.rs`, `types.rs`, `workout.rs`, each containing only a doc comment (e.g. `//! Filled in by a later task.`).

- [ ] **Step 3: Wire the workspace**

In `app/src-tauri/Cargo.toml` add (top level):

```toml
[workspace]
members = ["engine"]
```

and under `[dependencies]`:

```toml
engine = { path = "engine" }
```

- [ ] **Step 4: Verify both crates build**

```bash
cd app/src-tauri && cargo check && cargo test -p engine
```

Expected: `cargo check` compiles the tauri crate (webkit found); `cargo test -p engine` runs 0 tests, exit 0.

- [ ] **Step 5: Commit**

```bash
cd ../.. && git add -A && git commit -m "feat(app): scaffold Tauri v2 + React app with headless engine subcrate"
```

---

### Task 5: Engine types + clock

**Files:**
- Create/replace: `app/src-tauri/engine/src/types.rs`, `app/src-tauri/engine/src/clock.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (used by every later task):
  - `types::ExerciseKind` (`Rep` | `Continuous`), `types::ExerciseDef { name: String, kind: ExerciseKind, default_reps: u32, default_weight: f64, target_seconds: f64 }`
  - `types::Prescription { exercise: String, kind: ExerciseKind, target_reps: u32, target_seconds: f64, default_weight: f64 }`
  - `types::Phase` (`Coding`|`ExerciseRequired`|`WorkoutActive`|`WeightConfirmation`|`Unlocked`, serializes SCREAMING_SNAKE_CASE)
  - `types::Progress { value: f64, unit: String, satisfied: bool }`
  - `types::SetRecord { date: String, exercise: String, kind: ExerciseKind, reps: u32, seconds: f64, weight: f64, verified: bool }`
  - `types::Snapshot { phase, remaining_seconds: f64, prescription: Option<Prescription>, progress: Option<Progress>, capacity_used: u32, capacity_limit: u32, rotation: Vec<String>, pointer: usize }` (camelCase serde)
  - `clock::Clock` trait: `fn now(&self) -> f64; fn today(&self) -> String;` + `SystemClock` + `FakeClock::new(start: f64, date: &str)` with `advance(secs)` and `set_date(&str)`.

- [ ] **Step 1: Write the failing test** (in `types.rs` bottom + `clock.rs` bottom)

```rust
// in types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_serializes_screaming_snake() {
        let s = serde_json::to_string(&Phase::ExerciseRequired).unwrap();
        assert_eq!(s, "\"EXERCISE_REQUIRED\"");
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let snap = Snapshot {
            phase: Phase::Coding,
            remaining_seconds: 12.0,
            prescription: None,
            progress: None,
            capacity_used: 0,
            capacity_limit: 20,
            rotation: vec!["bench".into()],
            pointer: 0,
        };
        let v: serde_json::Value = serde_json::to_value(&snap).unwrap();
        assert!(v.get("remainingSeconds").is_some());
        assert!(v.get("capacityLimit").is_some());
    }
}
```

```rust
// in clock.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances() {
        let c = FakeClock::new(100.0, "2026-07-19");
        assert_eq!(c.now(), 100.0);
        c.advance(5.0);
        assert_eq!(c.now(), 105.0);
        assert_eq!(c.today(), "2026-07-19");
        c.set_date("2026-07-20");
        assert_eq!(c.today(), "2026-07-20");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine` — Expected: compile FAIL (types not defined).

- [ ] **Step 3: Implement**

`types.rs`:

```rust
//! Shared plain-data types. Snapshot is the single UI contract (camelCase).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExerciseKind {
    Rep,
    Continuous,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExerciseDef {
    pub name: String,
    pub kind: ExerciseKind,
    pub default_reps: u32,
    pub default_weight: f64,
    pub target_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prescription {
    pub exercise: String,
    pub kind: ExerciseKind,
    pub target_reps: u32,
    pub target_seconds: f64,
    pub default_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    Coding,
    ExerciseRequired,
    WorkoutActive,
    WeightConfirmation,
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub value: f64,
    pub unit: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRecord {
    pub date: String,
    pub exercise: String,
    pub kind: ExerciseKind,
    pub reps: u32,
    pub seconds: f64,
    pub weight: f64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub remaining_seconds: f64,
    pub prescription: Option<Prescription>,
    pub progress: Option<Progress>,
    pub capacity_used: u32,
    pub capacity_limit: u32,
    pub rotation: Vec<String>,
    pub pointer: usize,
}
```

`clock.rs`:

```rust
//! Time behind a trait so every engine test can use a fake clock.

use std::cell::Cell;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock {
    /// Monotonic-enough seconds. Only differences matter.
    fn now(&self) -> f64;
    /// Local date as ISO "YYYY-MM-DD" (drives daily capacity reset).
    fn today(&self) -> String;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before 1970")
            .as_secs_f64()
    }

    fn today(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }
}

pub struct FakeClock {
    now: Cell<f64>,
    date: Mutex<String>,
}

impl FakeClock {
    pub fn new(start: f64, date: &str) -> Self {
        Self { now: Cell::new(start), date: Mutex::new(date.to_string()) }
    }

    pub fn advance(&self, secs: f64) {
        self.now.set(self.now.get() + secs);
    }

    pub fn set_date(&self, date: &str) {
        *self.date.lock().unwrap() = date.to_string();
    }
}

impl Clock for FakeClock {
    fn now(&self) -> f64 {
        self.now.get()
    }

    fn today(&self) -> String {
        self.date.lock().unwrap().clone()
    }
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p engine` — Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/engine && git commit -m "feat(engine): shared types, Snapshot contract, Clock trait"
```

---

### Task 6: Workout engine (rotation + daily capacity)

**Files:**
- Create/replace: `app/src-tauri/engine/src/workout.rs`

**Interfaces:**
- Consumes: `types::{ExerciseDef, ExerciseKind, Prescription}`.
- Produces:
  - `WorkoutEngine::new(rotation: Vec<ExerciseDef>, continuous_pool: Vec<ExerciseDef>, capacity_limit: u32) -> Self`
  - `restore(&mut self, pointer: usize, capacity_used: u32, capacity_date: &str)`
  - `prescribe(&mut self, today: &str) -> Option<Prescription>` — rotation head while capacity remains; continuous pool (round-robin) once spent; rolls capacity on a new date; `None` only if both lists are empty.
  - `complete(&mut self, today: &str)` — advances the rotation pointer (wrapping) and, for Rep prescriptions, increments capacity.
  - Getters: `pointer() -> usize`, `capacity_used() -> u32`, `capacity_limit() -> u32`, `capacity_date() -> &str`, `rotation_names() -> Vec<String>`.

- [ ] **Step 1: Write the failing tests** (bottom of `workout.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExerciseDef, ExerciseKind};

    fn lift(name: &str) -> ExerciseDef {
        ExerciseDef {
            name: name.into(),
            kind: ExerciseKind::Rep,
            default_reps: 10,
            default_weight: 45.0,
            target_seconds: 0.0,
        }
    }

    fn cardio(name: &str, secs: f64) -> ExerciseDef {
        ExerciseDef {
            name: name.into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: secs,
        }
    }

    fn engine() -> WorkoutEngine {
        WorkoutEngine::new(
            vec![lift("bench"), lift("row"), lift("squat")],
            vec![cardio("jumprope", 60.0), cardio("stretch", 30.0)],
            2,
        )
    }

    #[test]
    fn rotation_steps_and_wraps() {
        let mut w = engine();
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "bench");
        w.complete("2026-07-19");
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "row");
        // pointer survives without completing: same prescription again
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "row");
    }

    #[test]
    fn capacity_switches_to_continuous_pool() {
        let mut w = engine();
        w.prescribe("2026-07-19");
        w.complete("2026-07-19"); // capacity 1/2
        w.prescribe("2026-07-19");
        w.complete("2026-07-19"); // capacity 2/2 spent
        let p = w.prescribe("2026-07-19").unwrap();
        assert_eq!(p.kind, ExerciseKind::Continuous);
        assert_eq!(p.exercise, "jumprope");
        w.complete("2026-07-19"); // continuous set does NOT add capacity
        assert_eq!(w.capacity_used(), 2);
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "stretch");
    }

    #[test]
    fn new_day_resets_capacity_but_not_pointer() {
        let mut w = engine();
        w.prescribe("2026-07-19");
        w.complete("2026-07-19");
        w.prescribe("2026-07-19");
        w.complete("2026-07-19");
        assert_eq!(w.capacity_used(), 2);
        let p = w.prescribe("2026-07-20").unwrap(); // new day
        assert_eq!(w.capacity_used(), 0);
        assert_eq!(p.kind, ExerciseKind::Rep);
        assert_eq!(p.exercise, "squat"); // pointer kept from yesterday
    }

    #[test]
    fn restore_resumes_state() {
        let mut w = engine();
        w.restore(2, 1, "2026-07-19");
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "squat");
        assert_eq!(w.capacity_used(), 1);
    }

    #[test]
    fn empty_rotation_prescribes_continuous() {
        let mut w = WorkoutEngine::new(vec![], vec![cardio("jumprope", 60.0)], 2);
        assert_eq!(w.prescribe("2026-07-19").unwrap().exercise, "jumprope");
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p engine workout` — Expected: compile FAIL.

- [ ] **Step 3: Implement**

```rust
//! Rotation + daily-capacity workout programming (V1 spec §6–7).
//! The pointer never resets; capacity resets on a new date.

use crate::types::{ExerciseDef, ExerciseKind, Prescription};

pub struct WorkoutEngine {
    rotation: Vec<ExerciseDef>,
    continuous_pool: Vec<ExerciseDef>,
    capacity_limit: u32,
    pointer: usize,
    continuous_pointer: usize,
    capacity_used: u32,
    capacity_date: String,
    last_kind: Option<ExerciseKind>,
}

impl WorkoutEngine {
    pub fn new(
        rotation: Vec<ExerciseDef>,
        continuous_pool: Vec<ExerciseDef>,
        capacity_limit: u32,
    ) -> Self {
        Self {
            rotation,
            continuous_pool,
            capacity_limit,
            pointer: 0,
            continuous_pointer: 0,
            capacity_used: 0,
            capacity_date: String::new(),
            last_kind: None,
        }
    }

    pub fn restore(&mut self, pointer: usize, capacity_used: u32, capacity_date: &str) {
        self.pointer = if self.rotation.is_empty() { 0 } else { pointer % self.rotation.len() };
        self.capacity_used = capacity_used;
        self.capacity_date = capacity_date.to_string();
    }

    fn roll_date(&mut self, today: &str) {
        if self.capacity_date != today {
            self.capacity_date = today.to_string();
            self.capacity_used = 0;
        }
    }

    pub fn prescribe(&mut self, today: &str) -> Option<Prescription> {
        self.roll_date(today);
        let lifting_open = self.capacity_used < self.capacity_limit;
        let def = if lifting_open && !self.rotation.is_empty() {
            &self.rotation[self.pointer]
        } else if !self.continuous_pool.is_empty() {
            &self.continuous_pool[self.continuous_pointer % self.continuous_pool.len()]
        } else if !self.rotation.is_empty() {
            &self.rotation[self.pointer]
        } else {
            return None;
        };
        self.last_kind = Some(def.kind);
        Some(Prescription {
            exercise: def.name.clone(),
            kind: def.kind,
            target_reps: def.default_reps,
            target_seconds: def.target_seconds,
            default_weight: def.default_weight,
        })
    }

    pub fn complete(&mut self, today: &str) {
        self.roll_date(today);
        match self.last_kind {
            Some(ExerciseKind::Rep) => {
                self.capacity_used += 1;
                if !self.rotation.is_empty() {
                    self.pointer = (self.pointer + 1) % self.rotation.len();
                }
            }
            Some(ExerciseKind::Continuous) => {
                if !self.continuous_pool.is_empty() {
                    self.continuous_pointer =
                        (self.continuous_pointer + 1) % self.continuous_pool.len();
                }
            }
            None => {}
        }
        self.last_kind = None;
    }

    pub fn pointer(&self) -> usize {
        self.pointer
    }

    pub fn capacity_used(&self) -> u32 {
        self.capacity_used
    }

    pub fn capacity_limit(&self) -> u32 {
        self.capacity_limit
    }

    pub fn capacity_date(&self) -> &str {
        &self.capacity_date
    }

    pub fn rotation_names(&self) -> Vec<String> {
        self.rotation.iter().map(|d| d.name.clone()).collect()
    }
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p engine workout` — Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/engine/src/workout.rs
git commit -m "feat(engine): rotation + daily-capacity workout engine"
```

---

### Task 7: Coding timer

**Files:**
- Create/replace: `app/src-tauri/engine/src/timer.rs`

**Interfaces:**
- Consumes: nothing (takes plain `f64` timestamps from `Clock::now`).
- Produces:
  - `CodingTimer::new(duration_secs: f64) -> Self`
  - `start(&mut self, now: f64)`, `remaining(&self, now: f64) -> f64` (clamped ≥ 0; equals duration when never started), `expired(&self, now: f64) -> bool` (false when never started), `stop(&mut self)`.

- [ ] **Step 1: Write the failing tests** (bottom of `timer.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_and_expires() {
        let mut t = CodingTimer::new(360.0);
        assert_eq!(t.remaining(50.0), 360.0); // not started
        assert!(!t.expired(50.0));
        t.start(100.0);
        assert_eq!(t.remaining(100.0), 360.0);
        assert_eq!(t.remaining(160.0), 300.0);
        assert!(!t.expired(459.9));
        assert!(t.expired(460.0));
        assert_eq!(t.remaining(500.0), 0.0);
    }

    #[test]
    fn stop_disarms() {
        let mut t = CodingTimer::new(360.0);
        t.start(0.0);
        t.stop();
        assert!(!t.expired(1000.0));
        assert_eq!(t.remaining(1000.0), 360.0);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p engine timer` — Expected: compile FAIL.

- [ ] **Step 3: Implement**

```rust
//! The coding countdown: work_minutes of focus before dues are owed.

pub struct CodingTimer {
    duration: f64,
    deadline: Option<f64>,
}

impl CodingTimer {
    pub fn new(duration_secs: f64) -> Self {
        Self { duration: duration_secs, deadline: None }
    }

    pub fn start(&mut self, now: f64) {
        self.deadline = Some(now + self.duration);
    }

    pub fn stop(&mut self) {
        self.deadline = None;
    }

    pub fn remaining(&self, now: f64) -> f64 {
        match self.deadline {
            Some(d) => (d - now).max(0.0),
            None => self.duration,
        }
    }

    pub fn expired(&self, now: f64) -> bool {
        matches!(self.deadline, Some(d) if now >= d)
    }
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p engine timer` — Expected: 2 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/engine/src/timer.rs
git commit -m "feat(engine): coding countdown timer"
```

---

### Task 8: Session state machine

**Files:**
- Create/replace: `app/src-tauri/engine/src/session.rs`

**Interfaces:**
- Consumes: `CodingTimer`, `WorkoutEngine`, `types::*` (exact signatures from Tasks 5–7).
- Produces (the tauri layer in Task 10 calls exactly these):
  - `Session::new(timer: CodingTimer, workout: WorkoutEngine) -> Self`
  - `start(&mut self, now: f64, today: &str)` — enters `Coding`, starts the timer.
  - `tick(&mut self, now: f64, today: &str) -> bool` — returns true if the phase changed (Coding + expired → ExerciseRequired with a fresh prescription).
  - `begin_workout(&mut self)` — ExerciseRequired → WorkoutActive (no-op otherwise).
  - `report_progress(&mut self, p: Progress)` — stores progress; if `satisfied` in WorkoutActive: Rep → WeightConfirmation, Continuous → completes immediately (record with weight 0, verified true) and → Unlocked.
  - `confirm_weight(&mut self, weight: f64, today: &str) -> Option<SetRecord>` — WeightConfirmation → Unlocked; returns the record to persist (`verified: true`).
  - `resume_coding(&mut self, now: f64)` — Unlocked → Coding, timer restarted.
  - `snapshot(&self, now: f64) -> Snapshot`.
  - `take_pending_record(&mut self) -> Option<SetRecord>` — the record produced by the last completion, once.
  - `workout(&self) -> &WorkoutEngine` (for persistence of pointer/capacity).

- [ ] **Step 1: Write the failing tests** (bottom of `session.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::CodingTimer;
    use crate::types::{ExerciseDef, ExerciseKind, Phase, Progress};
    use crate::workout::WorkoutEngine;

    fn session() -> Session {
        let rotation = vec![ExerciseDef {
            name: "bench".into(),
            kind: ExerciseKind::Rep,
            default_reps: 10,
            default_weight: 45.0,
            target_seconds: 0.0,
        }];
        let pool = vec![ExerciseDef {
            name: "jumprope".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 60.0,
        }];
        Session::new(CodingTimer::new(360.0), WorkoutEngine::new(rotation, pool, 20))
    }

    fn done(unit: &str, v: f64) -> Progress {
        Progress { value: v, unit: unit.into(), satisfied: true }
    }

    #[test]
    fn full_lift_cycle() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        assert_eq!(s.snapshot(0.0).phase, Phase::Coding);

        assert!(!s.tick(359.0, "2026-07-19"));
        assert!(s.tick(360.0, "2026-07-19"));
        let snap = s.snapshot(360.0);
        assert_eq!(snap.phase, Phase::ExerciseRequired);
        assert_eq!(snap.prescription.as_ref().unwrap().exercise, "bench");

        s.begin_workout();
        assert_eq!(s.snapshot(361.0).phase, Phase::WorkoutActive);

        s.report_progress(Progress { value: 3.0, unit: "reps".into(), satisfied: false });
        assert_eq!(s.snapshot(400.0).phase, Phase::WorkoutActive);

        s.report_progress(done("reps", 10.0));
        assert_eq!(s.snapshot(401.0).phase, Phase::WeightConfirmation);

        let rec = s.confirm_weight(145.0, "2026-07-19").unwrap();
        assert_eq!(rec.weight, 145.0);
        assert_eq!(rec.reps, 10);
        assert!(rec.verified);
        assert_eq!(s.snapshot(402.0).phase, Phase::Unlocked);

        s.resume_coding(500.0);
        let snap = s.snapshot(500.0);
        assert_eq!(snap.phase, Phase::Coding);
        assert_eq!(snap.remaining_seconds, 360.0);
        assert_eq!(snap.capacity_used, 1);
    }

    #[test]
    fn continuous_skips_weight_confirmation() {
        let mut s = session();
        // force capacity spent so prescription is continuous
        s.workout_mut_for_test().restore(0, 20, "2026-07-19");
        s.start(0.0, "2026-07-19");
        s.tick(360.0, "2026-07-19");
        assert_eq!(
            s.snapshot(360.0).prescription.as_ref().unwrap().kind,
            ExerciseKind::Continuous
        );
        s.begin_workout();
        s.report_progress(done("seconds", 60.0));
        assert_eq!(s.snapshot(361.0).phase, Phase::Unlocked);
        let rec = s.take_pending_record().unwrap();
        assert_eq!(rec.exercise, "jumprope");
        assert_eq!(rec.weight, 0.0);
    }

    #[test]
    fn guards_ignore_wrong_phase_calls() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        s.begin_workout(); // Coding: no-op
        assert_eq!(s.snapshot(1.0).phase, Phase::Coding);
        assert!(s.confirm_weight(100.0, "2026-07-19").is_none());
        s.report_progress(done("reps", 10.0)); // no active workout: no-op
        assert_eq!(s.snapshot(2.0).phase, Phase::Coding);
    }

    #[test]
    fn timer_stops_while_locked() {
        let mut s = session();
        s.start(0.0, "2026-07-19");
        s.tick(360.0, "2026-07-19");
        // long workout: coding timer must not be running
        assert_eq!(s.snapshot(10_000.0).remaining_seconds, 360.0);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p engine session` — Expected: compile FAIL.

- [ ] **Step 3: Implement**

```rust
//! The application state machine (V1 spec §20):
//! CODING → EXERCISE_REQUIRED → WORKOUT_ACTIVE → WEIGHT_CONFIRMATION → UNLOCKED → CODING.
//! Pure: driven by tick()/commands with explicit time, no threads, no IO.

use crate::timer::CodingTimer;
use crate::types::{ExerciseKind, Phase, Prescription, Progress, SetRecord, Snapshot};
use crate::workout::WorkoutEngine;

pub struct Session {
    phase: Phase,
    timer: CodingTimer,
    workout: WorkoutEngine,
    prescription: Option<Prescription>,
    progress: Option<Progress>,
    pending_record: Option<SetRecord>,
}

impl Session {
    pub fn new(timer: CodingTimer, workout: WorkoutEngine) -> Self {
        Self {
            phase: Phase::Coding,
            timer,
            workout,
            prescription: None,
            progress: None,
            pending_record: None,
        }
    }

    pub fn start(&mut self, now: f64, _today: &str) {
        self.phase = Phase::Coding;
        self.timer.start(now);
    }

    pub fn tick(&mut self, now: f64, today: &str) -> bool {
        if self.phase == Phase::Coding && self.timer.expired(now) {
            self.timer.stop();
            self.prescription = self.workout.prescribe(today);
            self.progress = None;
            self.phase = Phase::ExerciseRequired;
            return true;
        }
        false
    }

    pub fn begin_workout(&mut self) {
        if self.phase == Phase::ExerciseRequired && self.prescription.is_some() {
            self.phase = Phase::WorkoutActive;
        }
    }

    pub fn report_progress(&mut self, p: Progress) {
        if self.phase != Phase::WorkoutActive {
            return;
        }
        let satisfied = p.satisfied;
        self.progress = Some(p);
        if !satisfied {
            return;
        }
        match self.prescription.as_ref().map(|rx| rx.kind) {
            Some(ExerciseKind::Rep) => self.phase = Phase::WeightConfirmation,
            Some(ExerciseKind::Continuous) => {
                let rx = self.prescription.as_ref().unwrap();
                self.pending_record = Some(SetRecord {
                    date: self.workout.capacity_date().to_string(),
                    exercise: rx.exercise.clone(),
                    kind: rx.kind,
                    reps: 0,
                    seconds: rx.target_seconds,
                    weight: 0.0,
                    verified: true,
                });
                let date = self.workout.capacity_date().to_string();
                self.workout.complete(&date);
                self.phase = Phase::Unlocked;
            }
            None => {}
        }
    }

    pub fn confirm_weight(&mut self, weight: f64, today: &str) -> Option<SetRecord> {
        if self.phase != Phase::WeightConfirmation {
            return None;
        }
        let rx = self.prescription.as_ref()?;
        let record = SetRecord {
            date: today.to_string(),
            exercise: rx.exercise.clone(),
            kind: rx.kind,
            reps: rx.target_reps,
            seconds: 0.0,
            weight,
            verified: true,
        };
        self.workout.complete(today);
        self.pending_record = Some(record.clone());
        self.phase = Phase::Unlocked;
        Some(record)
    }

    pub fn resume_coding(&mut self, now: f64) {
        if self.phase == Phase::Unlocked {
            self.prescription = None;
            self.progress = None;
            self.start(now, "");
        }
    }

    pub fn take_pending_record(&mut self) -> Option<SetRecord> {
        self.pending_record.take()
    }

    pub fn snapshot(&self, now: f64) -> Snapshot {
        Snapshot {
            phase: self.phase,
            remaining_seconds: self.timer.remaining(now),
            prescription: self.prescription.clone(),
            progress: self.progress.clone(),
            capacity_used: self.workout.capacity_used(),
            capacity_limit: self.workout.capacity_limit(),
            rotation: self.workout.rotation_names(),
            pointer: self.workout.pointer(),
        }
    }

    pub fn workout(&self) -> &WorkoutEngine {
        &self.workout
    }

    #[cfg(test)]
    pub fn workout_mut_for_test(&mut self) -> &mut WorkoutEngine {
        &mut self.workout
    }
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p engine session` — Expected: 4 tests PASS. Also run the full crate: `cargo test -p engine` — all green.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/engine/src/session.rs
git commit -m "feat(engine): CODING->LOCKED->PAID session state machine"
```

---

### Task 9: SQLite store

**Files:**
- Create/replace: `app/src-tauri/engine/src/store.rs`

**Interfaces:**
- Consumes: `types::{ExerciseDef, ExerciseKind, SetRecord}`.
- Produces (Task 10 uses exactly these):
  - `Store::open(path: &std::path::Path) -> rusqlite::Result<Store>` — creates schema (versioned migration).
  - `load_rotation(&self) -> rusqlite::Result<Vec<ExerciseDef>>` / `save_rotation(&self, defs: &[ExerciseDef])` — seeds a default rotation on first open.
  - `load_pointer_state(&self) -> rusqlite::Result<(usize, u32, String)>` / `save_pointer_state(&self, pointer: usize, capacity_used: u32, capacity_date: &str)`.
  - `record_set(&self, rec: &SetRecord) -> rusqlite::Result<()>` / `history(&self, limit: u32) -> rusqlite::Result<Vec<SetRecord>>` (newest first).
  - `setting(&self, key: &str, default: &str) -> String` / `set_setting(&self, key: &str, value: &str)`.

- [ ] **Step 1: Write the failing tests** (bottom of `store.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExerciseKind, SetRecord};

    fn open_tmp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("reps.sqlite")).unwrap();
        (dir, store)
    }

    #[test]
    fn seeds_default_rotation_once() {
        let (dir, store) = open_tmp();
        let rot = store.load_rotation().unwrap();
        assert!(!rot.is_empty());
        // reopen: still there, not duplicated
        drop(store);
        let store = Store::open(&dir.path().join("reps.sqlite")).unwrap();
        assert_eq!(store.load_rotation().unwrap().len(), rot.len());
    }

    #[test]
    fn pointer_state_roundtrips() {
        let (_dir, store) = open_tmp();
        assert_eq!(store.load_pointer_state().unwrap(), (0, 0, String::new()));
        store.save_pointer_state(3, 5, "2026-07-19").unwrap();
        assert_eq!(
            store.load_pointer_state().unwrap(),
            (3, 5, "2026-07-19".to_string())
        );
    }

    #[test]
    fn history_newest_first() {
        let (_dir, store) = open_tmp();
        for (i, name) in ["bench", "row"].iter().enumerate() {
            store
                .record_set(&SetRecord {
                    date: format!("2026-07-1{i}"),
                    exercise: name.to_string(),
                    kind: ExerciseKind::Rep,
                    reps: 10,
                    seconds: 0.0,
                    weight: 100.0,
                    verified: true,
                })
                .unwrap();
        }
        let h = store.history(10).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].exercise, "row");
    }

    #[test]
    fn settings_roundtrip_with_default() {
        let (_dir, store) = open_tmp();
        assert_eq!(store.setting("work_minutes", "6"), "6");
        store.set_setting("work_minutes", "25").unwrap();
        assert_eq!(store.setting("work_minutes", "6"), "25");
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p engine store` — Expected: compile FAIL.

- [ ] **Step 3: Implement**

```rust
//! SQLite persistence. Only this module touches the database.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::types::{ExerciseDef, ExerciseKind, SetRecord};

pub struct Store {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS rotation (
    position INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    default_reps INTEGER NOT NULL,
    default_weight REAL NOT NULL,
    target_seconds REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS pointer_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    pointer INTEGER NOT NULL,
    capacity_used INTEGER NOT NULL,
    capacity_date TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS exercise_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    exercise TEXT NOT NULL,
    kind TEXT NOT NULL,
    reps INTEGER NOT NULL,
    seconds REAL NOT NULL,
    weight REAL NOT NULL,
    verified INTEGER NOT NULL
);
";

fn kind_to_str(k: ExerciseKind) -> &'static str {
    match k {
        ExerciseKind::Rep => "rep",
        ExerciseKind::Continuous => "continuous",
    }
}

fn kind_from_str(s: &str) -> ExerciseKind {
    if s == "continuous" { ExerciseKind::Continuous } else { ExerciseKind::Rep }
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        let version: Option<i64> = conn
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .ok();
        if version.is_none() {
            conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
        }
        let store = Self { conn };
        if store.load_rotation()?.is_empty() {
            store.save_rotation(&default_rotation())?;
        }
        Ok(store)
    }

    pub fn load_rotation(&self) -> rusqlite::Result<Vec<ExerciseDef>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, default_reps, default_weight, target_seconds
             FROM rotation ORDER BY position",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ExerciseDef {
                name: r.get(0)?,
                kind: kind_from_str(&r.get::<_, String>(1)?),
                default_reps: r.get(2)?,
                default_weight: r.get(3)?,
                target_seconds: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn save_rotation(&self, defs: &[ExerciseDef]) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM rotation", [])?;
        for (i, d) in defs.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO rotation (position, name, kind, default_reps, default_weight, target_seconds)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![i as i64, d.name, kind_to_str(d.kind), d.default_reps, d.default_weight, d.target_seconds],
            )?;
        }
        Ok(())
    }

    pub fn load_pointer_state(&self) -> rusqlite::Result<(usize, u32, String)> {
        let row = self
            .conn
            .query_row(
                "SELECT pointer, capacity_used, capacity_date FROM pointer_state WHERE id = 1",
                [],
                |r| Ok((r.get::<_, i64>(0)? as usize, r.get(1)?, r.get(2)?)),
            );
        match row {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, 0, String::new())),
            Err(e) => Err(e),
        }
    }

    pub fn save_pointer_state(
        &self,
        pointer: usize,
        capacity_used: u32,
        capacity_date: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO pointer_state (id, pointer, capacity_used, capacity_date)
             VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
               pointer = ?1, capacity_used = ?2, capacity_date = ?3",
            params![pointer as i64, capacity_used, capacity_date],
        )?;
        Ok(())
    }

    pub fn record_set(&self, rec: &SetRecord) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO exercise_history (date, exercise, kind, reps, seconds, weight, verified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.date,
                rec.exercise,
                kind_to_str(rec.kind),
                rec.reps,
                rec.seconds,
                rec.weight,
                rec.verified as i64
            ],
        )?;
        Ok(())
    }

    pub fn history(&self, limit: u32) -> rusqlite::Result<Vec<SetRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT date, exercise, kind, reps, seconds, weight, verified
             FROM exercise_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(SetRecord {
                date: r.get(0)?,
                exercise: r.get(1)?,
                kind: kind_from_str(&r.get::<_, String>(2)?),
                reps: r.get(3)?,
                seconds: r.get(4)?,
                weight: r.get(5)?,
                verified: r.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn setting(&self, key: &str, default: &str) -> String {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .unwrap_or_else(|_| default.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }
}

pub fn default_rotation() -> Vec<ExerciseDef> {
    let lift = |name: &str, weight: f64| ExerciseDef {
        name: name.into(),
        kind: ExerciseKind::Rep,
        default_reps: 10,
        default_weight: weight,
        target_seconds: 0.0,
    };
    vec![
        lift("bench", 95.0),
        lift("row", 65.0),
        lift("squat", 115.0),
        lift("shoulder_press", 55.0),
        lift("curl", 25.0),
    ]
}

pub fn default_continuous_pool() -> Vec<ExerciseDef> {
    vec![
        ExerciseDef {
            name: "jumprope".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 60.0,
        },
        ExerciseDef {
            name: "stretch".into(),
            kind: ExerciseKind::Continuous,
            default_reps: 0,
            default_weight: 0.0,
            target_seconds: 30.0,
        },
    ]
}
```

- [ ] **Step 4: Run to verify pass** — `cargo test -p engine store` — Expected: 4 tests PASS. Full crate green: `cargo test -p engine`.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/engine/src/store.rs
git commit -m "feat(engine): SQLite store with seeded rotation and history"
```

---

### Task 10: Tauri wiring — managed state, commands, tick loop

**Files:**
- Modify: `app/src-tauri/src/lib.rs` (replace scaffold body), `app/src-tauri/tauri.conf.json` (app title only)

**Interfaces:**
- Consumes: everything from Tasks 5–9.
- Produces the UI contract Task 11 relies on:
  - Event `"snapshot"` with a `types::Snapshot` payload, emitted on every tick and after every command.
  - Commands (all return the fresh `Snapshot`): `get_snapshot()`, `start_session()`, `begin_workout()`, `simulate_progress(value: f64, satisfied: bool)`, `confirm_weight(weight: f64)`, `resume_coding()`.
  - Database at `~/.local/share/reps-for-claude/reps.sqlite`. Settings key `work_minutes` (default `"6"`), capacity key `max_weighted_sets` (default `"20"`).

- [ ] **Step 1: Replace `app/src-tauri/src/lib.rs`**

```rust
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
```

Note: `confirm_weight` records via the returned `SetRecord`; the extra `take_pending_record()` drain prevents double-insert from `persist_and_snapshot`.

- [ ] **Step 2: Set the window title** — in `app/src-tauri/tauri.conf.json`, set the main window `"title": "Reps for Claude"`.

- [ ] **Step 3: Verify it compiles and engine still green**

```bash
cd app/src-tauri && cargo check && cargo test -p engine
```

Expected: clean check, all engine tests PASS.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri && git commit -m "feat(app): wire engine into Tauri commands, snapshot events, 1Hz tick"
```

---

### Task 11: Minimal Workstation view

**Files:**
- Create: `app/src/snapshot.ts`, `app/src/useSnapshot.ts`, `app/src/WorkstationCard.tsx`, `app/src/WorkstationCard.test.tsx`, `app/vitest.config.ts`, `app/src/test-setup.ts`
- Modify: `app/src/App.tsx` (replace scaffold), `app/package.json` (test script + dev deps)

**Interfaces:**
- Consumes: the `"snapshot"` event and the six commands from Task 10, exactly as named.
- Produces: the Workstation screen per V1 spec §5.1 — three timer-widget states — plus dev-mode simulate buttons.

- [ ] **Step 1: Install test tooling**

```bash
cd app
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom @vitejs/plugin-react
```

Add to `app/package.json` scripts: `"test": "vitest run"`.

`app/vitest.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
});
```

`app/src/test-setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: Write the snapshot type** — `app/src/snapshot.ts`:

```ts
export type Phase =
  | "CODING"
  | "EXERCISE_REQUIRED"
  | "WORKOUT_ACTIVE"
  | "WEIGHT_CONFIRMATION"
  | "UNLOCKED";

export interface Prescription {
  exercise: string;
  kind: "REP" | "CONTINUOUS";
  targetReps: number;
  targetSeconds: number;
  defaultWeight: number;
}

export interface Progress {
  value: number;
  unit: string;
  satisfied: boolean;
}

export interface Snapshot {
  phase: Phase;
  remainingSeconds: number;
  prescription: Prescription | null;
  progress: Progress | null;
  capacityUsed: number;
  capacityLimit: number;
  rotation: string[];
  pointer: number;
}
```

- [ ] **Step 3: Write the failing component test** — `app/src/WorkstationCard.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkstationCard } from "./WorkstationCard";
import type { Snapshot } from "./snapshot";

const base: Snapshot = {
  phase: "CODING",
  remainingSeconds: 1122,
  prescription: null,
  progress: null,
  capacityUsed: 3,
  capacityLimit: 20,
  rotation: ["bench", "row"],
  pointer: 1,
};

describe("WorkstationCard", () => {
  it("shows the countdown and next exercise while coding", () => {
    render(<WorkstationCard snapshot={base} onAction={vi.fn()} />);
    expect(screen.getByText("18:42")).toBeInTheDocument();
    expect(screen.getByText(/row/)).toBeInTheDocument();
    expect(screen.getByText(/Coding Session/)).toBeInTheDocument();
  });

  it("shows the prescription when exercise is required", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "EXERCISE_REQUIRED",
          prescription: {
            exercise: "bench",
            kind: "REP",
            targetReps: 10,
            targetSeconds: 0,
            defaultWeight: 95,
          },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/Movement Required/)).toBeInTheDocument();
    expect(screen.getByText(/bench/)).toBeInTheDocument();
    expect(screen.getByText(/0 \/ 10 reps/)).toBeInTheDocument();
  });

  it("shows live progress during the workout", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "WORKOUT_ACTIVE",
          prescription: {
            exercise: "bench",
            kind: "REP",
            targetReps: 10,
            targetSeconds: 0,
            defaultWeight: 95,
          },
          progress: { value: 4, unit: "reps", satisfied: false },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/4 \/ 10 reps/)).toBeInTheDocument();
  });

  it("shows unlocked state", () => {
    render(
      <WorkstationCard snapshot={{ ...base, phase: "UNLOCKED" }} onAction={vi.fn()} />,
    );
    expect(screen.getByText(/Unlocked/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run to verify failure**

Run: `npm test` — Expected: FAIL (`WorkstationCard` not found).

- [ ] **Step 5: Implement the card** — `app/src/WorkstationCard.tsx`:

```tsx
import type { Snapshot } from "./snapshot";

export type Action =
  | "start_session"
  | "begin_workout"
  | "simulate_rep"
  | "simulate_done"
  | "confirm_weight"
  | "resume_coding";

function mmss(total: number): string {
  const m = Math.floor(total / 60);
  const s = Math.floor(total % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function WorkstationCard({
  snapshot,
  onAction,
}: {
  snapshot: Snapshot;
  onAction: (a: Action, value?: number) => void;
}) {
  const { phase, prescription, progress } = snapshot;
  const next = snapshot.rotation[snapshot.pointer] ?? "—";
  const target = prescription?.targetReps ?? 0;
  const done = progress?.value ?? 0;

  if (phase === "CODING") {
    return (
      <section>
        <h1>Coding Session</h1>
        <p className="timer">{mmss(snapshot.remainingSeconds)}</p>
        <p>Next: {next}</p>
        <p>
          Capacity: {snapshot.capacityUsed} / {snapshot.capacityLimit} sets
        </p>
        <button onClick={() => onAction("start_session")}>Restart timer</button>
      </section>
    );
  }
  if (phase === "EXERCISE_REQUIRED") {
    return (
      <section>
        <h1>Movement Required</h1>
        <p className="exercise">{prescription?.exercise}</p>
        <p>
          0 / {target} {prescription?.kind === "REP" ? "reps" : "seconds"}
        </p>
        <button onClick={() => onAction("begin_workout")}>Start workout</button>
      </section>
    );
  }
  if (phase === "WORKOUT_ACTIVE") {
    return (
      <section>
        <h1>{prescription?.exercise}</h1>
        <p>
          {done} / {target} {progress?.unit ?? "reps"}
        </p>
        <button onClick={() => onAction("simulate_rep", done + 1)}>
          Simulate rep (dev)
        </button>
        <button onClick={() => onAction("simulate_done")}>Simulate done (dev)</button>
      </section>
    );
  }
  if (phase === "WEIGHT_CONFIRMATION") {
    return (
      <section>
        <h1>What weight did you use?</h1>
        <p>{prescription?.exercise}</p>
        <button
          onClick={() => onAction("confirm_weight", prescription?.defaultWeight ?? 0)}
        >
          Confirm {prescription?.defaultWeight ?? 0} lbs
        </button>
      </section>
    );
  }
  return (
    <section>
      <h1>Unlocked</h1>
      <p>Coding session available</p>
      <button onClick={() => onAction("resume_coding")}>Back to coding</button>
    </section>
  );
}
```

- [ ] **Step 6: Run to verify pass** — `npm test` — Expected: 4 tests PASS.

- [ ] **Step 7: Wire the live app** — `app/src/useSnapshot.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Snapshot } from "./snapshot";

export function useSnapshot(): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    invoke<Snapshot>("get_snapshot").then(setSnapshot);
    listen<Snapshot>("snapshot", (e) => setSnapshot(e.payload)).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, []);

  return snapshot;
}
```

Replace `app/src/App.tsx`:

```tsx
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import type { Action } from "./WorkstationCard";
import { WorkstationCard } from "./WorkstationCard";
import { useSnapshot } from "./useSnapshot";

const COMMANDS: Record<Action, (value?: number) => Promise<unknown>> = {
  start_session: () => invoke("start_session"),
  begin_workout: () => invoke("begin_workout"),
  simulate_rep: (v) => invoke("simulate_progress", { value: v ?? 1, satisfied: false }),
  simulate_done: () => invoke("simulate_progress", { value: 999, satisfied: true }),
  confirm_weight: (v) => invoke("confirm_weight", { weight: v ?? 0 }),
  resume_coding: () => invoke("resume_coding"),
};

export default function App() {
  const snapshot = useSnapshot();
  if (!snapshot) return <p>Connecting…</p>;
  return (
    <main className="container">
      <WorkstationCard
        snapshot={snapshot}
        onAction={(a, value) => void COMMANDS[a](value)}
      />
    </main>
  );
}
```

- [ ] **Step 8: Build + manual smoke test**

```bash
npm run build && npm run tauri dev
```

Expected: window "Reps for Claude" opens showing **Coding Session** with a live countdown. Walk one full cycle with the dev buttons: Restart timer → (set `work_minutes` low or wait) → Movement Required → Start workout → Simulate done → Confirm lbs → Unlocked → Back to coding. Then quit and relaunch: rotation pointer and capacity survive (check the Capacity line).

To make the wait short for the smoke test only:

```bash
sqlite3 ~/.local/share/reps-for-claude/reps.sqlite \
  "INSERT INTO settings (key, value) VALUES ('work_minutes','0.05')
   ON CONFLICT(key) DO UPDATE SET value='0.05';"
```

(Reset to `6` afterwards the same way.)

- [ ] **Step 9: Commit**

```bash
cd .. && git add app && git commit -m "feat(app): minimal Workstation view over live snapshots"
```

---

### Task 12: Full green + docs stamp

**Files:**
- Modify: `_docs/about/roadmap.md` (status only)

- [ ] **Step 1: Run everything**

```bash
cd vision && uv run pytest -q && cd ..
cd app/src-tauri && cargo test -p engine && cargo check && cd ../..
cd app && npm test && cd ..
```

Expected: all PASS.

- [ ] **Step 2: Stamp the wiki** — in `_docs/about/roadmap.md`, update the milestone diagram/status: Milestone 1 (Foundation) ✅ done; note that the repo is now `app/` + `vision/` per the Tauri rewrite spec. (A full wiki rewrite lands with later milestones.)

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "chore: Milestone 1 green — foundation complete"
```

---

## Self-review (against the spec)

- **Monorepo split** `app/` + `vision/`, legacy Python deleted → Tasks 2–4. ✓
- **Vision package** keeps pose/angles/exercises/activities/video/visualize + tests, hermetic, no new deps → Task 2. ✓
- **State machine** with exact V1 §20 phases, headless + fake-clock tested → Task 8. ✓
- **Timer Engine** (work_minutes) → Task 7; **Workout Engine** (rotation pointer that never resets, daily capacity → continuous pool, date rollover) → Task 6. ✓
- **SQLite** (settings, rotation, pointer state, exercise history with weight/verified) → Task 9. ✓
- **Snapshot as single UI contract, camelCase, event + commands** → Tasks 5, 10. ✓
- **Minimal Workstation view** with V1 §5.1's three widget states + dev simulate → Task 11. ✓
- **Engine crate tauri-free** (`cargo test -p engine` without webkit) → Tasks 4–9. ✓
- **This machine only; prerequisites explicit, sudo step flagged as user action** → Task 1. ✓

**Deferred to later milestones (not gaps):** sidecar JSON-lines CLI + Rust sidecar driver + Operator view (M2); xsecurelock + localhost server + `/lock` + weight-stepper UI polish (M3); Gym TV, themes/widgets (M4); Metrics view, settings UI (M5). Honor-mode fallback and override password belong to M2–M3 where the camera and lock exist.
