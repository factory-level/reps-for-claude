"""Pure credit economics: no IO, fully unit-testable.

Rules:
- Each rep earns `seconds_per_rep` of Claude time.
- Until the daily plan is complete, the banked balance is hard-capped at
  `precompletion_cap_seconds`. Earning never *reduces* an existing balance
  (e.g. one carried over from a completed prior day) — the cap only limits
  what new reps can add.
- An empty plan counts as complete (no plan, no cap).
"""

from __future__ import annotations


def credit_for_reps(reps: int, seconds_per_rep: int) -> float:
    """Seconds of credit earned by `reps` reps, before any cap."""
    if reps < 0:
        raise ValueError("reps must be >= 0")
    return float(reps * seconds_per_rep)


def is_workout_complete(plan: dict[str, int], reps: dict[str, int]) -> bool:
    """True when every exercise in the plan has met its target."""
    return all(reps.get(exercise, 0) >= target for exercise, target in plan.items())


def apply_earn(
    balance: float, earned: float, complete: bool, cap: float
) -> float:
    """New balance after earning `earned` seconds.

    Post-completion: uncapped. Pre-completion: capped at `cap`, but never
    below the existing balance.
    """
    if complete:
        return balance + earned
    return max(balance, min(balance + earned, cap))
