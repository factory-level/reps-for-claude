# Configuration

**What this page tells you:** every setting in `config.toml`, what it does, and its default.

The config lives at `~/.config/reps-for-claude/config.toml`. Run `reps init` to write a commented sample. Set the `REPS_HOME` environment variable to relocate config and state together (the test suite does this).

Bad values fail loudly: a zero target, a negative duration, or an unknown exercise name raises a clear `ConfigError` instead of misbehaving later.

## `[session]` — the coding timer

| Key | Default | Meaning |
|---|---|---|
| `work_minutes` | `6` | Minutes of coding before the machine locks. |

## `[goals.weekly]` — your weekly targets

A table of `exercise = reps` entries. These drive what the lock screen prescribes. Every exercise name must be one the detector knows (see `exercises.py`); targets must be positive.

```toml
[goals.weekly]
squat = 60
bench = 40
row = 40
```

## `[break]` — how big each payment is

| Key | Default | Meaning |
|---|---|---|
| `default_reps` | `10` | Reps prescribed per lift set. |
| `jumprope_seconds` | `60` | Seconds of continuous jumping that count as one payment. |
| `stretch_seconds` | `30` | Seconds a stretch must be held. |

## `[detector]` — the camera and the counter

| Key | Default | Meaning |
|---|---|---|
| `name` | `"keyboard"` | `"keyboard"` (press a key per rep) or `"mediapipe"` (webcam pose counting). |
| `camera_index` | `0` | Which `/dev/video*` device to read. |
| `width` | `1280` | Camera frame width. |
| `height` | `720` | Camera frame height. |

## `[display]` — where the scoreboard goes

| Key | Default | Meaning |
|---|---|---|
| `scoreboard_monitor` | `1` | Which monitor (0-indexed) shows the scoreboard (Plan C). |

## `[lock]` — the screen lock

| Key | Default | Meaning |
|---|---|---|
| `enabled` | `false` | Whether to really lock the desktop. |
| `override_password` | `""` | The escape-hatch password. Empty means no override. Every use is logged. |

## `[claude]`

| Key | Default | Meaning |
|---|---|---|
| `real_binary` | `""` | Path to the real `claude` binary; autodetected when empty. |

## Legacy sections

These belong to the old bank-then-spend model and will be retired in Plan B:

| Section | Keys | Meaning |
|---|---|---|
| `[economics]` | `seconds_per_rep`, `precompletion_cap_seconds` | Credit earned per rep, and the balance ceiling before the daily plan is done. |
| `[plan]` | `exercise = reps` entries | The old *daily* rep targets (weekly goals replace these). |
