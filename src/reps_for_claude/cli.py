"""Command-line interface: earn, status, balance, finish, guard, shim mgmt."""

from __future__ import annotations

import os
from pathlib import Path

import typer

from . import config as config_mod
from . import detector as detector_mod
from . import economics, report, shim
from .earn import earn as run_earn
from .guard import EXIT_NO_CREDIT, Guard
from .ledger import Ledger
from .lock import get_locker

app = typer.Typer(no_args_is_help=True, add_completion=False)


def _load() -> tuple[config_mod.Config, Ledger]:
    cfg = config_mod.load()
    ledger = Ledger(
        config_mod.state_dir(),
        rollover_cap=float(cfg.precompletion_cap_seconds),
    )
    return cfg, ledger


def _fmt(seconds: float) -> str:
    minutes, secs = divmod(int(seconds), 60)
    return f"{minutes}m {secs:02d}s"


@app.command()
def init() -> None:
    """Write a sample config file (if none exists)."""
    path = config_mod.write_sample()
    typer.echo(f"Config: {path}")


@app.command()
def earn(exercise: str) -> None:
    """Do reps for EXERCISE and bank Claude time."""
    cfg, ledger = _load()
    try:
        det = detector_mod.get_detector(cfg.detector, camera_index=cfg.camera_index)
    except detector_mod.DetectorError as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2)
    if cfg.detector == "keyboard":
        typer.echo(f"Counting {exercise} — press Enter per rep, 'done' to finish.")
    result = run_earn(
        exercise, det, ledger, cfg, on_rep=lambda n: typer.echo(f"  rep {n}")
    )
    typer.echo(
        f"{result.reps} {exercise} logged; +{_fmt(result.credited_seconds)} "
        f"(balance {_fmt(result.balance_seconds)})"
    )
    if result.capped:
        typer.echo(
            "Balance is capped until today's workout is complete — "
            "see `reps status`."
        )
    if result.workout_complete:
        typer.echo("Daily workout complete! Credit is now uncapped.")


@app.command()
def analyze(
    video: Path = typer.Argument(..., help="Video file to analyze."),
    exercise: str = typer.Option(..., "--exercise", "-e", help="Exercise to count."),
    output: Path = typer.Option(
        None, "--output", "-o", help="Write an annotated visualization video here."
    ),
    show: bool = typer.Option(
        False, "--show/--no-show", help="Pop up a live detection window (press q to stop)."
    ),
) -> None:
    """Count reps in a video file (analysis only — never credits the ledger)."""
    if not video.exists():
        typer.echo(f"error: no such file: {video}", err=True)
        raise typer.Exit(2)
    from .video import VideoRepCounter

    counter = VideoRepCounter(str(video), show=show, annotate=bool(output))
    on_frame, finalize = _writer_sink(video, output)
    try:
        total = counter.run(
            exercise, on_rep=lambda n: typer.echo(f"  rep {n}"), on_frame=on_frame
        )
    except detector_mod.DetectorError as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2)
    finalize()
    typer.echo(f"{total} {exercise} rep(s) detected in {video.name}")
    for i, ms in enumerate(counter.rep_timestamps_ms, 1):
        typer.echo(f"  rep {i} at {ms / 1000:.1f}s")
    if output:
        typer.echo(f"visualization written: {output}")
    typer.echo("(analysis only — no credit banked)")


def _writer_sink(video, output):
    """Return (on_frame, finalize) that writes annotated frames to `output`.

    The counter draws the overlay; this sink only encodes. No output -> a
    no-op pair. The writer is created lazily on the first frame (once its size
    is known) and reuses the input's frame rate for playback-accurate timing.
    """
    if output is None:
        return None, lambda: None

    import cv2

    probe = cv2.VideoCapture(str(video))
    fps = probe.get(cv2.CAP_PROP_FPS) or 25.0
    probe.release()
    output.parent.mkdir(parents=True, exist_ok=True)
    fourcc = cv2.VideoWriter_fourcc(*"mp4v")
    writer: dict[str, object] = {}

    def on_frame(frame, landmarks, angle, count):
        if "w" not in writer:
            h, w = frame.shape[:2]
            writer["w"] = cv2.VideoWriter(str(output), fourcc, fps, (w, h))
        writer["w"].write(frame)  # type: ignore[union-attr]
        return False

    def finalize():
        if "w" in writer:
            writer["w"].release()  # type: ignore[union-attr]

    return on_frame, finalize


@app.command()
def status() -> None:
    """Show plan progress, balance, and cap state."""
    cfg, ledger = _load()
    state = ledger.state
    complete = economics.is_workout_complete(cfg.plan, state.reps)
    typer.echo(f"Date: {state.date}")
    typer.echo(f"Balance: {_fmt(state.balance_seconds)}")
    typer.echo(f"Spent today: {_fmt(state.spent_seconds)}")
    if cfg.plan:
        typer.echo("Plan:")
        for exercise, target in sorted(cfg.plan.items()):
            done = state.reps.get(exercise, 0)
            mark = "✓" if done >= target else " "
            typer.echo(f"  [{mark}] {exercise}: {done}/{target}")
    extras = sorted(set(state.reps) - set(cfg.plan))
    for exercise in extras:
        typer.echo(f"  [+] {exercise}: {state.reps[exercise]} (off-plan)")
    if complete:
        typer.echo("Workout complete — credit uncapped.")
    else:
        typer.echo(
            f"Workout incomplete — balance capped at "
            f"{_fmt(cfg.precompletion_cap_seconds)}."
        )


@app.command()
def balance() -> None:
    """Show the current credit balance."""
    _, ledger = _load()
    typer.echo(_fmt(ledger.state.balance_seconds))


@app.command()
def finish() -> None:
    """Review today's counts and write the Form report for your trainer."""
    cfg, ledger = _load()
    report.finish(ledger, cfg, config_mod.state_dir() / "logs")


@app.command(
    context_settings={"allow_extra_args": True, "ignore_unknown_options": True}
)
def guard(ctx: typer.Context) -> None:
    """Run a command under the credit meter: reps guard -- claude ..."""
    cfg, ledger = _load()
    argv = list(ctx.args)
    if not argv:
        real = shim.find_real_claude(cfg)
        if real is None:
            typer.echo("error: no command given and real claude not found", err=True)
            raise typer.Exit(EXIT_NO_CREDIT)
        argv = [real]
    tick = float(os.environ.get("REPS_GUARD_TICK", "5"))
    g = Guard(ledger, cfg, get_locker(cfg.lock_enabled), tick_seconds=tick)
    raise typer.Exit(g.run(argv))


@app.command("install-shim")
def install_shim(
    bin_dir: Path = typer.Option(
        Path.home() / ".local" / "bin", help="Directory (early on PATH) for the shim."
    ),
) -> None:
    """Install the `claude` wrapper that enforces the credit meter."""
    cfg, _ = _load()
    try:
        path = shim.install(cfg, bin_dir)
    except FileNotFoundError as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2)
    typer.echo(f"Shim installed: {path}")
    typer.echo("Make sure that directory precedes the real claude on your PATH.")


@app.command("uninstall-shim")
def uninstall_shim(
    bin_dir: Path = typer.Option(Path.home() / ".local" / "bin"),
) -> None:
    """Remove the `claude` wrapper."""
    try:
        removed = shim.uninstall(bin_dir)
    except RuntimeError as e:
        typer.echo(f"error: {e}", err=True)
        raise typer.Exit(2)
    typer.echo("Shim removed." if removed else "No shim found.")


def main() -> None:
    app()
