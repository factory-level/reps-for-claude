# reps-for-claude — Design Spec

**Date:** 2026-07-06
**Status:** Approved (user goal: build with test suite built in)

## Purpose

Gate access to terminal Claude Code (and optionally the whole desktop) behind
completed exercise reps, counted via webcam. Reps bank a time-credit balance;
running Claude Code spends it. A daily workout plan limits spend until
complete, and the day ends with an editable "Form" report for the user's
personal trainer.

## Core mechanics

- **Bank-then-spend.** Reps earn unlock-credit measured in seconds. The
  balance ticks down **only while a `claude` session is actively running**
  (idle time is free). At zero balance, launching Claude is refused; if a
  session is running it is blocked and, when enabled, the desktop locks.
- **Limited spend until workout complete.** A config-defined daily plan sets
  per-exercise rep targets. Until the full plan is met, the banked balance is
  hard-capped at a config ceiling. Completing the plan lifts the cap.
- **Earning is on-demand.** `reps earn <exercise>` opens the webcam, a rep
  counter tallies reps live, and on exit the reps are logged and credited
  (subject to the cap). Camera is only on while earning.
- **Form report.** `reps finish` shows today's counts and spend, lets the
  user edit amounts, writes `logs/YYYY-MM-DD.md` (trainer-readable Markdown)
  plus `logs/YYYY-MM-DD.json` (machine record), and marks the day complete.

## Exercises (initial)

Push-ups, squats, bicep curls, pull-ups, bench press, overhead press, row.
Exercise list is config-driven; detectors are pluggable per exercise.

## Architecture

Single Python package `reps_for_claude`, no daemon. State in JSON files with
atomic writes. CV model deliberately deferred: a `RepCounter` interface with a
deterministic stub ships first so everything is buildable and testable with no
camera.

| Module      | Purpose                                                                 | Depends on |
|-------------|-------------------------------------------------------------------------|------------|
| `config`    | Load/validate `config.toml`: daily plan, economics, caps, enforcement   | stdlib     |
| `ledger`    | Source of truth: balance (seconds), today's rep log, day-complete flag  | `config`   |
| `economics` | Pure functions: reps→credit, cap application, `is_workout_complete`     | —          |
| `detector`  | `RepCounter` interface + `StubDetector`; real CV models slot in later   | —          |
| `earn`      | Earn session: detector → economics → ledger                             | above      |
| `guard`     | `claude` shim logic: gate launch, session timer decrements balance,     | `ledger`,  |
|             | on-zero block + optional lock, persist on exit                          | `lock`     |
| `lock`      | Desktop-lock adapter (`loginctl`/GNOME), no-op fallback                 | —          |
| `report`    | End-of-day Form: review/edit → Markdown + JSON → mark complete          | `ledger`   |
| `cli`       | Command dispatch                                                        | all        |

## CLI surface

- `reps earn <exercise>` — start an earn session
- `reps status` — plan progress, balance, cap state
- `reps balance` — just the balance
- `reps finish` — end-of-day review/edit → Form output
- `reps guard -- <cmd...>` — run a command under the credit meter (what the shim calls)
- `reps install-shim` / `reps uninstall-shim` — manage the `claude` wrapper on PATH

## Enforcement

- **Shim (primary):** installer writes a `claude` script into a PATH dir that
  precedes the real binary; it execs `reps guard -- <real-claude> "$@"`.
  Guard refuses at zero balance and decrements while the child runs.
- **Desktop lock (optional escalation):** same balance; when a running
  session hits zero and `lock.enabled = true`, lock the session via
  `loginctl lock-session` (fallbacks per desktop). No-op where unsupported.

## Locations

- Config: `~/.config/reps-for-claude/config.toml`
- State + logs: `~/.local/state/reps-for-claude/`
- Both overridable via `REPS_HOME` (hermetic tests). Sample config in repo.

## Config sketch

```toml
[economics]
seconds_per_rep = 90          # credit earned per rep (post-completion rate)
precompletion_cap_seconds = 1200  # max banked balance until plan is done

[plan]                        # today's targets
pushup = 30
squat = 40
row = 24

[lock]
enabled = false               # desktop-lock escalation

[claude]
real_binary = ""              # autodetected if empty
```

## Error handling

- Ledger writes are atomic (temp file + rename); corrupt/missing state files
  reset to a safe empty day with a warning, never crash the guard.
- Guard always releases cleanly: on crash/SIGTERM, elapsed time is persisted.
- Lock adapter failures degrade to shim-only enforcement with a warning.
- Detector failures (no camera) abort the earn session with zero credit,
  never partial/phantom reps.

## Testing

- `economics`: pure unit tests (rates, caps, completion predicate).
- `ledger`: temp-dir round-trips, atomic-write and corruption-recovery tests.
- `detector`: stub determinism; interface contract tests future models reuse.
- `guard`: fake clock + fake subprocess — gating, decrement, zero-balance
  stop, persistence on abnormal exit.
- `report`: golden-file Markdown/JSON output; edit flow with injected input.
- `cli`: smoke tests via runner.

## Out of scope (now)

Real CV model selection (MediaPipe vs YOLO-Pose), always-on daemon,
multi-session shared clock supervisor, trainer upload/sync.
