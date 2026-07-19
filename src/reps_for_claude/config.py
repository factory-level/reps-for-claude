"""Load and validate configuration for reps-for-claude.

Config lives at ~/.config/reps-for-claude/config.toml and state at
~/.local/state/reps-for-claude/, unless REPS_HOME is set, in which case both
live under that directory (config.toml + state/). This keeps tests hermetic.
"""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_SECONDS_PER_REP = 90
DEFAULT_PRECOMPLETION_CAP = 1200

SAMPLE_CONFIG = """\
[economics]
seconds_per_rep = 90              # credit earned per rep
precompletion_cap_seconds = 1200  # max banked balance until today's plan is done

[plan]                            # today's per-exercise rep targets
pushup = 30
squat = 40
row = 24

[session]
work_minutes = 6                  # coding minutes before the machine locks

[goals.weekly]                    # weekly rep targets that drive lock prescriptions
squat = 60
bench = 40
row = 40

[break]
default_reps = 10                 # reps prescribed per lift set
jumprope_seconds = 60             # jump-rope payment target
stretch_seconds = 30              # stretch hold target

[detector]
name = "keyboard"                 # "keyboard" or "mediapipe" (webcam pose counting)
camera_index = 0                  # which /dev/video* to use for mediapipe
width = 1280                      # camera frame width
height = 720                       # camera frame height

[display]
scoreboard_monitor = 1            # which monitor (0-indexed) shows scoreboard

[lock]
enabled = false                   # lock the desktop when balance hits zero mid-session
override_password = ""            # password to unlock; empty means no override

[claude]
real_binary = ""                  # path to the real claude; autodetected if empty
"""


class ConfigError(Exception):
    """Raised when config.toml is invalid."""


def config_dir() -> Path:
    home = os.environ.get("REPS_HOME")
    if home:
        return Path(home)
    return Path.home() / ".config" / "reps-for-claude"


def state_dir() -> Path:
    home = os.environ.get("REPS_HOME")
    if home:
        return Path(home) / "state"
    return Path.home() / ".local" / "state" / "reps-for-claude"


def cache_dir() -> Path:
    home = os.environ.get("REPS_HOME")
    if home:
        return Path(home) / "cache"
    return Path.home() / ".cache" / "reps-for-claude"


@dataclass
class Config:
    seconds_per_rep: int = DEFAULT_SECONDS_PER_REP
    precompletion_cap_seconds: int = DEFAULT_PRECOMPLETION_CAP
    plan: dict[str, int] = field(default_factory=dict)
    detector: str = "keyboard"
    camera_index: int = 0
    lock_enabled: bool = False
    real_claude: str = ""
    work_minutes: int = 6
    weekly_goals: dict[str, int] = field(default_factory=dict)
    default_reps: int = 10
    jumprope_seconds: int = 60
    stretch_seconds: int = 30
    cam_width: int = 1280
    cam_height: int = 720
    scoreboard_monitor: int = 1
    override_password: str = ""


def _require_positive_int(value: object, name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ConfigError(f"{name} must be a positive integer, got {value!r}")
    return value


def load(path: Path | None = None) -> Config:
    """Load config from path (default: config_dir()/config.toml).

    A missing file yields default Config; a malformed one raises ConfigError.
    """
    if path is None:
        path = config_dir() / "config.toml"
    if not path.exists():
        return Config()
    try:
        raw = tomllib.loads(path.read_text())
    except tomllib.TOMLDecodeError as e:
        raise ConfigError(f"could not parse {path}: {e}") from e

    cfg = Config()
    econ = raw.get("economics", {})
    if "seconds_per_rep" in econ:
        cfg.seconds_per_rep = _require_positive_int(
            econ["seconds_per_rep"], "economics.seconds_per_rep"
        )
    if "precompletion_cap_seconds" in econ:
        cfg.precompletion_cap_seconds = _require_positive_int(
            econ["precompletion_cap_seconds"], "economics.precompletion_cap_seconds"
        )

    plan = raw.get("plan", {})
    if not isinstance(plan, dict):
        raise ConfigError("plan must be a table of exercise = target entries")
    cfg.plan = {
        name: _require_positive_int(target, f"plan.{name}")
        for name, target in plan.items()
    }

    detector = raw.get("detector", {})
    if "name" in detector:
        if not isinstance(detector["name"], str):
            raise ConfigError("detector.name must be a string")
        cfg.detector = detector["name"]
    if "camera_index" in detector:
        idx = detector["camera_index"]
        if not isinstance(idx, int) or isinstance(idx, bool) or idx < 0:
            raise ConfigError(
                f"detector.camera_index must be a non-negative integer, got {idx!r}"
            )
        cfg.camera_index = idx

    lock = raw.get("lock", {})
    if "enabled" in lock:
        if not isinstance(lock["enabled"], bool):
            raise ConfigError("lock.enabled must be a boolean")
        cfg.lock_enabled = lock["enabled"]

    claude = raw.get("claude", {})
    if "real_binary" in claude:
        if not isinstance(claude["real_binary"], str):
            raise ConfigError("claude.real_binary must be a string")
        cfg.real_claude = claude["real_binary"]

    session = raw.get("session", {})
    if "work_minutes" in session:
        cfg.work_minutes = _require_positive_int(
            session["work_minutes"], "session.work_minutes"
        )

    goals = raw.get("goals", {})
    if not isinstance(goals, dict):
        raise ConfigError("goals must be a table containing a [goals.weekly] table")
    weekly = goals.get("weekly", {})
    if not isinstance(weekly, dict):
        raise ConfigError("goals.weekly must be a table of exercise = target entries")
    cfg.weekly_goals = {
        name: _require_positive_int(target, f"goals.weekly.{name}")
        for name, target in weekly.items()
    }

    brk = raw.get("break", {})
    for key, attr in (
        ("default_reps", "default_reps"),
        ("jumprope_seconds", "jumprope_seconds"),
        ("stretch_seconds", "stretch_seconds"),
    ):
        if key in brk:
            setattr(cfg, attr, _require_positive_int(brk[key], f"break.{key}"))

    for key, attr in (("width", "cam_width"), ("height", "cam_height")):
        if key in detector:
            setattr(cfg, attr, _require_positive_int(detector[key], f"detector.{key}"))

    if "scoreboard_monitor" in raw.get("display", {}):
        idx = raw["display"]["scoreboard_monitor"]
        if not isinstance(idx, int) or isinstance(idx, bool) or idx < 0:
            raise ConfigError(
                f"display.scoreboard_monitor must be a non-negative integer, got {idx!r}"
            )
        cfg.scoreboard_monitor = idx

    if "override_password" in lock:
        if not isinstance(lock["override_password"], str):
            raise ConfigError("lock.override_password must be a string")
        cfg.override_password = lock["override_password"]

    return cfg


def write_sample(path: Path | None = None) -> Path:
    """Write the sample config if none exists; return its path."""
    if path is None:
        path = config_dir() / "config.toml"
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_text(SAMPLE_CONFIG)
    return path
