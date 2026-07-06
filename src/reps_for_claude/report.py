"""End-of-day Form report: review/edit counts, write Markdown + JSON."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Callable

from . import economics
from .config import Config
from .ledger import Ledger


def _fmt_minutes(seconds: float) -> str:
    return f"{seconds / 60:.1f} min"


def render_markdown(
    date: str,
    plan: dict[str, int],
    reps: dict[str, int],
    spent_seconds: float,
    balance_seconds: float,
    complete: bool,
) -> str:
    lines = [
        f"# Workout Form — {date}",
        "",
        "| Exercise | Target | Completed |",
        "|---|---|---|",
    ]
    for exercise in sorted(set(plan) | set(reps)):
        target = plan.get(exercise, "—")
        lines.append(f"| {exercise} | {target} | {reps.get(exercise, 0)} |")
    lines += [
        "",
        f"- Workout complete: {'yes' if complete else 'no'}",
        f"- Claude time spent: {_fmt_minutes(spent_seconds)}",
        f"- Credit remaining: {_fmt_minutes(balance_seconds)}",
        "",
    ]
    return "\n".join(lines)


def review_counts(
    plan: dict[str, int],
    reps: dict[str, int],
    input_fn: Callable[[str], str],
    print_fn: Callable[[str], None],
) -> tuple[dict[str, int], bool]:
    """Let the user edit each count; blank keeps. Returns (counts, edited)."""
    edited = False
    final: dict[str, int] = {}
    print_fn("Review today's counts (blank keeps the counted value):")
    for exercise in sorted(set(plan) | set(reps)):
        current = reps.get(exercise, 0)
        try:
            answer = input_fn(f"  {exercise} [{current}]: ").strip()
        except EOFError:
            answer = ""
        if not answer:
            final[exercise] = current
            continue
        try:
            value = int(answer)
            if value < 0:
                raise ValueError
        except ValueError:
            print_fn(f"  invalid value {answer!r}; keeping {current}")
            final[exercise] = current
            continue
        final[exercise] = value
        edited = edited or value != current
    return final, edited


def finish(
    ledger: Ledger,
    config: Config,
    logs_dir: Path,
    input_fn: Callable[[str], str] = input,
    print_fn: Callable[[str], None] = print,
) -> tuple[Path, Path]:
    """Run the review flow, write the Form files, mark the day reported."""
    state = ledger.state
    counts, edited = review_counts(config.plan, state.reps, input_fn, print_fn)
    if edited:
        state.reps = dict(counts)
    complete = economics.is_workout_complete(config.plan, state.reps)

    logs_dir.mkdir(parents=True, exist_ok=True)
    md_path = logs_dir / f"{state.date}.md"
    json_path = logs_dir / f"{state.date}.json"

    md_path.write_text(
        render_markdown(
            state.date,
            config.plan,
            state.reps,
            state.spent_seconds,
            state.balance_seconds,
            complete,
        )
    )
    json_path.write_text(
        json.dumps(
            {
                "date": state.date,
                "plan": config.plan,
                "reps": state.reps,
                "workout_complete": complete,
                "spent_seconds": state.spent_seconds,
                "balance_seconds": state.balance_seconds,
                "edited": edited,
            },
            indent=2,
        )
    )
    ledger.mark_report_done()
    ledger.save()
    print_fn(f"Form written: {md_path}")
    return md_path, json_path
