"""Run one earn session: detector -> economics -> ledger."""

from __future__ import annotations

from dataclasses import dataclass

from . import economics
from .config import Config
from .detector import OnRep, RepCounter
from .ledger import Ledger


@dataclass
class EarnResult:
    exercise: str
    reps: int
    credited_seconds: float
    balance_seconds: float
    workout_complete: bool
    capped: bool


def earn(
    exercise: str,
    detector: RepCounter,
    ledger: Ledger,
    config: Config,
    on_rep: OnRep = lambda n: None,
) -> EarnResult:
    """Count reps, log them, and credit the balance (subject to the cap).

    Reps are logged first, so a session that finishes the daily plan earns
    its own credit at the uncapped rate. Zero reps changes nothing.
    """
    reps = detector.run(exercise, on_rep)
    if reps <= 0:
        complete = economics.is_workout_complete(config.plan, ledger.state.reps)
        return EarnResult(
            exercise, 0, 0.0, ledger.state.balance_seconds, complete, False
        )

    ledger.add_reps(exercise, reps)
    complete = economics.is_workout_complete(config.plan, ledger.state.reps)
    earned = economics.credit_for_reps(reps, config.seconds_per_rep)
    before = ledger.state.balance_seconds
    after = economics.apply_earn(
        before, earned, complete, float(config.precompletion_cap_seconds)
    )
    ledger.set_balance(after)
    ledger.save()
    credited = after - before
    return EarnResult(exercise, reps, credited, after, complete, credited < earned)
