"""Thin MediaPipe Pose wrapper — the only module that imports mediapipe.

Uses the Tasks API (the legacy solutions API was removed from mediapipe
0.10.30+). The Full pose model is a ~9MB .task file downloaded once into the
cache dir; override its location with REPS_POSE_MODEL. Swapping models or
backends later only touches this file.
"""

from __future__ import annotations

import hashlib
import math
import time
import tempfile
import os
import sys
import urllib.request
from pathlib import Path

from .paths import cache_dir
from .detector import DetectorError
from .exercises import Landmarks

INSTALL_HINT = "install the CV extra first: uv sync --extra cv"

MODEL_URL = (
    "https://storage.googleapis.com/mediapipe-models/pose_landmarker/"
    "pose_landmarker_full/float16/1/pose_landmarker_full.task"
)

MODEL_SHA256 = "5134a3aad27a58b93da0088d431f366da362b44e3ccfbe3462b3827a839011b1"

# The 33-point MediaPipe pose topology, in output order.
POSE_LANDMARK_NAMES = [
    "nose",
    "left_eye_inner", "left_eye", "left_eye_outer",
    "right_eye_inner", "right_eye", "right_eye_outer",
    "left_ear", "right_ear",
    "mouth_left", "mouth_right",
    "left_shoulder", "right_shoulder",
    "left_elbow", "right_elbow",
    "left_wrist", "right_wrist",
    "left_pinky", "right_pinky",
    "left_index", "right_index",
    "left_thumb", "right_thumb",
    "left_hip", "right_hip",
    "left_knee", "right_knee",
    "left_ankle", "right_ankle",
    "left_heel", "right_heel",
    "left_foot_index", "right_foot_index",
]

def ensure_model() -> Path:
    """Return the pose model path, downloading it into the cache once."""
    override = os.environ.get("REPS_POSE_MODEL")
    path = Path(override) if override else cache_dir() / "pose_landmarker_full.task"
    if not path.exists():
        if override:
            raise DetectorError(f"configured pose model is missing: {path}")
        path.parent.mkdir(parents=True, exist_ok=True)
        fd, temporary = tempfile.mkstemp(dir=path.parent, suffix=".part")
        try:
            with os.fdopen(fd, "wb") as output, urllib.request.urlopen(MODEL_URL, timeout=30) as response:
                output.write(response.read(32 * 1024 * 1024))
            if hashlib.sha256(Path(temporary).read_bytes()).hexdigest() != MODEL_SHA256:
                raise DetectorError("downloaded pose model checksum mismatch")
            os.replace(temporary, path)
        except OSError as exc:
            raise DetectorError(f"could not download pose model: {exc}") from exc
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
    if hashlib.sha256(path.read_bytes()).hexdigest() != MODEL_SHA256:
        raise DetectorError("pose model checksum mismatch; reinstall the pinned model")
    return path


class PoseEstimator:
    def __init__(self, model: Path | None = None) -> None:
        try:
            import cv2
            import mediapipe as mp
            from mediapipe.tasks.python import BaseOptions
            from mediapipe.tasks.python.vision import (
                PoseLandmarker,
                PoseLandmarkerOptions,
                RunningMode,
            )
        except ImportError as e:
            raise DetectorError(f"mediapipe/opencv not available — {INSTALL_HINT}") from e
        self._cv2 = cv2
        self._mp = mp
        self._timestamp_ms = -1
        options = PoseLandmarkerOptions(
            base_options=BaseOptions(model_asset_path=str(model or ensure_model())),
            running_mode=RunningMode.VIDEO,
            min_pose_detection_confidence=0.5,
            min_tracking_confidence=0.5,
        )
        self._landmarker = PoseLandmarker.create_from_options(options)

    def landmarks(self, frame_bgr, *, timestamp_ms: float | None = None) -> Landmarks | None:
        """Landmarks for the frame as {name: (x, y, visibility)}, or None."""
        # Replay supplies source time; standalone callers use elapsed wall time.
        # MediaPipe requires strictly increasing integer milliseconds, so frames
        # sharing a millisecond advance by the smallest representable interval.
        source_ms = time.monotonic() * 1000 if timestamp_ms is None else timestamp_ms
        if not math.isfinite(source_ms) or source_ms < 0:
            raise DetectorError("pose timestamp must be finite and nonnegative")
        next_ms = max(int(source_ms), self._timestamp_ms + 1)
        rgb = self._cv2.cvtColor(frame_bgr, self._cv2.COLOR_BGR2RGB)
        image = self._mp.Image(image_format=self._mp.ImageFormat.SRGB, data=rgb)
        result = self._landmarker.detect_for_video(image, next_ms)
        self._timestamp_ms = next_ms
        if not result.pose_landmarks:
            return None
        return {
            name: (lm.x, lm.y, lm.visibility)
            for name, lm in zip(POSE_LANDMARK_NAMES, result.pose_landmarks[0])
        }

    def close(self) -> None:
        self._landmarker.close()
