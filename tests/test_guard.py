from reps_for_claude.guard import EXIT_EXHAUSTED, EXIT_NO_CREDIT, Guard, reps_needed
from reps_for_claude.lock import NoOpLocker


class FakeClock:
    """Monotonic clock advanced by sleep() — the guard's only time sources."""

    def __init__(self):
        self.now = 0.0

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += seconds


class FakeProc:
    """poll() returns None until the clock passes `runs_for` seconds."""

    def __init__(self, clock: FakeClock, runs_for: float, returncode: int = 0):
        self._clock = clock
        self._ends_at = clock.now + runs_for
        self._rc = returncode
        self.terminated = False

    def poll(self):
        if self.terminated or self._clock.now >= self._ends_at:
            return self._rc
        return None

    def terminate(self):
        self.terminated = True
        self._rc = -15

    def wait(self):
        return self._rc


class SpyLocker:
    def __init__(self):
        self.locked = False

    def lock(self) -> bool:
        self.locked = True
        return True


def make_guard(ledger, cfg, clock, proc, locker=None, tick=5.0):
    return Guard(
        ledger,
        cfg,
        locker or NoOpLocker(),
        popen=lambda argv: proc,
        monotonic=clock.monotonic,
        sleep=clock.sleep,
        tick_seconds=tick,
        err=lambda m: None,
    )


class TestGuard:
    def test_refuses_with_zero_balance(self, cfg, ledger):
        clock = FakeClock()
        guard = make_guard(ledger, cfg, clock, FakeProc(clock, 10))
        assert guard.run(["claude"]) == EXIT_NO_CREDIT

    def test_passes_through_exit_code(self, cfg, ledger):
        ledger.set_balance(1000.0)
        clock = FakeClock()
        proc = FakeProc(clock, runs_for=12.0, returncode=7)
        guard = make_guard(ledger, cfg, clock, proc)
        assert guard.run(["claude"]) == 7

    def test_decrements_balance_by_session_time(self, cfg, ledger):
        ledger.set_balance(1000.0)
        clock = FakeClock()
        guard = make_guard(ledger, cfg, clock, FakeProc(clock, runs_for=30.0))
        guard.run(["claude"])
        assert ledger.state.balance_seconds == 970.0
        assert ledger.state.spent_seconds == 30.0

    def test_kills_session_when_exhausted(self, cfg, ledger):
        ledger.set_balance(20.0)
        clock = FakeClock()
        proc = FakeProc(clock, runs_for=10_000.0)
        locker = SpyLocker()
        guard = make_guard(ledger, cfg, clock, proc, locker=locker)
        assert guard.run(["claude"]) == EXIT_EXHAUSTED
        assert proc.terminated
        assert locker.locked
        assert ledger.state.balance_seconds == 0.0

    def test_persists_on_exit(self, cfg, ledger, tmp_path):
        ledger.set_balance(500.0)
        clock = FakeClock()
        guard = make_guard(ledger, cfg, clock, FakeProc(clock, runs_for=45.0))
        guard.run(["claude"])
        from reps_for_claude.ledger import Ledger

        reloaded = Ledger(tmp_path / "state", today=lambda: "2026-07-06")
        assert reloaded.state.balance_seconds == 455.0

    def test_persists_even_if_child_crashes_the_loop(self, cfg, ledger):
        ledger.set_balance(500.0)
        clock = FakeClock()

        class ExplodingProc(FakeProc):
            def poll(self):
                if clock.now >= 10.0:
                    raise RuntimeError("boom")
                return None

        guard = make_guard(ledger, cfg, clock, ExplodingProc(clock, 10_000.0))
        try:
            guard.run(["claude"])
        except RuntimeError:
            pass
        # time up to the last completed tick was charged and saved
        assert ledger.state.balance_seconds < 500.0


class TestRepsNeeded:
    def test_rounds_up(self):
        assert reps_needed(100.0, 90) == 2

    def test_minimum_one(self):
        assert reps_needed(0.0, 90) == 1
