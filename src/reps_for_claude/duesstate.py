"""Whether the machine currently owes a workout.

Behind a Protocol so a privileged root authority can replace the file store in
Phase 2. A missing or unreadable file means no dues are owed (fail-open: never
strand the user because of a bad file).
"""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path
from typing import Protocol


class DuesState(Protocol):
    def owed(self) -> bool: ...
    def set_owed(self, owed: bool) -> None: ...


class FileDuesState:
    def __init__(self, state_dir: Path) -> None:
        self._dir = Path(state_dir)
        self._path = self._dir / "dues.json"

    def owed(self) -> bool:
        try:
            return bool(json.loads(self._path.read_text())["owed"])
        except (OSError, json.JSONDecodeError, KeyError, TypeError, ValueError):
            return False

    def set_owed(self, owed: bool) -> None:
        self._dir.mkdir(parents=True, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=self._dir, prefix=".dues-")
        try:
            with os.fdopen(fd, "w") as f:
                json.dump({"owed": bool(owed)}, f)
            os.replace(tmp, self._path)
        except BaseException:
            os.unlink(tmp)
            raise
