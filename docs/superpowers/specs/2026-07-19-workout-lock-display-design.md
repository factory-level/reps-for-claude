# Workout-Lock Display — Design Spec (Phase 1)

**Date:** 2026-07-19
**Status:** Draft for user review
**Supersedes economics of:** `2026-07-06-reps-for-claude-design.md` (bank-then-spend
balance/cap is replaced by a lock-until-workout model).

## Purpose

Turn reps-for-claude into a **fitness-pomodoro that kicks you off the computer.**
You code while an hourglass (⌛) coding timer runs on a second HDMI screen that
also acts as a **scoreboard**. When the timer expires the **whole machine locks**
(real X11 lock). The lock screen shows a GSAP Claude mascot and prescribes a
lift — driven by your **weekly goals** — or a jump-rope interval. You do the set,
the C920x webcam counts your reps, you **log the weight (lbs)**, and completing
the payment **unlocks the machine** and returns to the ⌛ scoreboard. Loop, 6
days/week.

The goal of the lock is **not** cryptographic un-bypassability (you own root; a
reboot or live-USB always wins, and the user has explicitly accepted reboot as an
escape). The goal is that a casual, in-the-moment bypass costs more effort than
just doing the workout.

## Scope

**Phase 1 (this spec):** the full loop working end to end — ⌛ timer → real lock
→ mascot workout screen → rep detection (C920x) + lbs logging → unlock →
scoreboard. Dues-state lives behind an interface so Phase 2 can swap it for a
privileged authority without rewrites.

**Phase 2 (future, not this spec):** root systemd enforcer owning dues-state in a
root-owned file (so you can't edit your way out), a deliberately-expensive
override (forced delay + long passphrase, logged as a skipped workout), and
harder anti-kill/anti-VT plumbing. Explicitly **not** boot-time re-lock — reboot
is an accepted escape.

## Platform (verified)

X11 / Cinnamon (Linux Mint) / systemd, `DISPLAY=:0`, single seat. `google-chrome`
present (kiosk saver). `xsecurelock` **not yet installed** → `apt install
xsecurelock` is a setup prerequisite. Webcam: Logitech C920x (UVC, `/dev/video0`,
read at 1280×720 via OpenCV/V4L2).

## The loop (state machine)

- **CODING** — no lock. HDMI screen shows the **scoreboard**: ⌛ countdown,
  weekly-goal progress, today's totals (reps, lbs, jump-rope time), streak.
  Camera **off**. Mascot idles/"codes." Timer hits 0 → LOCKED.
- **LOCKED** — `xsecurelock` locks all monitors (input grabbed, VT-switch
  disabled). Saver renders the **workout view**: the prescribed lift (or jump
  rope), the GSAP mascot doing it, and — camera **on** — a live rep count / timer
  from the C920x. When the rep target is met, prompt for **lbs lifted**; on submit
  the payment is satisfied → PAID. (Jump rope: hit the time target → PAID. Stretch:
  hold the timer → PAID.)
- **PAID** — brief celebration, camera off, `xsecurelock` released, dues marked
  paid + logged, weekly progress updated, ⌛ timer reset → CODING.

Escape hatch in every state: an **override** (Phase 1: password; Phase 2:
expensive delay + passphrase), always logged.

## Architecture

One user-level supervisor process; no privileged component in Phase 1.

- **`reps session`** (supervisor, foreground) — you launch it when you sit down to
  code (later: autostart on login). Runs the state machine + ⌛ timer, serves the
  local web app, owns the webcam during LOCKED, drives `xsecurelock`, and streams
  state to the browser. It does **not** wrap or pause Claude — the OS-level lock
  handles enforcement, so coding in any app is blocked while locked.
- **Browser (Chrome kiosk)** — renders two views from the same backend:
  `/scoreboard` (HDMI screen during CODING) and `/lock` (the `xsecurelock` saver
  during LOCKED). Display-only; authority stays in the supervisor.

Data flow during a break:
`C920x frame → MediaPipe (pose.py) → landmarks → Activity.update() →
{progress, satisfied} → WebSocket → mascot + counter`. Camera only on during
LOCKED (privacy).

## Components

New unless noted. Existing detection stack is reused as-is.

| Module | Purpose | Notes |
|--------|---------|-------|
| `session.py` | Supervisor / state machine (CODING↔LOCKED↔PAID) | fake-clock testable, like today's `guard` |
| `locker.py` | Drive `xsecurelock`: launch with our saver (`/lock` view) + our auth (unlock when dues paid / override); detect release | extends today's `lock.py` command-runner; degrades to a warning if `xsecurelock` missing |
| `duesstate.py` | `DuesState` interface: dues owed?, mark paid, weekly progress | Phase-1 impl = user file under `REPS_HOME`; Phase-2 = root service |
| `goals.py` | Weekly goal model, progress, "what to prescribe next" | pure functions |
| `server.py` | localhost-only HTTP + WebSocket; serve static frontend + stream state JSON | bound to 127.0.0.1 |
| `web/` | `scoreboard.html`, `lock.html`, `app.js` (WS client + GSAP mascot), `styles.css`, mascot SVG | GSAP vendored locally (Artifact CSP-style: self-contained) |
| `activities/` | `BreakActivity` streaming-progress interface + `LiftActivity`, `JumpRopeActivity`, `StretchActivity` | see Detection |
| `config.py` (changed) | add `[session]`, `[goals.weekly]`, `[display]`, extend `[lock]` | see Config |
| `ledger.py` / `report.py` (changed) | log reps **+ lbs** + sessions; feed weekly progress + trainer report | keep the Form report |
| `economics.py` (trimmed) | drop balance/cap; keep any pure helpers goals need | |
| `guard.py`, `shim.py` (removed) | superseded by `session` + real lock | remove or archive |

Reused unchanged: `pose.py`, `angles.py`, `exercises.py`, `visualize.py`,
`detector.py` (for lift counting).

## Detection (activities)

Today's `RepCounter` returns only a final total. The lock screen needs **live
streaming progress**, so introduce a small `BreakActivity` interface:
`update(landmarks) -> Progress{value, unit, satisfied}` fed one pose frame at a
time, plus a `target`.

- **LiftActivity** — reuse `exercises.py` + `RepStateMachine`. Counts reps; target
  = prescribed reps. On completion, the UI collects **lbs** (stepper in `/lock`).
  Solid, reuses proven code.
- **JumpRopeActivity** — motion-gated continuous timer: sustained vertical bounce
  (hip/ankle y-oscillation over a threshold) keeps the clock running; a stop of
  >~2 s resets the streak. Target = seconds in a row. **Honest limitation:** this
  is bounce-detection, not true rope-swing rep counting — good enough to gate
  unlock, tuned against real C920x footage.
- **StretchActivity** — honor-start timed hold (target = seconds). Auto-posture
  detection is out of scope for Phase 1.

## Weekly goals

`config` defines a weekly plan (per-exercise weekly rep/set targets, e.g.
`squat = 60`, `bench = 40`). `goals.py` tracks cumulative weekly progress (from the
ledger) and, at each lock, **prescribes the next payment**: one set of whichever
goal is most behind, or lets you pick, with jump rope always available as an
alternative. The scoreboard shows progress bars toward each weekly target and
resets weekly. Logged lbs accumulate as weekly **volume** for the trainer report.

## Config sketch

```toml
[session]
work_minutes = 6            # ⌛ coding period before a lock

[goals.weekly]              # weekly rep targets that drive prescriptions
squat = 60
bench = 40
row = 40

[break]
default_reps = 10          # prescribed reps per lift set
jumprope_seconds = 60      # jump-rope payment target
stretch_seconds = 30       # stretch hold target

[detector]
name = "mediapipe"
camera_index = 0           # C920x at /dev/video0
width = 1280
height = 720

[display]
scoreboard_monitor = 1     # which X monitor is the HDMI scoreboard

[lock]
enabled = true
xsecurelock = ""           # autodetected if empty
override_password = ""     # Phase 1 escape hatch (hashed at rest)

[claude]
real_binary = ""
```

## Error handling / degradation

- **No `xsecurelock`** → `locker` prints a clear "install xsecurelock" warning and
  degrades to a best-effort fullscreen Chrome kiosk window (friction-only); never
  crashes the loop.
- **No / failed camera (C920x unplugged)** → the break falls back to honor-system
  "press Done" (or the keyboard counter); never phantom reps.
- **Browser / second screen absent** → fall back to a terminal readout of state.
- **Supervisor crash while LOCKED** → `try/finally` always releases the lock so you
  are never stranded; on restart it reads persisted state.
- **State file corrupt/missing** → safe reset with a warning (existing pattern).
- Web server bound to `127.0.0.1` only.

## Testing

- `session` state machine — fake clock + fake locker + fake activity: assert
  CODING→LOCKED→PAID transitions, lock invoked, release-on-crash.
- `activities` — synthetic landmark/clock sequences: LiftActivity rep counting,
  JumpRopeActivity streak + reset on stop, StretchActivity hold. Pure, hermetic.
- `goals` — pure unit tests: progress accumulation, prescription ("most behind"),
  weekly reset.
- `duesstate` — temp-dir round-trips under `REPS_HOME`; mark-paid/owed.
- `locker` — inject a fake `xsecurelock` runner: assert invocation + unlock
  detection + missing-binary degradation.
- `server` — state-serialization unit test + one fake-WebSocket-client test.
- `report` — golden-file Markdown/JSON including lbs + weekly volume.
- Frontend — smoke test only: page loads and applies a pushed state message.
- Keep the `REPS_HOME` hermetic pattern throughout.

## Out of scope (Phase 1)

Phase 2 hardening (root enforcer, expensive override, anti-kill/anti-VT); boot-time
re-lock (reboot is an accepted escape); true rope-swing rep counting; auto-posture
stretch detection; auto-detecting which monitor is HDMI (set in config);
cross-OS/Wayland support; activity-gated timer (Phase 1 counts wall-clock while the
session runs).
