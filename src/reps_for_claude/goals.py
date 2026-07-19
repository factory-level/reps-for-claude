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
        out[exercise] = 1.0 if target <= 0 else min(1.0, max(0.0, done.get(exercise, 0) / target))
    return out


def most_behind(targets: dict[str, int], done: dict[str, int]) -> str | None:
    """The unmet goal with the lowest completion fraction; name tie-break."""
    unmet = [ex for ex, target in targets.items() if target > 0 and done.get(ex, 0) < target]
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
