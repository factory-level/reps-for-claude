"""Filesystem locations. REPS_HOME relocates everything (tests use this)."""

from __future__ import annotations

import os
from pathlib import Path


def cache_dir() -> Path:
    home = os.environ.get("REPS_HOME")
    if home:
        return Path(home) / "cache"
    return Path.home() / ".cache" / "reps-for-claude"
