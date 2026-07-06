"""The ledger: single source of truth for balance, reps, and day state.

State is one JSON file written atomically (temp file + rename). Corrupt or
missing state resets to a safe empty day — the guard must never crash because
of a bad state file. On day rollover, reps and spend reset; the balance
carries over but is clamped to the pre-completion cap (the new day's workout
isn't done yet).
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


@dataclass
class DayState:
    date: str
    balance_seconds: float = 0.0
    spent_seconds: float = 0.0
    reps: dict[str, int] = field(default_factory=dict)
    report_done: bool = False


class Ledger:
    def __init__(
        self,
        state_dir: Path,
        *,
        rollover_cap: float | None = None,
        today: Callable[[], str] = _today,
    ) -> None:
        self._dir = Path(state_dir)
        self._path = self._dir / "state.json"
        self._today = today
        self._rollover_cap = rollover_cap
        self.state = self._load()

    # -- persistence ---------------------------------------------------

    def _load(self) -> DayState:
        try:
            raw = json.loads(self._path.read_text())
            state = DayState(
                date=str(raw["date"]),
                balance_seconds=float(raw["balance_seconds"]),
                spent_seconds=float(raw["spent_seconds"]),
                reps={str(k): int(v) for k, v in raw["reps"].items()},
                report_done=bool(raw["report_done"]),
            )
        except FileNotFoundError:
            return DayState(date=self._today())
        except (json.JSONDecodeError, KeyError, TypeError, ValueError):
            print(
                f"warning: corrupt state file {self._path}; starting a fresh day",
                file=sys.stderr,
            )
            return DayState(date=self._today())

        if state.date != self._today():
            balance = state.balance_seconds
            if self._rollover_cap is not None:
                balance = min(balance, self._rollover_cap)
            return DayState(date=self._today(), balance_seconds=balance)
        return state

    def save(self) -> None:
        self._dir.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=self._dir, prefix=".state-")
        try:
            with os.fdopen(fd, "w") as f:
                json.dump(asdict(self.state), f, indent=2)
            os.replace(tmp, self._path)
        except BaseException:
            os.unlink(tmp)
            raise

    # -- mutations -----------------------------------------------------

    def add_reps(self, exercise: str, count: int) -> None:
        if count < 0:
            raise ValueError("count must be >= 0")
        self.state.reps[exercise] = self.state.reps.get(exercise, 0) + count

    def set_balance(self, seconds: float) -> None:
        self.state.balance_seconds = max(0.0, seconds)

    def spend(self, seconds: float) -> None:
        """Consume balance; clamps at zero and tracks total spend."""
        if seconds < 0:
            raise ValueError("seconds must be >= 0")
        charged = min(seconds, self.state.balance_seconds)
        self.state.balance_seconds -= charged
        self.state.spent_seconds += charged

    def mark_report_done(self) -> None:
        self.state.report_done = True
