import pytest

from reps_for_claude.economics import (
    apply_earn,
    credit_for_reps,
    is_workout_complete,
)


class TestCreditForReps:
    def test_basic_rate(self):
        assert credit_for_reps(10, 90) == 900.0

    def test_zero_reps(self):
        assert credit_for_reps(0, 90) == 0.0

    def test_negative_rejected(self):
        with pytest.raises(ValueError):
            credit_for_reps(-1, 90)


class TestIsWorkoutComplete:
    def test_complete(self):
        assert is_workout_complete({"pushup": 10}, {"pushup": 10})

    def test_over_target(self):
        assert is_workout_complete({"pushup": 10}, {"pushup": 15})

    def test_incomplete(self):
        assert not is_workout_complete({"pushup": 10}, {"pushup": 9})

    def test_missing_exercise(self):
        assert not is_workout_complete({"pushup": 10, "squat": 5}, {"pushup": 10})

    def test_off_plan_reps_ignored(self):
        assert not is_workout_complete({"pushup": 10}, {"squat": 50})

    def test_empty_plan_is_complete(self):
        assert is_workout_complete({}, {})


class TestApplyEarn:
    def test_uncapped_when_complete(self):
        assert apply_earn(500.0, 900.0, complete=True, cap=600.0) == 1400.0

    def test_capped_when_incomplete(self):
        assert apply_earn(500.0, 900.0, complete=False, cap=600.0) == 600.0

    def test_under_cap_earns_fully(self):
        assert apply_earn(100.0, 200.0, complete=False, cap=600.0) == 300.0

    def test_cap_never_confiscates_existing_balance(self):
        # e.g. balance carried above cap — earning must not reduce it
        assert apply_earn(800.0, 100.0, complete=False, cap=600.0) == 800.0
