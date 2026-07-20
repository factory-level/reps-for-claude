"""JSON-lines streaming protocol tests — hermetic, fake estimator, no mediapipe.

Mirrors the fake-estimator injection pattern from test_video.py, driving
run_stream() instead of VideoRepCounter directly so the sidecar protocol
(open/progress/frame/done events) is exercised without a real pose model.
"""

from __future__ import annotations

import base64

import pytest

from reps_vision.detector import DetectorError
from reps_vision.stream import run_stream

cv2 = pytest.importorskip("cv2")
np = pytest.importorskip("numpy")

JPEG_MAGIC = b"\xff\xd8"


def write_clip(path, frames: int, size=(64, 48)) -> str:
    writer = cv2.VideoWriter(
        str(path), cv2.VideoWriter_fourcc(*"MJPG"), 10.0, size
    )
    assert writer.isOpened()
    for _ in range(frames):
        writer.write(np.zeros((size[1], size[0], 3), dtype=np.uint8))
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


def hip_pose(y: float) -> dict:
    return {
        "left_hip": (0.5, y, 0.9),
        "right_hip": (0.5, y, 0.9),
    }


class FakeEstimator:
    def __init__(self, script):
        self._script = list(script)
        self.closed = False

    def landmarks(self, frame):
        return self._script.pop(0) if self._script else None

    def close(self):
        self.closed = True


def collect(video, exercise, script, **kwargs):
    estimator = FakeEstimator(script)
    events = []
    total = run_stream(video, exercise, events.append, estimator=estimator, **kwargs)
    return events, total, estimator


class TestOpenEvent:
    def test_open_is_first_event_with_metadata(self, tmp_path):
        script = [arm_pose(k) for k in ("up", "down", "up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, _, _ = collect(clip, "pushup", script)
        assert events[0]["event"] == "open"
        assert events[0]["exercise"] == "pushup"
        assert events[0]["fps"] > 0
        assert events[0]["frameCount"] >= len(script)


class TestProgressEvents:
    def test_progress_fires_every_decoded_frame(self, tmp_path):
        # up -> down -> up -> down -> up: two pushups
        script = [arm_pose(k) for k in ("up", "down", "up", "down", "up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, total, _ = collect(clip, "pushup", script)
        progress = [e for e in events if e["event"] == "progress"]
        assert len(progress) == len(script)
        assert [p["frame"] for p in progress] == list(range(len(script)))
        assert progress[-1]["value"] == 2
        assert total == 2
        assert all(p["unit"] == "reps" for p in progress)

    def test_estimator_closed_after_run(self, tmp_path):
        script = [arm_pose("up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        _, _, estimator = collect(clip, "pushup", script)
        assert estimator.closed


class TestFrameEvents:
    def test_jpeg_every_cadence(self, tmp_path):
        script = [arm_pose("up") for _ in range(6)]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, _, _ = collect(clip, "pushup", script, jpeg_every=3)
        frames = [e for e in events if e["event"] == "frame"]
        assert [f["frame"] for f in frames] == [0, 3]

    def test_jpeg_b64_decodes_to_real_jpeg(self, tmp_path):
        script = [arm_pose("up") for _ in range(2)]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, _, _ = collect(clip, "pushup", script, jpeg_every=1)
        frame_events = [e for e in events if e["event"] == "frame"]
        assert frame_events
        raw = base64.b64decode(frame_events[0]["jpegB64"])
        assert raw[:2] == JPEG_MAGIC

    def test_jpeg_downscaled_to_requested_width(self, tmp_path):
        script = [arm_pose("up")]
        clip = write_clip(tmp_path / "clip.avi", frames=1, size=(64, 48))
        events, _, _ = collect(clip, "pushup", script, jpeg_every=1, jpeg_width=32)
        frame_events = [e for e in events if e["event"] == "frame"]
        raw = base64.b64decode(frame_events[0]["jpegB64"])
        decoded = cv2.imdecode(np.frombuffer(raw, dtype=np.uint8), cv2.IMREAD_COLOR)
        assert decoded.shape[1] == 32


class TestDoneEvent:
    def test_done_is_last_event_and_matches_return(self, tmp_path):
        script = [arm_pose(k) for k in ("up", "down", "up")]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, total, _ = collect(clip, "pushup", script)
        assert events[-1]["event"] == "done"
        assert events[-1]["total"] == total == 1
        assert events[-1]["satisfied"] is False


class TestUnknownExercise:
    def test_raises_before_emitting_events(self, tmp_path):
        clip = write_clip(tmp_path / "clip.avi", frames=1)
        events = []
        with pytest.raises(DetectorError, match="unknown exercise"):
            run_stream(clip, "juggling", events.append, estimator=FakeEstimator([]))
        assert events == []


class TestJumpRopePath:
    def test_progress_uses_seconds_unit(self, tmp_path):
        # oscillating hip y so consecutive frames register as "moving"
        script = [hip_pose(0.3), hip_pose(0.5), hip_pose(0.3), hip_pose(0.5)]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, total, estimator = collect(clip, "jumprope", script)
        assert events[0]["event"] == "open"
        assert events[0]["exercise"] == "jumprope"
        progress = [e for e in events if e["event"] == "progress"]
        assert len(progress) == len(script)
        assert all(p["unit"] == "seconds" for p in progress)
        assert progress[-1]["value"] >= 0.0
        assert isinstance(total, int)
        assert events[-1]["event"] == "done"
        assert events[-1]["total"] == total
        assert estimator.closed

    def test_jumprope_emits_frame_events(self, tmp_path):
        script = [hip_pose(0.3), hip_pose(0.5)]
        clip = write_clip(tmp_path / "clip.avi", frames=len(script))
        events, _, _ = collect(clip, "jumprope", script, jpeg_every=1)
        frame_events = [e for e in events if e["event"] == "frame"]
        assert len(frame_events) == len(script)
        raw = base64.b64decode(frame_events[0]["jpegB64"])
        assert raw[:2] == JPEG_MAGIC
