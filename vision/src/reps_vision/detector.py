"""Rep counters behind one interface, so CV models can slot in later.

A RepCounter runs one earn session and returns the number of reps counted,
invoking `on_rep(total_so_far)` as each rep lands. Two implementations ship
now: a deterministic stub for tests, and a keyboard counter usable today
(one Enter per rep) until a webcam model is chosen.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Callable, TextIO

OnRep = Callable[[int], None]


class DetectorError(Exception):
    """Raised when a detector cannot run (e.g. no camera, unknown name)."""


class RepCounter(ABC):
    name: str = "abstract"

    @abstractmethod
    def run(self, exercise: str, on_rep: OnRep) -> int:
        """Count reps for one session; return the total."""


class StubDetector(RepCounter):
    """Deterministic counter for tests: emits a fixed number of reps."""

    name = "stub"

    def __init__(self, total_reps: int) -> None:
        self.total_reps = total_reps

    def run(self, exercise: str, on_rep: OnRep) -> int:
        for i in range(1, self.total_reps + 1):
            on_rep(i)
        return self.total_reps


class KeyboardDetector(RepCounter):
    """Interim manual counter: press Enter per rep; 'done' or EOF to finish."""

    name = "keyboard"

    def __init__(self, stream: TextIO | None = None) -> None:
        import sys

        self._stream = stream if stream is not None else sys.stdin

    def run(self, exercise: str, on_rep: OnRep) -> int:
        count = 0
        for line in self._stream:
            if line.strip().lower() in ("done", "q", "quit"):
                break
            count += 1
            on_rep(count)
        return count


_FUTURE_CV_NAMES = ("yolo", "yolo-pose")


def get_detector(name: str, **kwargs: object) -> RepCounter:
    if name == "stub":
        return StubDetector(int(kwargs.get("total_reps", 0)))  # type: ignore[call-overload]
    if name == "keyboard":
        return KeyboardDetector()
    if name == "mediapipe":
        from .video import VideoRepCounter

        return VideoRepCounter(
            int(kwargs.get("camera_index", 0)),  # type: ignore[call-overload]
            show=bool(kwargs.get("show", True)),
        )
    if name in _FUTURE_CV_NAMES:
        raise DetectorError(
            f"detector {name!r} is not implemented yet — use 'mediapipe' "
            "(see design spec)"
        )
    raise DetectorError(f"unknown detector {name!r}")
