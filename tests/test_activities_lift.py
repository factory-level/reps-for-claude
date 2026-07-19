from reps_for_claude.activities.base import Progress
from reps_for_claude.activities.lift import LiftActivity


def _pose(knee_angle_deg: float):
    """A minimal side-on landmark set that yields the given knee angle.

    Hip at origin, knee straight below it, ankle placed so the hip-knee-ankle
    angle equals `knee_angle_deg`. Coordinates are normalized (0..1) with
    visibility 1.0. Only the squat's KNEE joints need to be present.
    """
    import math
    hip = (0.5, 0.4, 1.0)
    knee = (0.5, 0.6, 1.0)
    rad = math.radians(180 - knee_angle_deg)
    ankle = (0.5 + 0.2 * math.sin(rad), 0.6 + 0.2 * math.cos(rad), 1.0)
    return {"left_hip": hip, "left_knee": knee, "left_ankle": ankle}


def test_counts_one_squat_rep():
    act = LiftActivity("squat", target_reps=2)
    # squat: down_below=110, up_above=160. Start up, go down, come up = 1 rep.
    assert act.update(_pose(170), now=0.0).value == 0.0   # up
    assert act.update(_pose(100), now=1.0).value == 0.0   # down (no rep yet)
    p = act.update(_pose(170), now=2.0)                    # back up -> rep!
    assert p == Progress(1.0, "reps", False)


def test_satisfied_at_target():
    act = LiftActivity("squat", target_reps=1)
    act.update(_pose(170), now=0.0)
    act.update(_pose(100), now=1.0)
    p = act.update(_pose(170), now=2.0)
    assert p.satisfied is True


def test_none_landmarks_advance_nothing():
    act = LiftActivity("squat", target_reps=1)
    p = act.update(None, now=0.0)
    assert p == Progress(0.0, "reps", False)
