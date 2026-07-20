# CLI Commands

**What this page tells you:** every `reps` command that exists today, and which ones are on their way out.

Run everything through `uv run reps <command>` (or plain `reps` if installed on your PATH).

## Current commands

### `reps init`

Writes a commented sample `config.toml` to `~/.config/reps-for-claude/` if none exists. Start here.

### `reps analyze VIDEO -e EXERCISE`

Runs real pose detection over a video file and reports the rep count. **Never credits anything** — it is a debugging and tuning tool, so you can check what the detector sees before trusting it with your lock screen.

| Flag | Meaning |
|---|---|
| `--show` | Open a live window with the skeleton drawn on each frame (press `q` to quit). |
| `-o out.mp4` | Write an annotated copy of the video. |

A sample annotated clip lives in `docs/demo/`.

## Legacy commands (old model)

These four implement the old *bank-then-spend* design, where reps earned "Claude time" like money in an account. They still work, but Plan B replaces this whole surface with a single `reps session` supervisor. Expect them to be retired.

| Command | What it does |
|---|---|
| `reps earn EXERCISE` | Count reps (keyboard or webcam) and bank credit. |
| `reps status` | Show plan progress, balance, and cap state. |
| `reps balance` | Show the credit balance. |
| `reps finish` | Review today's counts and write the end-of-day Form report for your trainer. |

## Coming in Plan B

### `reps session`

The one command you will actually live in: it runs the coding timer, locks the machine when time is up, drives the workout screen, and unlocks when your dues are paid. See [The loop](../concepts/the-loop.md).

> Note: `reps guard`, `reps install-shim`, and `reps uninstall-shim` used to exist. They wrapped the `claude` binary with a credit meter. They were removed — the whole-screen lock made them pointless, since the lock stops *everything*, not just Claude.
