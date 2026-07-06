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
