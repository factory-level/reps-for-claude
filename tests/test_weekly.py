from reps_for_claude.weekly import WeeklyLog


def test_log_lift_accumulates_reps_and_volume(tmp_path):
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    log.log_lift("squat", reps=10, lbs=45.0)
    log.log_lift("squat", reps=5, lbs=45.0)
    assert log.state.reps == {"squat": 15}
    assert log.state.volume_lbs == {"squat": 675.0}  # (10+5)*45


def test_persists_and_reloads_same_week(tmp_path):
    WeeklyLog(tmp_path, today=lambda: "2026-07-19").log_jumprope(30.0)
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    # nothing saved yet -> fresh
    assert log.state.jumprope_seconds == 0.0

    a = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    a.log_stretch(20.0)
    a.save()
    b = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    assert b.state.stretch_seconds == 20.0


def test_new_iso_week_resets(tmp_path):
    a = WeeklyLog(tmp_path, today=lambda: "2026-07-19")  # ISO week 29
    a.log_lift("squat", reps=10, lbs=45.0)
    a.save()
    b = WeeklyLog(tmp_path, today=lambda: "2026-07-27")  # ISO week 31
    assert b.state.reps == {}
    assert b.state.week == "2026-W31"


def test_corrupt_file_resets(tmp_path, capsys):
    (tmp_path / "weekly.json").write_text("{not json")
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    assert log.state.reps == {}
    assert "corrupt" in capsys.readouterr().err


def test_negative_rejected(tmp_path):
    import pytest
    log = WeeklyLog(tmp_path, today=lambda: "2026-07-19")
    with pytest.raises(ValueError):
        log.log_lift("squat", reps=-1, lbs=45.0)
