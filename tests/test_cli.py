from typer.testing import CliRunner

from reps_for_claude.cli import app
from reps_for_claude.ledger import Ledger

runner = CliRunner()

CONFIG = """\
[economics]
seconds_per_rep = 60
precompletion_cap_seconds = 600

[plan]
pushup = 3
squat = 2

[detector]
name = "keyboard"
"""


def write_config(reps_home, body=CONFIG):
    (reps_home / "config.toml").write_text(body)


class TestInit:
    def test_writes_sample_config(self, reps_home):
        result = runner.invoke(app, ["init"])
        assert result.exit_code == 0
        assert (reps_home / "config.toml").exists()


class TestEarn:
    def test_keyboard_earn_credits_balance(self, reps_home):
        write_config(reps_home)
        result = runner.invoke(app, ["earn", "pushup"], input="\n\n\ndone\n")
        assert result.exit_code == 0
        assert "3 pushup logged" in result.output
        assert "+3m 00s" in result.output

    def test_completing_plan_reports_uncapped(self, reps_home):
        write_config(reps_home)
        runner.invoke(app, ["earn", "pushup"], input="\n\n\ndone\n")
        result = runner.invoke(app, ["earn", "squat"], input="\n\ndone\n")
        assert "Daily workout complete" in result.output


class TestStatus:
    def test_shows_plan_progress(self, reps_home):
        write_config(reps_home)
        runner.invoke(app, ["earn", "pushup"], input="\n\ndone\n")
        result = runner.invoke(app, ["status"])
        assert result.exit_code == 0
        assert "pushup: 2/3" in result.output
        assert "capped" in result.output


class TestBalance:
    def test_zero_by_default(self, reps_home):
        write_config(reps_home)
        result = runner.invoke(app, ["balance"])
        assert result.exit_code == 0
        assert "0m 00s" in result.output


class TestFinish:
    def test_writes_form_files(self, reps_home):
        write_config(reps_home)
        runner.invoke(app, ["earn", "pushup"], input="\n\n\ndone\n")
        result = runner.invoke(app, ["finish"], input="\n\n")
        assert result.exit_code == 0
        assert (reps_home / "state" / "logs").exists()
        files = sorted(p.name for p in (reps_home / "state" / "logs").iterdir())
        assert len(files) == 2
        assert files[0].endswith(".json") and files[1].endswith(".md")


def test_guard_and_shim_commands_removed():
    from typer.testing import CliRunner
    from reps_for_claude.cli import app
    runner = CliRunner()
    for cmd in ("guard", "install-shim", "uninstall-shim"):
        result = runner.invoke(app, [cmd, "--help"])
        assert result.exit_code != 0, f"{cmd} should no longer exist"
