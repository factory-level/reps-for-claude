# reps-for-claude

Earn Claude Code time by doing exercise reps. No reps, no Claude.

Reps bank a time-credit balance (**bank-then-spend**). The balance ticks down
only while a `claude` session is actually running. Until your daily workout
plan is complete, the balance is hard-capped; finish the plan and credit is
uncapped. At day's end, `reps finish` produces an editable **Form** report
(Markdown + JSON) for your personal trainer.

## Install

```sh
uv sync
uv run reps init          # writes ~/.config/reps-for-claude/config.toml
uv run reps install-shim  # wraps `claude` with the credit meter
```

The shim goes in `~/.local/bin` (configurable) — make sure it precedes the
real `claude` on your `PATH`.

## Use

```sh
reps earn pushup                       # count reps (keyboard or webcam detector)
reps analyze workout.mp4 -e pushup     # count reps in a video file (never credits)
reps status                            # plan progress, balance, cap state
reps balance
reps finish                            # review/edit counts → Form report for your trainer
```

For webcam pose counting, install the CV extra and switch the detector:

```sh
uv sync --extra cv         # mediapipe + opencv; pose model downloads once (~5MB)
# config.toml: [detector] name = "mediapipe"
```

With the shim installed, `claude` launches only when you have credit, meters
your balance while it runs, and stops the session (optionally locking the
desktop, see `lock.enabled`) when it hits zero.

## Configuration

`~/.config/reps-for-claude/config.toml` (see `reps init` for a sample):

- `economics.seconds_per_rep` — credit per rep
- `economics.precompletion_cap_seconds` — balance ceiling until the plan is done
- `[plan]` — per-exercise daily rep targets
- `detector.name` — `keyboard` today; CV detectors plug in behind the same interface
- `lock.enabled` — lock the desktop when credit runs out mid-session
- `claude.real_binary` — path to the real claude (autodetected if empty)

Set `REPS_HOME` to relocate config + state (used by the test suite).

## Development

```sh
uv run pytest                                # unit suite (no camera, no model)
uv run python scripts/fetch_fixtures.py      # download CC-licensed workout clips
uv run pytest -m cvvideo                     # real pose estimation over the clips
```

State lives in `~/.local/state/reps-for-claude/` — a single JSON ledger with
atomic writes; corrupt state recovers to a fresh day rather than crashing the
guard. Design spec: `docs/superpowers/specs/2026-07-06-reps-for-claude-design.md`.

## Roadmap

- Live-camera validation once a webcam is plugged in (the detector already
  takes a camera index — only the hardware is missing)
- Threshold tuning per exercise from real footage of *your* form
- Multi-session shared clock supervisor
