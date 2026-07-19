from reps_for_claude.goals import Prescription, most_behind, prescribe, progress


class TestProgress:
    def test_fractions(self):
        assert progress({"squat": 60}, {"squat": 30}) == {"squat": 0.5}

    def test_clamped_to_one(self):
        assert progress({"squat": 60}, {"squat": 90}) == {"squat": 1.0}

    def test_missing_is_zero(self):
        assert progress({"squat": 60}, {}) == {"squat": 0.0}

    def test_negative_done_clamps_to_zero(self):
        assert progress({"squat": 60}, {"squat": -30}) == {"squat": 0.0}


class TestMostBehind:
    def test_picks_lowest_fraction(self):
        # squat 50% done, bench 25% done -> bench
        assert most_behind({"squat": 60, "bench": 40}, {"squat": 30, "bench": 10}) == "bench"

    def test_none_when_all_met(self):
        assert most_behind({"squat": 60}, {"squat": 60}) is None

    def test_none_when_empty(self):
        assert most_behind({}, {}) is None

    def test_name_tiebreak(self):
        assert most_behind({"squat": 10, "bench": 10}, {}) == "bench"

    def test_zero_target_never_divides(self):
        # target 0 with negative done must not raise and counts as met
        assert most_behind({"squat": 0}, {"squat": -1}) is None


class TestPrescribe:
    def test_lift_of_most_behind(self):
        p = prescribe({"squat": 60, "bench": 40}, {"squat": 30, "bench": 10}, default_reps=10)
        assert p == Prescription(kind="lift", exercise="bench", target=10)

    def test_none_when_goals_met(self):
        assert prescribe({"squat": 60}, {"squat": 60}, default_reps=10) is None
