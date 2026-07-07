import pytest

from reps_for_claude.exercises import SPECS, ExerciseSpec, get_spec

ALL_EXERCISES = ["pushup", "squat", "curl", "pullup", "bench", "overhead", "row"]
VALID_JOINTS = {"shoulder", "elbow", "wrist", "hip", "knee", "ankle"}


class TestSpecs:
    def test_all_seven_exercises_present(self):
        assert sorted(SPECS) == sorted(ALL_EXERCISES)

    @pytest.mark.parametrize("name", ALL_EXERCISES)
    def test_thresholds_sane(self, name):
        spec = SPECS[name]
        assert 0 < spec.down_below < spec.up_above < 180

    @pytest.mark.parametrize("name", ALL_EXERCISES)
    def test_joints_valid(self, name):
        assert set(SPECS[name].joints) <= VALID_JOINTS

    def test_get_spec_unknown(self):
        with pytest.raises(KeyError, match="unknown exercise"):
            get_spec("juggling")


def lm(x, y, vis=0.9):
    return (x, y, vis)


class TestAngleFrom:
    spec = ExerciseSpec("test", ("shoulder", "elbow", "wrist"), 90.0, 160.0)

    def test_reads_visible_side(self):
        landmarks = {
            "left_shoulder": lm(0, 1),
            "left_elbow": lm(0, 0),
            "left_wrist": lm(1, 0),
        }
        assert self.spec.angle_from(landmarks) == pytest.approx(90.0)

    def test_prefers_more_visible_side(self):
        landmarks = {
            # left side: straight arm, barely visible
            "left_shoulder": lm(-1, 0, 0.55),
            "left_elbow": lm(0, 0, 0.55),
            "left_wrist": lm(1, 0, 0.55),
            # right side: right angle, clearly visible
            "right_shoulder": lm(0, 1, 0.95),
            "right_elbow": lm(0, 0, 0.95),
            "right_wrist": lm(1, 0, 0.95),
        }
        assert self.spec.angle_from(landmarks) == pytest.approx(90.0)

    def test_low_visibility_returns_none(self):
        landmarks = {
            "left_shoulder": lm(0, 1, 0.1),
            "left_elbow": lm(0, 0, 0.1),
            "left_wrist": lm(1, 0, 0.1),
        }
        assert self.spec.angle_from(landmarks) is None

    def test_missing_landmarks_return_none(self):
        assert self.spec.angle_from({}) is None

    def test_degenerate_geometry_returns_none(self):
        landmarks = {
            "left_shoulder": lm(0, 0),
            "left_elbow": lm(0, 0),
            "left_wrist": lm(1, 0),
        }
        assert self.spec.angle_from(landmarks) is None
