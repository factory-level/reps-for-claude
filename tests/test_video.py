"""Frame-loop tests with a fake estimator — no MediaPipe, no camera.

A tiny synthetic clip is written with cv2.VideoWriter; the fake estimator
scripts one landmark set per frame, so rep counting is exercised end-to-end
through VideoRepCounter without any real pose estimation.
"""

import pytest

from reps_for_claude.detector import DetectorError
from reps_for_claude.video import VideoRepCounter

cv2 = pytest.importorskip("cv2")
np = pytest.importorskip("numpy")


def write_clip(path, frames: int) -> str:
    writer = cv2.VideoWriter(
        str(path), cv2.VideoWriter_fourcc(*"MJPG"), 10.0, (64, 48)
    )
    assert writer.isOpened()
    for _ in range(frames):
        writer.write(np.zeros((48, 64, 3), dtype=np.uint8))
    writer.release()
    return str(path)


def arm_pose(elbow_angle_kind: str) -> dict:
    """Landmarks for a straight ('up') or right-angled ('down') left arm."""
    wrist = (1.0, 0.0) if elbow_angle_kind == "up" else (0.0, 1.0)
    return {
        "left_shoulder": (-1.0, 0.0, 0.9),
        "left_elbow": (0.0, 0.0, 0.9),
        "left_wrist": (*wrist, 0.9),
    }


class FakeEstimator:
    def __init__(self, script):
        self._script = list(script)
        self.closed = False

    def landmarks(self, frame):
        return self._script.pop(0) if self._script else None

    def close(self):
        self.closed = True


def make_counter(clip, script):
    estimator = FakeEstimator(script)
    counter = VideoRepCounter(clip, estimator_factory=lambda: estimator)
    return counter, estimator


class TestVideoRepCounter:
    def test_counts_scripted_reps(self, tmp_path):
        # up -> down -> up -> down -> up: two pushups
        script = [arm_pose(k) for k in ("up", "down", "up", "down", "up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        counter, estimator = make_counter(clip, script)
        seen = []
        assert counter.run("pushup", seen.append) == 2
        assert seen == [1, 2]
        assert estimator.closed

    def test_records_rep_timestamps(self, tmp_path):
        script = [arm_pose(k) for k in ("up", "down", "up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        counter, _ = make_counter(clip, script)
        counter.run("pushup", lambda n: None)
        assert len(counter.rep_timestamps_ms) == 1
        assert counter.rep_timestamps_ms[0] >= 0.0

    def test_frames_without_pose_are_skipped(self, tmp_path):
        script = [arm_pose("up"), None, arm_pose("down"), None, arm_pose("up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        counter, _ = make_counter(clip, script)
        assert counter.run("pushup", lambda n: None) == 1

    def test_unknown_exercise_rejected(self, tmp_path):
        clip = write_clip(tmp_path / "clip.avi", frames=1)
        counter, _ = make_counter(clip, [])
        with pytest.raises(DetectorError, match="unknown exercise"):
            counter.run("juggling", lambda n: None)

    def test_unopenable_source_rejected(self):
        counter = VideoRepCounter(
            "/nonexistent/clip.avi", estimator_factory=lambda: FakeEstimator([])
        )
        with pytest.raises(DetectorError, match="could not open"):
            counter.run("pushup", lambda n: None)

    def test_partial_movement_counts_nothing(self, tmp_path):
        script = [arm_pose("up")] * 4  # arm never bends
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        counter, _ = make_counter(clip, script)
        assert counter.run("pushup", lambda n: None) == 0

    def test_on_frame_fires_once_per_decoded_frame(self, tmp_path):
        # includes a None (no-pose) frame — the hook must still fire for it
        script = [arm_pose("up"), None, arm_pose("down"), arm_pose("up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        counter, _ = make_counter(clip, script)
        frames = []
        counter.run(
            "pushup",
            lambda n: None,
            on_frame=lambda f, lm, ang, c: frames.append((lm is not None, ang, c)),
        )
        assert len(frames) == 4
        # rep completes on the final up; count reaches 1, last angle is not None
        assert frames[-1][2] == 1
        assert frames[1][0] is False  # the no-pose frame reported no landmarks
