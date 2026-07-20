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
