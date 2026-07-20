# Tauri Rewrite — Design Spec

**Date:** 2026-07-19
**Status:** Approved in brainstorming; awaiting written-spec review
**Source of truth:** the V1 Technical Design Specification ("Reps for Claude", Tauri + React + TypeScript)
**Supersedes:** `2026-07-19-workout-lock-display-design.md` (Python supervisor + web UI) and the weekly-goals prescription model of Plan A.

## Decisions (locked in with the user)

1. **Source of truth:** the V1 Tauri spec. Desktop app, four screens, SQLite, themes/widgets.
2. **Vision Engine:** Python sidecar reusing the proven detection stack (MediaPipe pose, angle state machine, streaming activities). No in-webview camera (WebKitGTK getUserMedia is unreliable on Linux).
3. **Repo:** monorepo split in place — `app/` (Tauri + React + TS) and `vision/` (trimmed Python sidecar). All other Python (cli, ledger, economics, earn, goals, weekly, duesstate, lock, report) is deleted and reimplemented per this spec.
4. **Enforcement:** real OS lock via `xsecurelock`; degrades to a fullscreen always-on-top window if missing; developer mode never locks.
5. **Programming model:** workout **rotation + daily capacity** (V1 spec §6–7). The Plan A weekly-goals/most-behind model is retired.
6. **Core home:** Rust authority + TS views. Engines, timers, storage writes, and dangerous lifecycles live in Rust; React windows are pure, theme-able views.
7. **Target:** exactly one machine — Linux Mint, X11/Cinnamon, single seat, Logitech C920x, `google-chrome` present. No cross-platform work.

## Monorepo layout

```
reps-for-claude/
├── app/
│   ├── src/          # React + TS (Vite): routed views /workstation /tv /operator /metrics /lock
│   └── src-tauri/    # Rust core: engines, SQLite, sidecar driver, lock driver, HTTP/WS server
├── vision/           # Python sidecar (uv project): pose, angles, exercises, activities,
│                     # video, visualize + their pytest suite; JSON-lines CLI entry
├── _docs/            # the wiki (updated as milestones land)
└── docs/superpowers/ # specs + plans (history preserved)
```

## Process model

Three processes:

1. **Rust core — the authority.** Owns:
   - The application state machine (V1 spec §20): `CODING → EXERCISE_REQUIRED → WORKOUT_ACTIVE → WEIGHT_CONFIRMATION → UNLOCKED → CODING`.
   - **Timer Engine:** coding duration, unlock reward, exercises per break, break frequency (all configurable).
   - **Workout Engine:** rotation pointer (persistent, never resets), per-exercise defaults, daily weighted-set capacity; when capacity is spent, prescriptions switch to the continuous pool (jump rope / stretch / mobility) for the rest of the day.
   - **Storage:** SQLite via `rusqlite` with migrations. Tables: settings, rotation, exercise_history (exercise, weight, sets, reps, date, verified), theme_config, session_log (coding time earned, overrides used). Only the core writes.
   - **Sidecar driver** and **lock driver** with drop-safe lifecycles.
   - **Localhost server** (`axum`, bound 127.0.0.1): serves the built frontend and a WebSocket streaming state snapshots. Consumers: the xsecurelock saver (Chrome kiosk) and OBS browser-sources for streaming.
2. **React windows — pure views.** One React app, four routes opened as Tauri windows: Workstation (main), Gym TV (fullscreen on `scoreboard_monitor`), Operator, Metrics; plus `/lock` served over localhost. Views render the latest snapshot and send intents as Tauri commands (start session, choose exercise, confirm weight, override, settings changes). No view holds state authority.
3. **Python vision sidecar.** Spawned when a workout starts, killed when it ends — the camera is on only while locked (privacy rule). Ports `pose.py`, `angles.py`, `exercises.py`, `activities/` (lift, jump rope with 2s grace period, stretch), `video.py`, `visualize.py` and their tests essentially unchanged.

**State flow:** every change in the core emits one JSON snapshot (state, timers, prescription, live progress, rotation position, capacity, today totals). Tauri windows receive it via events; the WebSocket mirrors it. UI is a pure function of the snapshot.

## Sidecar protocol (JSON lines over stdin/stdout)

Core → sidecar:
```json
{"cmd": "start", "activity": "lift", "exercise": "squat", "target": 10, "camera": {"index": 0, "width": 1280, "height": 720}}
{"cmd": "stop"}
{"cmd": "simulate", "event": "rep"}
```

Sidecar → core (at frame rate):
```json
{"event": "landmarks", "points": {"left_knee": [0.5, 0.6, 0.98]}, "confidence": 0.97}
{"event": "progress", "value": 3, "unit": "reps", "satisfied": false}
{"event": "error", "kind": "camera_open_failed", "detail": "/dev/video0"}
```

Golden-transcript tests replay recorded lines against both sides so the two languages cannot drift.

## Screens (V1 spec §5)

| Screen | Window | Contents |
|---|---|---|
| Workstation | normal | Timer widget with the spec's three states (Coding Active / Exercise Required / Workout Complete), next exercise, session progress, unlock reward, settings, dev tools |
| Gym TV | fullscreen on configured monitor | Game-HUD: current exercise, big rep counter, progress animations, timers, theme visuals. No debug info. Same content serves as the lock screen. |
| Operator | normal | Live webcam feed + landmark overlay, confidence values, calibration controls, simulate buttons, raw state dump |
| Metrics | normal | Weekly volume, exercise history, total reps, sets, weight progression, coding time earned, workout frequency — read-only SQLite queries |

## Themes and widgets (V1 spec §13–14, kept V1-sane)

Views compose from a **widget registry**: coding timer, current exercise card, rep progress, weekly volume, streak, time remaining, workout completion. A **theme** is a JSON file of design tokens (colors, fonts, backgrounds, GIF/animation refs) plus per-view widget layout; tokens land as CSS variables. Swapping themes never touches logic. V1 ships two built-in themes. Theme editor and marketplace: out of scope.

## Weight logging (V1 spec §12)

On a satisfied rep-based set the core enters `WEIGHT_CONFIRMATION`; the UI shows a stepper pre-filled from the last session for that exercise (falling back to rotation defaults). Confirming writes exercise/weight/sets/reps/date, advances the rotation pointer, and increments the daily capacity counter. Continuous activities skip this state.

## Lock (enforcement)

- `xsecurelock` with a custom saver script that launches `google-chrome --kiosk http://127.0.0.1:<port>/lock`, and a custom auth module that unlocks when the core reports dues paid or the override password matches.
- Missing `xsecurelock` → warn once, degrade to a fullscreen always-on-top Tauri window (friction, not security).
- The lock's goal is friction, not un-bypassability: a reboot always wins and that is accepted.
- Override password: usable in every locked state, always logged as a skipped workout, shown in Metrics.
- Developer mode: never locks, ever (V1 spec §15).

## Error handling

| Failure | Behavior |
|---|---|
| Core crash/panic while locked | Drop guards release `xsecurelock` and kill the sidecar. Never strand the user. |
| `xsecurelock` missing | One clear warning; fullscreen-window fallback. |
| Camera absent or dies mid-set | Honor-mode "press Done" fallback, logged as `verified = false`. Never phantom reps, never a dead end. |
| Sidecar crash | One auto-restart, then honor-mode fallback. |
| SQLite corrupt | Back up the bad file, recreate fresh, warn loudly. |
| Invalid settings | Clear startup error; refuse to guess. |
| Webcam device changed | Settings → webcam picker re-enumerates (V1 spec §17); preferred device remembered. |

## Testing (V1 spec §21)

- **Rust engines:** plain unit tests — fake clock, fake locker, fake sidecar. The state machine is Tauri-free code.
- **Sidecar:** the existing pytest suite moves with it (synthetic landmarks, hermetic tmp dirs, `cvvideo` marker for real footage).
- **Protocol:** golden JSON-lines transcripts replayed against both implementations.
- **React:** vitest + Testing Library; widgets are pure functions of canned snapshots.
- **End to end:** one manual script per milestone, run on the actual target machine.
- Developer mode makes every state reachable without exercising or locking.

## Milestones (each gets its own implementation plan)

1. **Foundation** — monorepo split (create `app/`, `vision/`; delete legacy Python), Tauri scaffold, SQLite schema + migrations, state machine + Timer/Workout engines headless and fully tested, minimal Workstation view rendering real snapshots.
2. **Vision** — sidecar JSON-lines entry point over the ported detection stack, sidecar driver in Rust, Operator view (feed, landmarks, confidence, simulate).
3. **The loop closes** — xsecurelock driver + saver/auth scripts, localhost HTTP/WS server, `/lock` view, weight confirmation. The product is usable daily.
4. **Gym TV + themes** — HUD view, widget registry, two themes, OBS-ready output.
5. **Metrics + polish** — Metrics view, settings UI (webcam picker, rotation editor, capacity, timers, theme select), dev-mode ergonomics.

## Out of scope (V1, per spec §3 plus decisions)

Cloud sync, accounts, social, leaderboards, mobile, wearables, AI coaching, automated programming, theme marketplace/editor; weekly-goal prescription mode; Wayland or any non-this-machine platform; boot-time re-lock (reboot is an accepted escape); true rope-swing counting (bounce-gated timer is the honest V1); posture-checked stretches.
