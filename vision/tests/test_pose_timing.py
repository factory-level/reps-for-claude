"""Exercise timestamps reaching MediaPipe without loading a model or camera."""
from types import SimpleNamespace

import pytest

from reps_vision.detector import DetectorError
from reps_vision.pose import PoseEstimator


def estimator():
    instance = PoseEstimator.__new__(PoseEstimator)
    instance._timestamp_ms = -1
    instance._cv2 = SimpleNamespace(COLOR_BGR2RGB=1, cvtColor=lambda frame, _: frame)
    instance._mp = SimpleNamespace(ImageFormat=SimpleNamespace(SRGB=1), Image=lambda **kw: kw)
    times = []

    def detect(image, timestamp):
        times.append(timestamp)
        return SimpleNamespace(pose_landmarks=[])

    instance._landmarker = SimpleNamespace(detect_for_video=detect)
    return instance, times


def test_irregular_source_intervals_reach_tracker():
    pose, times = estimator()
    for timestamp in [0, 41.7, 83.4, 250, 1250]:
        pose.landmarks(object(), timestamp_ms=timestamp)
    assert times == [0, 41, 83, 250, 1250]


def test_integer_collisions_remain_strictly_increasing():
    pose, times = estimator()
    for timestamp in [0, 0.4, 1, 40]:
        pose.landmarks(object(), timestamp_ms=timestamp)
    assert times == [0, 1, 2, 40]


def test_standalone_uses_monotonic_clock(monkeypatch):
    pose, times = estimator()
    clock = iter([10, 10.125, 11])
    monkeypatch.setattr('reps_vision.pose.time.monotonic', lambda: next(clock))
    for _ in range(3):
        pose.landmarks(object())
    assert times == [10000, 10125, 11000]


@pytest.mark.parametrize('timestamp', [-1, float('inf'), float('nan')])
def test_invalid_timestamps_never_reach_tracker(timestamp):
    pose, times = estimator()
    with pytest.raises(DetectorError, match='timestamp'):
        pose.landmarks(object(), timestamp_ms=timestamp)
    assert times == []
