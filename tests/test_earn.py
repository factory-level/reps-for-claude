from reps_for_claude.detector import StubDetector
from reps_for_claude.earn import earn


class TestEarn:
    def test_credits_at_rate(self, cfg, ledger):
        result = earn("pushup", StubDetector(5), ledger, cfg)
        assert result.reps == 5
        assert result.credited_seconds == 300.0  # 5 * 60
        assert result.balance_seconds == 300.0
        assert not result.workout_complete
        assert not result.capped

    def test_cap_limits_precompletion_credit(self, cfg, ledger):
        # 20 reps * 60s = 1200s earned, but cap is 600s and plan incomplete
        # (pushup target met, squat not)
        result = earn("pushup", StubDetector(20), ledger, cfg)
        assert result.balance_seconds == 600.0
        assert result.capped
        assert not result.workout_complete

    def test_completing_plan_lifts_cap_same_session(self, cfg, ledger):
        earn("pushup", StubDetector(10), ledger, cfg)
        # squat session completes the plan, so its credit is uncapped
        result = earn("squat", StubDetector(5), ledger, cfg)
        assert result.workout_complete
        assert not result.capped
        assert result.balance_seconds == 600.0 + 300.0

    def test_zero_reps_changes_nothing(self, cfg, ledger):
        result = earn("pushup", StubDetector(0), ledger, cfg)
        assert result.reps == 0
        assert result.credited_seconds == 0.0
        assert ledger.state.reps == {}

    def test_reps_are_logged(self, cfg, ledger):
        earn("pushup", StubDetector(3), ledger, cfg)
        earn("pushup", StubDetector(4), ledger, cfg)
        assert ledger.state.reps == {"pushup": 7}

    def test_persists_to_disk(self, cfg, ledger, tmp_path):
        earn("pushup", StubDetector(5), ledger, cfg)
        from reps_for_claude.ledger import Ledger

        reloaded = Ledger(tmp_path / "state", today=lambda: "2026-07-06")
        assert reloaded.state.balance_seconds == 300.0
        assert reloaded.state.reps == {"pushup": 5}

    def test_on_rep_callback(self, cfg, ledger):
        seen = []
        earn("pushup", StubDetector(3), ledger, cfg, on_rep=seen.append)
        assert seen == [1, 2, 3]
