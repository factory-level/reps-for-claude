from reps_for_claude.activities.stretch import StretchActivity


def test_hold_accumulates_from_first_update():
    act = StretchActivity(target_seconds=30.0)
    assert act.update(None, now=100.0).value == 0.0     # starts the clock
    p = act.update(None, now=115.0)
    assert p.value == 15.0 and not p.satisfied


def test_satisfied_after_target():
    act = StretchActivity(target_seconds=30.0)
    act.update(None, now=0.0)
    p = act.update(None, now=30.0)
    assert p.value == 30.0 and p.satisfied and p.unit == "seconds"
