"""Run a command under the credit meter.

The `claude` shim execs `reps guard -- <real-claude> ...`, which lands here.
Balance is checked before launch, decremented while the child runs (via a
poll loop), and persisted on every tick so a crash never loses accounting.
When the balance hits zero mid-session the child is terminated and, if
enabled, the desktop locks.
"""

from __future__ import annotations

import math
import subprocess
import sys
import time
from typing import Callable

from .config import Config
from .ledger import Ledger
from .lock import Locker

EXIT_NO_CREDIT = 3
EXIT_EXHAUSTED = 4


def reps_needed(deficit_seconds: float, seconds_per_rep: int) -> int:
    """Minimum reps to cover a credit deficit."""
    return max(1, math.ceil(deficit_seconds / seconds_per_rep))


class Guard:
    def __init__(
        self,
        ledger: Ledger,
        config: Config,
        locker: Locker,
        *,
        popen: Callable[..., object] = subprocess.Popen,
        monotonic: Callable[[], float] = time.monotonic,
        sleep: Callable[[float], None] = time.sleep,
        tick_seconds: float = 5.0,
        err: Callable[[str], None] = lambda m: print(m, file=sys.stderr),
    ) -> None:
        self._ledger = ledger
        self._config = config
        self._locker = locker
        self._popen = popen
        self._monotonic = monotonic
        self._sleep = sleep
        self._tick = tick_seconds
        self._err = err

    def run(self, argv: list[str]) -> int:
        ledger = self._ledger
        if ledger.state.balance_seconds <= 0:
            need = reps_needed(
                float(self._config.seconds_per_rep), self._config.seconds_per_rep
            )
            self._err(
                "No Claude credit banked. "
                f"Do some reps first: `reps earn <exercise>` (~{need} rep minimum)."
            )
            return EXIT_NO_CREDIT

        proc = self._popen(argv)
        last = self._monotonic()

        def charge() -> None:
            nonlocal last
            now = self._monotonic()
            ledger.spend(now - last)
            last = now
            ledger.save()

        try:
            while True:
                rc = proc.poll()  # type: ignore[attr-defined]
                charge()
                if rc is not None:
                    return rc
                if ledger.state.balance_seconds <= 0:
                    proc.terminate()  # type: ignore[attr-defined]
                    proc.wait()  # type: ignore[attr-defined]
                    self._err(
                        "Claude credit exhausted — session stopped. "
                        "Earn more with `reps earn <exercise>`."
                    )
                    self._locker.lock()
                    return EXIT_EXHAUSTED
                self._sleep(self._tick)
        finally:
            charge()
