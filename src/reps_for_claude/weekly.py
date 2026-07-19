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
