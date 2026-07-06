from types import SimpleNamespace

from reps_for_claude.lock import CommandLocker, NoOpLocker, get_locker


class TestGetLocker:
    def test_disabled_is_noop(self):
        assert isinstance(get_locker(False), NoOpLocker)

    def test_enabled_is_command(self):
        assert isinstance(get_locker(True), CommandLocker)


class TestNoOpLocker:
    def test_never_locks(self):
        assert NoOpLocker().lock() is False


class TestCommandLocker:
    def test_first_success_wins(self):
        calls = []

        def runner(cmd, **kw):
            calls.append(cmd)
            return SimpleNamespace(returncode=0)

        assert CommandLocker(runner).lock() is True
        assert len(calls) == 1

    def test_falls_through_missing_commands(self):
        calls = []

        def runner(cmd, **kw):
            calls.append(cmd)
            if len(calls) < 3:
                raise FileNotFoundError
            return SimpleNamespace(returncode=0)

        assert CommandLocker(runner).lock() is True
        assert len(calls) == 3

    def test_all_fail_degrades_gracefully(self, capsys):
        def runner(cmd, **kw):
            raise FileNotFoundError

        assert CommandLocker(runner).lock() is False
        assert "shim-only" in capsys.readouterr().err

    def test_nonzero_exit_tries_next(self):
        calls = []

        def runner(cmd, **kw):
            calls.append(cmd)
            return SimpleNamespace(returncode=1 if len(calls) < 2 else 0)

        assert CommandLocker(runner).lock() is True
        assert len(calls) == 2
