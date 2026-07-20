from reps_vision.activities.jumprope import JumpRopeActivity


def _hips(y: float):
    return {"left_hip": (0.5, y, 1.0), "right_hip": (0.5, y, 1.0)}


def test_bouncing_accrues_time():
    act = JumpRopeActivity(target_seconds=3.0)
    act.update(_hips(0.50), now=0.0)          # first frame: seeds baseline
    p = act.update(_hips(0.40), now=1.0)      # moved 0.10 >= threshold -> +1s
    assert p.value == 1.0 and not p.satisfied
    p = act.update(_hips(0.50), now=2.0)      # moved -> +1s
    p = act.update(_hips(0.40), now=3.0)      # moved -> +1s -> streak 3.0
    assert p.value == 3.0 and p.satisfied


def test_stillness_resets_streak():
    act = JumpRopeActivity(target_seconds=10.0, reset_after=2.0)
    act.update(_hips(0.50), now=0.0)
    act.update(_hips(0.40), now=1.0)          # streak 1.0
    act.update(_hips(0.40), now=2.0)          # still (dt within reset window)
    p = act.update(_hips(0.40), now=4.0)      # still >= reset_after -> reset
    assert p.value == 0.0


def test_none_landmarks_treated_as_still():
    act = JumpRopeActivity(target_seconds=10.0, reset_after=2.0)
    act.update(_hips(0.50), now=0.0)
    p = act.update(None, now=1.0)
    assert p.value == 0.0 and p.unit == "seconds"
