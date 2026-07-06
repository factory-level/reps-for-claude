from pathlib import Path

import pytest

from reps_for_claude.config import Config
from reps_for_claude.ledger import Ledger


@pytest.fixture
def cfg() -> Config:
    return Config(
        seconds_per_rep=60,
        precompletion_cap_seconds=600,
        plan={"pushup": 10, "squat": 5},
    )


@pytest.fixture
def ledger(tmp_path: Path) -> Ledger:
    return Ledger(tmp_path / "state", today=lambda: "2026-07-06")


@pytest.fixture
def reps_home(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point REPS_HOME at a temp dir so CLI tests are hermetic."""
    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))
    return home
