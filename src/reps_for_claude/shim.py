"""Install/uninstall the `claude` shim that routes launches through the guard."""

from __future__ import annotations

import os
import stat
from pathlib import Path

from .config import Config

SHIM_MARKER = "# reps-for-claude shim"

SHIM_TEMPLATE = """\
#!/bin/sh
{marker}
exec reps guard -- {real} "$@"
"""


def is_shim(path: Path) -> bool:
    try:
        return SHIM_MARKER in path.read_text()
    except (OSError, UnicodeDecodeError):
        return False


def find_real_claude(config: Config, shim_dir: Path | None = None) -> str | None:
    """Locate the real claude binary, skipping any installed shim."""
    if config.real_claude:
        return config.real_claude
    for entry in os.environ.get("PATH", "").split(os.pathsep):
        if not entry:
            continue
        directory = Path(entry)
        if shim_dir is not None and directory.resolve() == shim_dir.resolve():
            continue
        candidate = directory / "claude"
        if (
            candidate.is_file()
            and os.access(candidate, os.X_OK)
            and not is_shim(candidate)
        ):
            return str(candidate)
    return None


def install(config: Config, bin_dir: Path) -> Path:
    """Write the shim into bin_dir; bin_dir must precede the real claude on PATH."""
    real = find_real_claude(config, shim_dir=bin_dir)
    if real is None:
        raise FileNotFoundError(
            "could not find the real claude binary; set claude.real_binary in config"
        )
    bin_dir.mkdir(parents=True, exist_ok=True)
    shim = bin_dir / "claude"
    shim.write_text(SHIM_TEMPLATE.format(marker=SHIM_MARKER, real=real))
    shim.chmod(shim.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return shim


def uninstall(bin_dir: Path) -> bool:
    """Remove the shim if present. Refuses to delete a non-shim `claude`."""
    shim = bin_dir / "claude"
    if not shim.exists():
        return False
    if not is_shim(shim):
        raise RuntimeError(f"{shim} is not a reps-for-claude shim; refusing to remove")
    shim.unlink()
    return True
