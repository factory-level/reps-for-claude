import pytest

from reps_for_claude import config as config_mod
from reps_for_claude.config import Config, ConfigError, load, write_sample


class TestLoad:
    def test_missing_file_gives_defaults(self, tmp_path):
        cfg = load(tmp_path / "nope.toml")
        assert cfg == Config()

    def test_full_config(self, tmp_path):
        path = tmp_path / "config.toml"
        path.write_text(
            """
            [economics]
            seconds_per_rep = 45
            precompletion_cap_seconds = 900

            [plan]
            pushup = 30
            row = 24

            [detector]
            name = "stub"

            [lock]
            enabled = true

            [claude]
            real_binary = "/opt/claude/claude"
            """
        )
        cfg = load(path)
        assert cfg.seconds_per_rep == 45
        assert cfg.precompletion_cap_seconds == 900
        assert cfg.plan == {"pushup": 30, "row": 24}
        assert cfg.detector == "stub"
        assert cfg.lock_enabled is True
        assert cfg.real_claude == "/opt/claude/claude"

    def test_malformed_toml(self, tmp_path):
        path = tmp_path / "config.toml"
        path.write_text("this is [ not toml")
        with pytest.raises(ConfigError, match="could not parse"):
            load(path)

    @pytest.mark.parametrize(
        "body,match",
        [
            ("[economics]\nseconds_per_rep = -5", "seconds_per_rep"),
            ("[economics]\nseconds_per_rep = 'lots'", "seconds_per_rep"),
            ("[plan]\npushup = 0", "plan.pushup"),
            ("[lock]\nenabled = 'yes'", "lock.enabled"),
        ],
    )
    def test_invalid_values(self, tmp_path, body, match):
        path = tmp_path / "config.toml"
        path.write_text(body)
        with pytest.raises(ConfigError, match=match):
            load(path)


class TestDirs:
    def test_reps_home_overrides(self, monkeypatch, tmp_path):
        monkeypatch.setenv("REPS_HOME", str(tmp_path))
        assert config_mod.config_dir() == tmp_path
        assert config_mod.state_dir() == tmp_path / "state"


class TestWriteSample:
    def test_writes_valid_sample(self, tmp_path):
        path = write_sample(tmp_path / "config.toml")
        cfg = load(path)
        assert cfg.plan  # sample has a plan
        assert cfg.detector == "keyboard"

    def test_does_not_overwrite(self, tmp_path):
        path = tmp_path / "config.toml"
        path.write_text("[plan]\nrow = 1\n")
        write_sample(path)
        assert load(path).plan == {"row": 1}


def test_loads_new_model_sections(tmp_path):
    path = tmp_path / "config.toml"
    path.write_text(
        """
[session]
work_minutes = 6

[goals.weekly]
squat = 60
bench = 40

[break]
default_reps = 12
jumprope_seconds = 90
stretch_seconds = 45

[detector]
width = 640
height = 480

[display]
scoreboard_monitor = 2

[lock]
override_password = "hunter2"
"""
    )
    cfg = load(path)
    assert cfg.work_minutes == 6
    assert cfg.weekly_goals == {"squat": 60, "bench": 40}
    assert cfg.default_reps == 12
    assert cfg.jumprope_seconds == 90
    assert cfg.stretch_seconds == 45
    assert cfg.cam_width == 640
    assert cfg.cam_height == 480
    assert cfg.scoreboard_monitor == 2
    assert cfg.override_password == "hunter2"


def test_new_model_defaults(tmp_path):
    cfg = load(tmp_path / "missing.toml")
    assert cfg.work_minutes == 6
    assert cfg.weekly_goals == {}
    assert cfg.default_reps == 10
    assert cfg.scoreboard_monitor == 1
    assert cfg.override_password == ""


def test_weekly_goals_reject_non_positive(tmp_path):
    path = tmp_path / "config.toml"
    path.write_text("[goals.weekly]\nsquat = 0\n")
    with pytest.raises(ConfigError):
        load(path)
