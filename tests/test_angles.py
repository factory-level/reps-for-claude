import pytest

from reps_for_claude.angles import RepStateMachine, angle


class TestAngle:
    def test_right_angle(self):
        assert angle((0, 1), (0, 0), (1, 0)) == pytest.approx(90.0)

    def test_straight_line(self):
        assert angle((-1, 0), (0, 0), (1, 0)) == pytest.approx(180.0)

    def test_acute(self):
        assert angle((1, 1), (0, 0), (1, 0)) == pytest.approx(45.0)

    def test_zero_angle_collinear_same_side(self):
        assert angle((1, 0), (0, 0), (2, 0)) == pytest.approx(0.0)

    def test_order_of_outer_points_is_symmetric(self):
        assert angle((0, 1), (0, 0), (1, 0)) == pytest.approx(
            angle((1, 0), (0, 0), (0, 1))
        )

    def test_degenerate_points_raise(self):
        with pytest.raises(ValueError):
            angle((0, 0), (0, 0), (1, 0))


class TestRepStateMachine:
    def make(self):
        return RepStateMachine(down_below=90.0, up_above=160.0)

    def feed(self, machine, samples):
        return [machine.update(s) for s in samples]

    def test_full_rep_counts_once(self):
        machine = self.make()
        results = self.feed(machine, [170, 120, 80, 120, 170])
        assert results == [False, False, False, False, True]

    def test_partial_rep_does_not_count(self):
        machine = self.make()
        # never gets below 90
        assert not any(self.feed(machine, [170, 120, 100, 120, 170]))

    def test_jitter_between_thresholds_cannot_double_count(self):
        machine = self.make()
        results = self.feed(machine, [170, 80, 165, 150, 155, 150, 165])
        assert sum(results) == 1

    def test_multiple_reps(self):
        machine = self.make()
        one_rep = [80, 170]
        results = self.feed(machine, one_rep * 5)
        assert sum(results) == 5

    def test_starting_at_bottom_counts_on_first_extension(self):
        # e.g. overhead press racked at the bottom position
        machine = self.make()
        assert self.feed(machine, [70, 170]) == [False, True]

    def test_invalid_thresholds_rejected(self):
        with pytest.raises(ValueError):
            RepStateMachine(down_below=160.0, up_above=90.0)
