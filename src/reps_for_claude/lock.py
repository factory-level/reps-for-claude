"""Desktop-lock adapters. Failure degrades to shim-only enforcement."""

from __future__ import annotations

import subprocess
import sys
from typing import Callable, Protocol

LOCK_COMMANDS: list[list[str]] = [
    ["loginctl", "lock-session"],
    ["gnome-screensaver-command", "--lock"],
    ["xdg-screensaver", "lock"],
]


class Locker(Protocol):
    def lock(self) -> bool: ...


class NoOpLocker:
    def lock(self) -> bool:
        return False


class CommandLocker:
    """Try each known lock command until one succeeds."""

    def __init__(self, runner: Callable[..., object] | None = None) -> None:
        self._run = runner if runner is not None else subprocess.run

    def lock(self) -> bool:
        for cmd in LOCK_COMMANDS:
            try:
                result = self._run(cmd, capture_output=True)
                if getattr(result, "returncode", 1) == 0:
                    return True
            except FileNotFoundError:
                continue
        print(
            "warning: could not lock the desktop; falling back to shim-only enforcement",
            file=sys.stderr,
        )
        return False


def get_locker(enabled: bool, runner: Callable[..., object] | None = None) -> Locker:
    return CommandLocker(runner) if enabled else NoOpLocker()
