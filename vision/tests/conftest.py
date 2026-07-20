from pathlib import Path

import pytest


@pytest.fixture
def reps_home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point REPS_HOME at a temp dir so model-cache paths are hermetic."""
    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))
    return home
