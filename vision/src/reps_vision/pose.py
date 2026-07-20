"""Thin MediaPipe Pose wrapper — the only module that imports mediapipe.

Uses the Tasks API (the legacy solutions API was removed from mediapipe
0.10.30+). The pose model is a ~5MB .task file downloaded once into the
cache dir; override its location with REPS_POSE_MODEL. Swapping models or
backends later only touches this file.
"""

from __future__ import annotations

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
    "pose_landmarker_lite/float16/latest/pose_landmarker_lite.task"
)

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

_FRAME_MS = 33  # synthetic timestamp step for the VIDEO running mode


def ensure_model() -> Path:
    """Return the pose model path, downloading it into the cache once."""
    override = os.environ.get("REPS_POSE_MODEL")
    if override:
        return Path(override)
    path = cache_dir() / "pose_landmarker_lite.task"
    if not path.exists():
        print(f"downloading pose model to {path} ...", file=sys.stderr)
        path.parent.mkdir(parents=True, exist_ok=True)
        try:
            with urllib.request.urlopen(MODEL_URL) as resp:
                path.write_bytes(resp.read())
        except OSError as e:
            raise DetectorError(f"could not download pose model: {e}") from e
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
        self._timestamp_ms = 0
        options = PoseLandmarkerOptions(
            base_options=BaseOptions(model_asset_path=str(model or ensure_model())),
            running_mode=RunningMode.VIDEO,
            min_pose_detection_confidence=0.5,
            min_tracking_confidence=0.5,
        )
        self._landmarker = PoseLandmarker.create_from_options(options)

    def landmarks(self, frame_bgr) -> Landmarks | None:
        """Landmarks for the frame as {name: (x, y, visibility)}, or None."""
        rgb = self._cv2.cvtColor(frame_bgr, self._cv2.COLOR_BGR2RGB)
        image = self._mp.Image(image_format=self._mp.ImageFormat.SRGB, data=rgb)
        self._timestamp_ms += _FRAME_MS
        result = self._landmarker.detect_for_video(image, self._timestamp_ms)
        if not result.pose_landmarks:
            return None
        return {
            name: (lm.x, lm.y, lm.visibility)
            for name, lm in zip(POSE_LANDMARK_NAMES, result.pose_landmarks[0])
        }

    def close(self) -> None:
        self._landmarker.close()
