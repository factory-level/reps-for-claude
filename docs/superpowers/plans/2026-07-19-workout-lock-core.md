# Workout-Lock Core (Plan A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the headless engine for the fitness-pomodoro model — config, weekly-goal math, persistent weekly/dues stores, and streaming break-activity detectors — with no UI and no lock, fully unit-tested.

**Architecture:** Pure logic (`goals`) and streaming detectors (`activities/*`) sit on top of persistent JSON stores (`weekly`, `duesstate`) that mirror the existing atomic-write `ledger` pattern. The obsolete `guard`/`shim` gating is removed. Everything is hermetic via `REPS_HOME` and needs no camera, browser, or lock to test.

**Tech Stack:** Python ≥3.12 (stdlib: `tomllib`, `dataclasses`, `json`, `tempfile`, `datetime`), `typer` (existing CLI), `pytest`. Detection reuses the existing `pose`/`angles`/`exercises` modules. No new runtime dependencies.

## Global Constraints

- **Python ≥3.12**; every module starts with `from __future__ import annotations`.
- **No new dependencies** — stdlib only for new modules (detectors reuse the already-optional `mediapipe`/`opencv` via existing `pose.py`; no direct import in Plan A).
- **Atomic JSON persistence**: write to a temp file in the target dir, then `os.replace` — copy the existing `Ledger.save` pattern exactly. Corrupt/missing files degrade to a safe empty state with a `warning:` to stderr, never raise.
- **Hermetic tests**: use `tmp_path`; never touch real `~/.config` or `~/.local`. The `reps_home` fixture (in `tests/conftest.py`) sets `REPS_HOME`.
- **Run tests with**: `uv run pytest` (the project is uv-managed; `[tool.pytest.ini_options]` excludes the `cvvideo` marker by default).
- **Every git commit message** ends with these two trailers (per repo convention):
  ```
  Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01RoofxQWWKGJx8B5S4dAb4r
  ```
- **Branch**: work on `workout-lock-display` (already created; the design spec is committed there).
- Model naming: user-facing copy says "Claude"; never abbreviate.

---

### Task 1: Extend config for the new model

**Files:**
- Modify: `src/reps_for_claude/config.py`
- Test: `tests/test_config.py`

**Interfaces:**
- Consumes: nothing new.
- Produces: `Config` gains fields `work_minutes: int = 6`, `weekly_goals: dict[str, int]` (default `{}`), `default_reps: int = 10`, `jumprope_seconds: int = 60`, `stretch_seconds: int = 30`, `cam_width: int = 1280`, `cam_height: int = 720`, `scoreboard_monitor: int = 1`, `override_password: str = ""`. Parsed from `[session]`, `[goals.weekly]`, `[break]`, `[display]`, `[detector]` (width/height), `[lock]` (override_password). Existing fields unchanged.

- [ ] **Step 1: Write the failing tests**

Add to `tests/test_config.py`:

```python
def test_loads_new_model_sections(tmp_path):
    from reps_for_claude.config import load
    path = tmp_path / "config.toml"
    path.write_text(
        """
[session]
work_minutes = 6

[goals.weekly]
squat = 60
bench = 40

[break]
default_reps = 12
jumprope_seconds = 90
stretch_seconds = 45

[detector]
width = 640
height = 480

[display]
scoreboard_monitor = 2

[lock]
override_password = "hunter2"
"""
    )
    cfg = load(path)
    assert cfg.work_minutes == 6
    assert cfg.weekly_goals == {"squat": 60, "bench": 40}
    assert cfg.default_reps == 12
    assert cfg.jumprope_seconds == 90
    assert cfg.stretch_seconds == 45
    assert cfg.cam_width == 640
    assert cfg.cam_height == 480
    assert cfg.scoreboard_monitor == 2
    assert cfg.override_password == "hunter2"


def test_new_model_defaults(tmp_path):
    from reps_for_claude.config import load
    cfg = load(tmp_path / "missing.toml")
    assert cfg.work_minutes == 6
    assert cfg.weekly_goals == {}
    assert cfg.default_reps == 10
    assert cfg.scoreboard_monitor == 1
    assert cfg.override_password == ""


def test_weekly_goals_reject_non_positive(tmp_path):
    from reps_for_claude.config import load, ConfigError
    path = tmp_path / "config.toml"
    path.write_text("[goals.weekly]\nsquat = 0\n")
    with pytest.raises(ConfigError):
        load(path)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_config.py -k "new_model or weekly_goals_reject" -v`
Expected: FAIL (`AttributeError: 'Config' object has no attribute 'work_minutes'`).

- [ ] **Step 3: Implement the config changes**

In `src/reps_for_claude/config.py`, add fields to the `Config` dataclass (after `real_claude`):

```python
    work_minutes: int = 6
    weekly_goals: dict[str, int] = field(default_factory=dict)
    default_reps: int = 10
    jumprope_seconds: int = 60
    stretch_seconds: int = 30
    cam_width: int = 1280
    cam_height: int = 720
    scoreboard_monitor: int = 1
    override_password: str = ""
```

In `load()`, before `return cfg`, add parsing:

```python
    session = raw.get("session", {})
    if "work_minutes" in session:
        cfg.work_minutes = _require_positive_int(
            session["work_minutes"], "session.work_minutes"
        )

    weekly = raw.get("goals", {}).get("weekly", {})
    if not isinstance(weekly, dict):
        raise ConfigError("goals.weekly must be a table of exercise = target entries")
    cfg.weekly_goals = {
        name: _require_positive_int(target, f"goals.weekly.{name}")
        for name, target in weekly.items()
    }

    brk = raw.get("break", {})
    for key, attr in (
        ("default_reps", "default_reps"),
        ("jumprope_seconds", "jumprope_seconds"),
        ("stretch_seconds", "stretch_seconds"),
    ):
        if key in brk:
            setattr(cfg, attr, _require_positive_int(brk[key], f"break.{key}"))

    for key, attr in (("width", "cam_width"), ("height", "cam_height")):
        if key in detector:
            setattr(cfg, attr, _require_positive_int(detector[key], f"detector.{key}"))

    if "scoreboard_monitor" in raw.get("display", {}):
        idx = raw["display"]["scoreboard_monitor"]
        if not isinstance(idx, int) or isinstance(idx, bool) or idx < 0:
            raise ConfigError(
                f"display.scoreboard_monitor must be a non-negative integer, got {idx!r}"
            )
        cfg.scoreboard_monitor = idx

    if "override_password" in lock:
        if not isinstance(lock["override_password"], str):
            raise ConfigError("lock.override_password must be a string")
        cfg.override_password = lock["override_password"]
```

(The `detector` and `lock` local variables already exist earlier in `load()`; reuse them — do not re-fetch.)

Then update `SAMPLE_CONFIG` to include the new sections:

```python
[session]
work_minutes = 6                  # coding minutes before the machine locks

[goals.weekly]                    # weekly rep targets that drive lock prescriptions
squat = 60
bench = 40
row = 40

[break]
default_reps = 10                 # reps prescribed per lift set
jumprope_seconds = 60             # jump-rope payment target
stretch_seconds = 30              # stretch hold target
```

Add to the existing `[detector]` block: `width = 1280` and `height = 720`; to `[display]` (new): `scoreboard_monitor = 1`; to `[lock]`: `override_password = ""`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_config.py -v`
Expected: PASS (all config tests, old and new).

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/config.py tests/test_config.py
git commit -m "feat(config): add session/goals/break/display fields for lock model"
```

---

### Task 2: Remove the obsolete guard/shim gating

**Files:**
- Delete: `src/reps_for_claude/guard.py`, `src/reps_for_claude/shim.py`, `tests/test_guard.py`, `tests/test_shim.py`
- Modify: `src/reps_for_claude/cli.py`
- Test: `tests/test_cli.py`

**Interfaces:**
- Consumes: nothing.
- Produces: `cli.app` no longer exposes `guard`, `install-shim`, `uninstall-shim`. Remaining commands (`init`, `earn`, `analyze`, `status`, `balance`, `finish`) unchanged.

- [ ] **Step 1: Write the failing test**

Add to `tests/test_cli.py`:

```python
def test_guard_and_shim_commands_removed():
    from typer.testing import CliRunner
    from reps_for_claude.cli import app
    runner = CliRunner()
    for cmd in ("guard", "install-shim", "uninstall-shim"):
        result = runner.invoke(app, [cmd, "--help"])
        assert result.exit_code != 0, f"{cmd} should no longer exist"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_cli.py::test_guard_and_shim_commands_removed -v`
Expected: FAIL (the `guard` command still exists, exit_code == 0).

- [ ] **Step 3: Delete modules and trim the CLI**

```bash
git rm src/reps_for_claude/guard.py src/reps_for_claude/shim.py tests/test_guard.py tests/test_shim.py
```

In `src/reps_for_claude/cli.py`:
- Remove imports: `from . import ... shim` (drop `shim` from that line), `from .guard import EXIT_NO_CREDIT, Guard`, `from .lock import get_locker`.
- Delete the entire `guard`, `install_shim`, and `uninstall_shim` command functions (the `@app.command(...)`-decorated blocks for those three).

- [ ] **Step 4: Run the full suite to verify green**

Run: `uv run pytest -v`
Expected: PASS. `test_guard_and_shim_commands_removed` passes; no import errors from the removals.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove guard/shim; superseded by whole-screen lock"
```

---

### Task 3: Weekly-goal math (pure)

**Files:**
- Create: `src/reps_for_claude/goals.py`
- Test: `tests/test_goals.py`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `Prescription(kind: str, exercise: str, target: int)` — frozen dataclass; `kind` is `"lift"`.
  - `progress(targets: dict[str, int], done: dict[str, int]) -> dict[str, float]` — fraction per goal in `[0, 1]`.
  - `most_behind(targets: dict[str, int], done: dict[str, int]) -> str | None` — lowest-completion unmet goal, name tie-break; `None` if all met/empty.
  - `prescribe(targets: dict[str, int], done: dict[str, int], default_reps: int) -> Prescription | None` — auto-pick a lift set of the most-behind goal; `None` when all goals met.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_goals.py`:

```python
from reps_for_claude.goals import Prescription, most_behind, prescribe, progress


class TestProgress:
    def test_fractions(self):
        assert progress({"squat": 60}, {"squat": 30}) == {"squat": 0.5}

    def test_clamped_to_one(self):
        assert progress({"squat": 60}, {"squat": 90}) == {"squat": 1.0}

    def test_missing_is_zero(self):
        assert progress({"squat": 60}, {}) == {"squat": 0.0}


class TestMostBehind:
    def test_picks_lowest_fraction(self):
        # squat 50% done, bench 25% done -> bench
        assert most_behind({"squat": 60, "bench": 40}, {"squat": 30, "bench": 10}) == "bench"

    def test_none_when_all_met(self):
        assert most_behind({"squat": 60}, {"squat": 60}) is None

    def test_none_when_empty(self):
        assert most_behind({}, {}) is None

    def test_name_tiebreak(self):
        assert most_behind({"squat": 10, "bench": 10}, {}) == "bench"


class TestPrescribe:
    def test_lift_of_most_behind(self):
        p = prescribe({"squat": 60, "bench": 40}, {"squat": 30, "bench": 10}, default_reps=10)
        assert p == Prescription(kind="lift", exercise="bench", target=10)

    def test_none_when_goals_met(self):
        assert prescribe({"squat": 60}, {"squat": 60}, default_reps=10) is None
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_goals.py -v`
Expected: FAIL (`ModuleNotFoundError: reps_for_claude.goals`).

- [ ] **Step 3: Implement `goals.py`**

```python
"""Weekly goal math: progress and what to prescribe next. Pure, no IO."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Prescription:
    kind: str      # "lift"
    exercise: str  # the lift name
    target: int    # reps to perform


def progress(targets: dict[str, int], done: dict[str, int]) -> dict[str, float]:
    """Fraction complete per goal, clamped to [0, 1]."""
    out: dict[str, float] = {}
    for exercise, target in targets.items():
        out[exercise] = 1.0 if target <= 0 else min(1.0, done.get(exercise, 0) / target)
    return out


def most_behind(targets: dict[str, int], done: dict[str, int]) -> str | None:
    """The unmet goal with the lowest completion fraction; name tie-break."""
    unmet = [ex for ex, target in targets.items() if done.get(ex, 0) < target]
    if not unmet:
        return None
    return min(unmet, key=lambda ex: (done.get(ex, 0) / targets[ex], ex))


def prescribe(
    targets: dict[str, int], done: dict[str, int], default_reps: int
) -> Prescription | None:
    """Auto-pick a lift set of the most-behind goal; None when all goals met."""
    exercise = most_behind(targets, done)
    if exercise is None:
        return None
    return Prescription(kind="lift", exercise=exercise, target=default_reps)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_goals.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/goals.py tests/test_goals.py
git commit -m "feat(goals): weekly progress + auto-pick prescription"
```

---

### Task 4: Weekly accumulator store

**Files:**
- Create: `src/reps_for_claude/weekly.py`
- Test: `tests/test_weekly.py`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `WeekState(week: str, reps: dict[str, int], volume_lbs: dict[str, float], jumprope_seconds: float, stretch_seconds: float)`.
  - `WeeklyLog(state_dir: Path, *, today: Callable[[], str] = _today)` with `.state: WeekState`, methods `log_lift(exercise, reps, lbs)`, `log_jumprope(seconds)`, `log_stretch(seconds)`, `save()`.
  - ISO-week key format `"YYYY-Www"`; loading in a new ISO week resets to an empty `WeekState`.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_weekly.py`:

```python
from reps_for_claude.weekly import WeeklyLog


def test_log_lift_accumulates_reps_and_volume(tmp_path):
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    log.log_lift("squat", reps=10, lbs=45.0)
    log.log_lift("squat", reps=5, lbs=45.0)
    assert log.state.reps == {"squat": 15}
    assert log.state.volume_lbs == {"squat": 675.0}  # (10+5)*45


def test_persists_and_reloads_same_week(tmp_path):
    WeeklyLog(tmp_path, today=lambda: "2026-07-19").log_jumprope(30.0)
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")  # reload before save? see below
    # nothing saved yet -> fresh
    assert log.state.jumprope_seconds == 0.0

    a = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    a.log_stretch(20.0)
    a.save()
    b = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    assert b.state.stretch_seconds == 20.0


def test_new_iso_week_resets(tmp_path):
    a = WeeklyLog(tmp_path, today=lambda: "2026-07-19")  # ISO week 29
    a.log_lift("squat", reps=10, lbs=45.0)
    a.save()
    b = WeeklyLog(tmp_path, today=lambda: "2026-07-27")  # ISO week 31
    assert b.state.reps == {}
    assert b.state.week == "2026-W31"


def test_corrupt_file_resets(tmp_path, capsys):
    (tmp_path / "weekly.json").write_text("{not json")
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    assert log.state.reps == {}
    assert "corrupt" in capsys.readouterr().err


def test_negative_rejected(tmp_path):
    import pytest
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    with pytest.raises(ValueError):
        log.log_lift("squat", reps=-1, lbs=45.0)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_weekly.py -v`
Expected: FAIL (`ModuleNotFoundError: reps_for_claude.weekly`).

- [ ] **Step 3: Implement `weekly.py`**

```python
"""Per-ISO-week accumulator of reps, lifted volume, and cardio seconds.

One JSON file written atomically (temp + rename), mirroring Ledger. Loading in
a new ISO week resets to an empty week; a corrupt/missing file resets with a
warning and never raises.
"""

from __future__ import annotations

import datetime
import json
import os
import sys
import tempfile
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Callable


def _today() -> str:
    return datetime.date.today().isoformat()


def _iso_week(day_iso: str) -> str:
    year, week, _ = datetime.date.fromisoformat(day_iso).isocalendar()
    return f"{year}-W{week:02d}"


@dataclass
class WeekState:
    week: str
    reps: dict[str, int] = field(default_factory=dict)
    volume_lbs: dict[str, float] = field(default_factory=dict)
    jumprope_seconds: float = 0.0
    stretch_seconds: float = 0.0


class WeeklyLog:
    def __init__(self, state_dir: Path, *, today: Callable[[], str] = _today) -> None:
        self._dir = Path(state_dir)
        self._path = self._dir / "weekly.json"
        self._today = today
        self.state = self._load()

    def _load(self) -> WeekState:
        week = _iso_week(self._today())
        try:
            raw = json.loads(self._path.read_text())
            state = WeekState(
                week=str(raw["week"]),
                reps={str(k): int(v) for k, v in raw["reps"].items()},
                volume_lbs={str(k): float(v) for k, v in raw["volume_lbs"].items()},
                jumprope_seconds=float(raw["jumprope_seconds"]),
                stretch_seconds=float(raw["stretch_seconds"]),
            )
        except FileNotFoundError:
            return WeekState(week=week)
        except (json.JSONDecodeError, KeyError, TypeError, ValueError):
            print(
                f"warning: corrupt weekly file {self._path}; starting a fresh week",
                file=sys.stderr,
            )
            return WeekState(week=week)
        return state if state.week == week else WeekState(week=week)

    def save(self) -> None:
        self._dir.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=self._dir, prefix=".weekly-")
        try:
            with os.fdopen(fd, "w") as f:
                json.dump(asdict(self.state), f, indent=2)
            os.replace(tmp, self._path)
        except BaseException:
            os.unlink(tmp)
            raise

    def log_lift(self, exercise: str, reps: int, lbs: float) -> None:
        if reps < 0 or lbs < 0:
            raise ValueError("reps and lbs must be >= 0")
        self.state.reps[exercise] = self.state.reps.get(exercise, 0) + reps
        self.state.volume_lbs[exercise] = (
            self.state.volume_lbs.get(exercise, 0.0) + reps * lbs
        )

    def log_jumprope(self, seconds: float) -> None:
        if seconds < 0:
            raise ValueError("seconds must be >= 0")
        self.state.jumprope_seconds += seconds

    def log_stretch(self, seconds: float) -> None:
        if seconds < 0:
            raise ValueError("seconds must be >= 0")
        self.state.stretch_seconds += seconds
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_weekly.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/weekly.py tests/test_weekly.py
git commit -m "feat(weekly): ISO-week accumulator for reps, volume, cardio"
```

---

### Task 5: Dues-state store (interface + file impl)

**Files:**
- Create: `src/reps_for_claude/duesstate.py`
- Test: `tests/test_duesstate.py`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `DuesState` — `typing.Protocol` with `owed() -> bool` and `set_owed(owed: bool) -> None`.
  - `FileDuesState(state_dir: Path)` implementing it, persisting `{"owed": bool}` to `dues.json` atomically; a missing/corrupt file reads as **not owed** (`False`). This is the seam Phase 2 replaces with a root-owned authority.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_duesstate.py`:

```python
from reps_for_claude.duesstate import FileDuesState


def test_defaults_to_not_owed(tmp_path):
    assert FileDuesState(tmp_path).owed() is False


def test_roundtrip(tmp_path):
    d = FileDuesState(tmp_path)
    d.set_owed(True)
    assert FileDuesState(tmp_path).owed() is True
    d.set_owed(False)
    assert FileDuesState(tmp_path).owed() is False


def test_corrupt_reads_not_owed(tmp_path):
    (tmp_path / "dues.json").write_text("garbage")
    assert FileDuesState(tmp_path).owed() is False
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_duesstate.py -v`
Expected: FAIL (`ModuleNotFoundError: reps_for_claude.duesstate`).

- [ ] **Step 3: Implement `duesstate.py`**

```python
"""Whether the machine currently owes a workout.

Behind a Protocol so a privileged root authority can replace the file store in
Phase 2. A missing or unreadable file means no dues are owed (fail-open: never
strand the user because of a bad file).
"""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Protocol


class DuesState(Protocol):
    def owed(self) -> bool: ...
    def set_owed(self, owed: bool) -> None: ...


class FileDuesState:
    def __init__(self, state_dir: Path) -> None:
        self._dir = Path(state_dir)
        self._path = self._dir / "dues.json"

    def owed(self) -> bool:
        try:
            return bool(json.loads(self._path.read_text())["owed"])
        except (FileNotFoundError, json.JSONDecodeError, KeyError, TypeError, ValueError):
            return False

    def set_owed(self, owed: bool) -> None:
        self._dir.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=self._dir, prefix=".dues-")
        try:
            with os.fdopen(fd, "w") as f:
                json.dump({"owed": bool(owed)}, f)
            os.replace(tmp, self._path)
        except BaseException:
            os.unlink(tmp)
            raise
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_duesstate.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/duesstate.py tests/test_duesstate.py
git commit -m "feat(duesstate): file-backed dues flag behind a Protocol"
```

---

### Task 6: Break-activity interface + lift detector

**Files:**
- Create: `src/reps_for_claude/activities/__init__.py`, `src/reps_for_claude/activities/base.py`, `src/reps_for_claude/activities/lift.py`
- Test: `tests/test_activities_lift.py`

**Interfaces:**
- Consumes: `exercises.get_spec`, `exercises.Landmarks`, `angles.RepStateMachine` (existing).
- Produces:
  - `Progress(value: float, unit: str, satisfied: bool)` — frozen dataclass; `unit` is `"reps"` or `"seconds"`.
  - `BreakActivity` — `Protocol` with `update(landmarks: Landmarks | None, now: float) -> Progress`.
  - `LiftActivity(exercise: str, target_reps: int)` — counts reps via the existing angle state machine; `satisfied` once `count >= target_reps`. `None` landmarks (no pose that frame) advance nothing.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_activities_lift.py`:

```python
from reps_for_claude.activities.base import Progress
from reps_for_claude.activities.lift import LiftActivity


def _pose(knee_angle_deg: float):
    """A minimal side-on landmark set that yields the given knee angle.

    Hip at origin, knee straight below it, ankle placed so the hip-knee-ankle
    angle equals `knee_angle_deg`. Coordinates are normalized (0..1) with
    visibility 1.0. Only the squat's KNEE joints need to be present.
    """
    import math
    hip = (0.5, 0.4, 1.0)
    knee = (0.5, 0.6, 1.0)
    rad = math.radians(knee_angle_deg)
    ankle = (0.5 + 0.2 * math.sin(rad), 0.6 + 0.2 * math.cos(rad), 1.0)
    return {"left_hip": hip, "left_knee": knee, "left_ankle": ankle}


def test_counts_one_squat_rep():
    act = LiftActivity("squat", target_reps=2)
    # squat: down_below=110, up_above=160. Start up, go down, come up = 1 rep.
    assert act.update(_pose(170), now=0.0).value == 0.0   # up
    assert act.update(_pose(100), now=1.0).value == 0.0   # down (no rep yet)
    p = act.update(_pose(170), now=2.0)                    # back up -> rep!
    assert p == Progress(1.0, "reps", False)


def test_satisfied_at_target():
    act = LiftActivity("squat", target_reps=1)
    act.update(_pose(170), now=0.0)
    act.update(_pose(100), now=1.0)
    p = act.update(_pose(170), now=2.0)
    assert p.satisfied is True


def test_none_landmarks_advance_nothing():
    act = LiftActivity("squat", target_reps=1)
    p = act.update(None, now=0.0)
    assert p == Progress(0.0, "reps", False)
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_activities_lift.py -v`
Expected: FAIL (`ModuleNotFoundError: reps_for_claude.activities`).

- [ ] **Step 3: Implement the package, base, and lift**

Create `src/reps_for_claude/activities/__init__.py`:

```python
"""Streaming break activities: fed pose frames or clock ticks, report progress."""
```

Create `src/reps_for_claude/activities/base.py`:

```python
"""The BreakActivity contract: one frame in, live Progress out."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from ..exercises import Landmarks


@dataclass(frozen=True)
class Progress:
    value: float   # reps or seconds accumulated
    unit: str      # "reps" | "seconds"
    satisfied: bool


class BreakActivity(Protocol):
    def update(self, landmarks: Landmarks | None, now: float) -> Progress: ...
```

Create `src/reps_for_claude/activities/lift.py`:

```python
"""Rep-counting break activity built on the existing angle state machine."""

from __future__ import annotations

from ..angles import RepStateMachine
from ..exercises import Landmarks, get_spec
from .base import Progress


class LiftActivity:
    def __init__(self, exercise: str, target_reps: int) -> None:
        self._spec = get_spec(exercise)
        self._machine = RepStateMachine(self._spec.down_below, self._spec.up_above)
        self._target = target_reps
        self._count = 0

    def update(self, landmarks: Landmarks | None, now: float) -> Progress:
        if landmarks is not None:
            angle = self._spec.angle_from(landmarks)
            if angle is not None and self._machine.update(angle):
                self._count += 1
        return Progress(float(self._count), "reps", self._count >= self._target)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_activities_lift.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/activities/ tests/test_activities_lift.py
git commit -m "feat(activities): streaming BreakActivity + LiftActivity"
```

---

### Task 7: Jump-rope activity (motion-gated timer)

**Files:**
- Create: `src/reps_for_claude/activities/jumprope.py`
- Test: `tests/test_activities_jumprope.py`

**Interfaces:**
- Consumes: `activities.base.Progress`, `exercises.Landmarks`.
- Produces: `JumpRopeActivity(target_seconds: float, *, bounce_threshold: float = 0.015, reset_after: float = 2.0)`. Uses mean hip `y`. Frame-to-frame vertical change ≥ `bounce_threshold` counts as motion and accrues `dt` to the streak; being still (below threshold) for `reset_after` seconds zeroes the streak. `satisfied` once streak ≥ `target_seconds`.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_activities_jumprope.py`:

```python
from reps_for_claude.activities.jumprope import JumpRopeActivity


def _hips(y: float):
    return {"left_hip": (0.5, y, 1.0), "right_hip": (0.5, y, 1.0)}


def test_bouncing_accrues_time():
    act = JumpRopeActivity(target_seconds=3.0)
    act.update(_hips(0.50), now=0.0)          # first frame: seeds baseline
    p = act.update(_hips(0.40), now=1.0)      # moved 0.10 >= threshold -> +1s
    assert p.value == 1.0 and not p.satisfied
    p = act.update(_hips(0.50), now=2.0)      # moved -> +1s
    p = act.update(_hips(0.40), now=3.0)      # moved -> +1s -> streak 3.0
    assert p.value == 3.0 and p.satisfied


def test_stillness_resets_streak():
    act = JumpRopeActivity(target_seconds=10.0, reset_after=2.0)
    act.update(_hips(0.50), now=0.0)
    act.update(_hips(0.40), now=1.0)          # streak 1.0
    act.update(_hips(0.40), now=2.0)          # still (dt within reset window)
    p = act.update(_hips(0.40), now=4.0)      # still >= reset_after -> reset
    assert p.value == 0.0


def test_none_landmarks_treated_as_still():
    act = JumpRopeActivity(target_seconds=10.0, reset_after=2.0)
    act.update(_hips(0.50), now=0.0)
    p = act.update(None, now=1.0)
    assert p.value == 0.0 and p.unit == "seconds"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_activities_jumprope.py -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3: Implement `jumprope.py`**

```python
"""Jump-rope time-in-a-row via pose bounce detection.

Honest limitation: this measures sustained vertical motion, not individual
rope swings. Good enough to gate an unlock; tune thresholds against real
C920x footage before trusting the exact seconds.
"""

from __future__ import annotations

from ..exercises import Landmarks
from .base import Progress


class JumpRopeActivity:
    def __init__(
        self,
        target_seconds: float,
        *,
        bounce_threshold: float = 0.015,
        reset_after: float = 2.0,
    ) -> None:
        self._target = target_seconds
        self._threshold = bounce_threshold
        self._reset_after = reset_after
        self._streak = 0.0
        self._last_now: float | None = None
        self._last_y: float | None = None
        self._still_since: float | None = None

    def update(self, landmarks: Landmarks | None, now: float) -> Progress:
        y = self._body_y(landmarks)
        if self._last_now is None:
            self._last_now, self._last_y = now, y
            return Progress(0.0, "seconds", False)

        dt = now - self._last_now
        moving = (
            y is not None
            and self._last_y is not None
            and abs(y - self._last_y) >= self._threshold
        )
        if moving:
            self._still_since = None
            self._streak += dt
        else:
            if self._still_since is None:
                self._still_since = now
            elif now - self._still_since >= self._reset_after:
                self._streak = 0.0
        self._last_now, self._last_y = now, y
        return Progress(self._streak, "seconds", self._streak >= self._target)

    @staticmethod
    def _body_y(landmarks: Landmarks | None) -> float | None:
        if not landmarks:
            return None
        pts = [landmarks.get("left_hip"), landmarks.get("right_hip")]
        ys = [p[1] for p in pts if p is not None]
        return sum(ys) / len(ys) if ys else None
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_activities_jumprope.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/activities/jumprope.py tests/test_activities_jumprope.py
git commit -m "feat(activities): jump-rope motion-gated timer"
```

---

### Task 8: Stretch activity (timed hold)

**Files:**
- Create: `src/reps_for_claude/activities/stretch.py`
- Test: `tests/test_activities_stretch.py`

**Interfaces:**
- Consumes: `activities.base.Progress`, `exercises.Landmarks`.
- Produces: `StretchActivity(target_seconds: float)`. Honor-start: the timer begins on the first `update` and reports elapsed wall-clock; `satisfied` once held ≥ `target_seconds`. Landmarks are ignored (no posture detection in Phase 1).

- [ ] **Step 1: Write the failing tests**

Create `tests/test_activities_stretch.py`:

```python
from reps_for_claude.activities.stretch import StretchActivity


def test_hold_accumulates_from_first_update():
    act = StretchActivity(target_seconds=30.0)
    assert act.update(None, now=100.0).value == 0.0     # starts the clock
    p = act.update(None, now=115.0)
    assert p.value == 15.0 and not p.satisfied


def test_satisfied_after_target():
    act = StretchActivity(target_seconds=30.0)
    act.update(None, now=0.0)
    p = act.update(None, now=30.0)
    assert p.value == 30.0 and p.satisfied and p.unit == "seconds"
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `uv run pytest tests/test_activities_stretch.py -v`
Expected: FAIL (`ModuleNotFoundError`).

- [ ] **Step 3: Implement `stretch.py`**

```python
"""Timed-hold stretch activity (honor-start; no posture detection in Phase 1)."""

from __future__ import annotations

from ..exercises import Landmarks
from .base import Progress


class StretchActivity:
    def __init__(self, target_seconds: float) -> None:
        self._target = target_seconds
        self._start: float | None = None

    def update(self, landmarks: Landmarks | None, now: float) -> Progress:
        if self._start is None:
            self._start = now
        held = now - self._start
        return Progress(held, "seconds", held >= self._target)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_activities_stretch.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/activities/stretch.py tests/test_activities_stretch.py
git commit -m "feat(activities): timed-hold stretch"
```

---

### Task 9: Weekly-volume report renderer

**Files:**
- Modify: `src/reps_for_claude/report.py`
- Test: `tests/test_report.py`

**Interfaces:**
- Consumes: nothing new.
- Produces: `render_weekly_markdown(week: str, targets: dict[str, int], reps: dict[str, int], volume_lbs: dict[str, float], jumprope_seconds: float) -> str` — a trainer-readable table of weekly targets vs. reps vs. lifted volume, plus jump-rope minutes. Pure; the CLI wiring lands in a later plan.

- [ ] **Step 1: Write the failing test**

Add to `tests/test_report.py`:

```python
def test_render_weekly_markdown():
    from reps_for_claude.report import render_weekly_markdown
    md = render_weekly_markdown(
        week="2026-W29",
        targets={"squat": 60, "bench": 40},
        reps={"squat": 45, "bench": 40},
        volume_lbs={"squat": 2025.0, "bench": 3200.0},
        jumprope_seconds=180.0,
    )
    assert "# Weekly Volume — 2026-W29" in md
    assert "| squat | 60 | 45 | 2025 |" in md
    assert "| bench | 40 | 40 | 3200 |" in md
    assert "Jump rope: 3.0 min" in md
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_report.py::test_render_weekly_markdown -v`
Expected: FAIL (`ImportError: cannot import name 'render_weekly_markdown'`).

- [ ] **Step 3: Implement the renderer**

Add to `src/reps_for_claude/report.py`:

```python
def render_weekly_markdown(
    week: str,
    targets: dict[str, int],
    reps: dict[str, int],
    volume_lbs: dict[str, float],
    jumprope_seconds: float,
) -> str:
    lines = [
        f"# Weekly Volume — {week}",
        "",
        "| Exercise | Target | Reps | Volume (lbs) |",
        "|---|---|---|---|",
    ]
    for exercise in sorted(set(targets) | set(reps)):
        target = targets.get(exercise, "—")
        lines.append(
            f"| {exercise} | {target} | {reps.get(exercise, 0)} "
            f"| {volume_lbs.get(exercise, 0.0):.0f} |"
        )
    lines += ["", f"- Jump rope: {jumprope_seconds / 60:.1f} min", ""]
    return "\n".join(lines)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `uv run pytest tests/test_report.py -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/reps_for_claude/report.py tests/test_report.py
git commit -m "feat(report): weekly-volume markdown renderer"
```

---

### Task 10: Full-suite green + spec parity check

**Files:**
- Test: whole suite.

- [ ] **Step 1: Run the entire suite**

Run: `uv run pytest -v`
Expected: PASS, no errors, no import failures from the guard/shim removal. Confirm the new test files (`test_goals`, `test_weekly`, `test_duesstate`, `test_activities_lift`, `test_activities_jumprope`, `test_activities_stretch`) and the config/report additions all pass.

- [ ] **Step 2: Confirm no stale references**

Run: `git grep -n "import.*\bguard\b\|import.*\bshim\b\|get_locker" -- src tests`
Expected: no output (all references removed). If any remain, fix and re-run the suite.

- [ ] **Step 3: Commit any fixups (if needed)**

```bash
git add -A && git commit -m "chore: Plan A green — headless core complete"
```

(Skip if Step 2 was clean and nothing changed.)

---

## Self-review (against the spec)

- **Config** (`[session]`, `[goals.weekly]`, `[break]`, `[display]`, `[detector]` w/h, `[lock].override_password`) → Task 1. ✓
- **Remove `guard`/`shim`** → Task 2. ✓
- **`goals.py`** (weekly progress, auto-pick most-behind) → Task 3. ✓
- **Weekly progress store** (reps + lbs volume + cardio, ISO-week reset) → Task 4. ✓
- **`duesstate.py`** (`DuesState` Protocol + file impl, Phase-2 seam) → Task 5. ✓
- **`BreakActivity` + Lift/JumpRope/Stretch** (streaming progress) → Tasks 6–8. ✓
- **Report weekly volume (lbs)** → Task 9. ✓
- **Hermetic `REPS_HOME`, atomic writes, no new deps** → Global Constraints + per-task. ✓

**Deferred to later plans (not gaps):** `locker` (xsecurelock), `session` state machine, `reps session` CLI, and removing the legacy `balance/cap`/`economics`/`earn` surface → **Plan B**. The `server` + web scoreboard/lock UI + GSAP mascot → **Plan C**. These are intentionally out of Plan A's headless scope.

## What Plan B and Plan C cover (outline)

**Plan B — Lock & session loop:**
- `locker.py` — drive `xsecurelock` (custom saver → our lock URL; custom auth → unlock when `DuesState.owed()` is false or the override password matches), with a fake-runner test and missing-binary degradation. Extends today's `lock.py`.
- `session.py` — the CODING→LOCKED→PAID state machine (fake clock, fake locker, fake activity, `DuesState`, `WeeklyLog`); resume/release-on-crash via try/finally.
- `cli.py` — `reps session` command; retire the legacy `earn`/`status`/`balance` balance surface and trim `economics` to what `goals` needs.

**Plan C — Web scoreboard + GSAP lock UI:**
- `server.py` — localhost HTTP + WebSocket streaming session state.
- `web/` — `scoreboard.html` (⌛ + weekly-goal bars + today's totals + streak) and `lock.html` (prescribed lift / jump rope, live counter, GSAP Claude mascot), vendored GSAP, shared WS client.
- Wire the browser (Chrome kiosk) as the `xsecurelock` saver on the scoreboard monitor.
