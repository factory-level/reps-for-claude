import io

import pytest

from reps_for_claude.detector import (
    DetectorError,
    KeyboardDetector,
    StubDetector,
    get_detector,
)


class TestStubDetector:
    def test_emits_fixed_reps(self):
        seen = []
        total = StubDetector(5).run("pushup", seen.append)
        assert total == 5
        assert seen == [1, 2, 3, 4, 5]

    def test_zero_reps(self):
        assert StubDetector(0).run("pushup", lambda n: None) == 0


class TestKeyboardDetector:
    def test_counts_lines_until_done(self):
        det = KeyboardDetector(io.StringIO("\n\n\ndone\n\n"))
        seen = []
        assert det.run("squat", seen.append) == 3
        assert seen == [1, 2, 3]

    def test_eof_ends_session(self):
        det = KeyboardDetector(io.StringIO("\n\n"))
        assert det.run("squat", lambda n: None) == 2

    def test_quit_aliases(self):
        assert KeyboardDetector(io.StringIO("\nq\n\n")).run("row", lambda n: None) == 1


class TestRegistry:
    def test_stub(self):
        det = get_detector("stub", total_reps=7)
        assert isinstance(det, StubDetector)
        assert det.total_reps == 7

    def test_keyboard(self):
        assert isinstance(get_detector("keyboard"), KeyboardDetector)

    def test_mediapipe_returns_video_counter(self):
        from reps_for_claude.video import VideoRepCounter

        det = get_detector("mediapipe", camera_index=2)
        assert isinstance(det, VideoRepCounter)
        assert det._source == 2

    def test_yolo_not_yet_implemented(self):
        with pytest.raises(DetectorError, match="not implemented yet"):
            get_detector("yolo-pose")

    def test_unknown_name(self):
        with pytest.raises(DetectorError, match="unknown detector"):
            get_detector("telepathy")
