import json
from pathlib import Path

import pytest

from reps_for_claude.ledger import Ledger


def make(tmp_path: Path, day: str = "2026-07-06", cap: float | None = None) -> Ledger:
    return Ledger(tmp_path / "state", today=lambda: day, rollover_cap=cap)


class TestFreshState:
    def test_missing_file_starts_empty_day(self, tmp_path):
        ledger = make(tmp_path)
        assert ledger.state.date == "2026-07-06"
        assert ledger.state.balance_seconds == 0.0
        assert ledger.state.reps == {}
        assert not ledger.state.report_done

    def test_corrupt_file_recovers(self, tmp_path, capsys):
        state_dir = tmp_path / "state"
        state_dir.mkdir()
        (state_dir / "state.json").write_text("{not json!!")
        ledger = make(tmp_path)
        assert ledger.state.balance_seconds == 0.0
        assert "corrupt state" in capsys.readouterr().err

    def test_missing_keys_recover(self, tmp_path):
        state_dir = tmp_path / "state"
        state_dir.mkdir()
        (state_dir / "state.json").write_text('{"date": "2026-07-06"}')
        ledger = make(tmp_path)
        assert ledger.state.balance_seconds == 0.0


class TestRoundTrip:
    def test_save_and_reload(self, tmp_path):
        ledger = make(tmp_path)
        ledger.add_reps("pushup", 12)
        ledger.set_balance(300.0)
        ledger.spend(50.0)
        ledger.save()

        again = make(tmp_path)
        assert again.state.reps == {"pushup": 12}
        assert again.state.balance_seconds == 250.0
        assert again.state.spent_seconds == 50.0

    def test_atomic_write_leaves_no_temp_files(self, tmp_path):
        ledger = make(tmp_path)
        ledger.save()
        files = list((tmp_path / "state").iterdir())
        assert [f.name for f in files] == ["state.json"]
        json.loads((tmp_path / "state" / "state.json").read_text())  # valid JSON


class TestRollover:
    def test_new_day_resets_reps_and_spend(self, tmp_path):
        old = make(tmp_path, day="2026-07-05")
        old.add_reps("pushup", 30)
        old.set_balance(400.0)
        old.spend(100.0)
        old.mark_report_done()
        old.save()

        new = make(tmp_path, day="2026-07-06")
        assert new.state.date == "2026-07-06"
        assert new.state.reps == {}
        assert new.state.spent_seconds == 0.0
        assert not new.state.report_done
        assert new.state.balance_seconds == 300.0  # carries over

    def test_rollover_clamps_balance_to_cap(self, tmp_path):
        old = make(tmp_path, day="2026-07-05")
        old.set_balance(5000.0)
        old.save()
        new = make(tmp_path, day="2026-07-06", cap=600.0)
        assert new.state.balance_seconds == 600.0


class TestMutations:
    def test_spend_clamps_at_zero(self, tmp_path):
        ledger = make(tmp_path)
        ledger.set_balance(10.0)
        ledger.spend(25.0)
        assert ledger.state.balance_seconds == 0.0
        assert ledger.state.spent_seconds == 10.0  # only what existed

    def test_negative_spend_rejected(self, tmp_path):
        with pytest.raises(ValueError):
            make(tmp_path).spend(-1.0)

    def test_negative_reps_rejected(self, tmp_path):
        with pytest.raises(ValueError):
            make(tmp_path).add_reps("pushup", -3)

    def test_balance_floor_is_zero(self, tmp_path):
        ledger = make(tmp_path)
        ledger.set_balance(-5.0)
        assert ledger.state.balance_seconds == 0.0
