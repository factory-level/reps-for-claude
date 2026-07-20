"""Tests for scripts/fetch_youtube.py — hermetic, no network.

QUERIES completeness and the manifest round-trip run unconditionally.
verify_clip is exercised only when cv2 is importable (importorskip-guarded),
against the tracked squat_demo.webm sample fixture — never a downloaded clip.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

from reps_vision.exercises import SPECS

SCRIPT_PATH = Path(__file__).parent.parent / "scripts" / "fetch_youtube.py"
SQUAT_DEMO = Path(__file__).parent / "fixtures" / "videos" / "squat_demo.webm"


def _load_script():
    spec = importlib.util.spec_from_file_location("fetch_youtube", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fetch_youtube = _load_script()


class TestQueries:
    def test_covers_every_spec_exercise_plus_jumprope(self):
        assert set(fetch_youtube.QUERIES) == set(SPECS) | {"jumprope"}

    def test_every_query_is_a_nonempty_string(self):
        for exercise, query in fetch_youtube.QUERIES.items():
            assert isinstance(query, str) and query.strip(), exercise


class TestManifestRoundTrip:
    def test_write_then_load(self, tmp_path):
        entries = {
            "squat": {
                "file": "squat.mp4",
                "url": "https://example.com/watch?v=abc123",
                "title": "Squat Demo",
            },
            "pushup": {
                "file": "pushup.mp4",
                "url": "https://example.com/watch?v=def456",
                "title": "Pushup Demo",
            },
        }
        fetch_youtube.write_manifest(tmp_path, entries)
        assert fetch_youtube.load_manifest(tmp_path) == entries

    def test_load_missing_manifest_returns_empty_dict(self, tmp_path):
        assert fetch_youtube.load_manifest(tmp_path) == {}

    def test_manifest_filename_is_stable(self, tmp_path):
        fetch_youtube.write_manifest(tmp_path, {})
        assert (tmp_path / "youtube_manifest.json").exists()


class TestVerifyClip:
    def setup_method(self):
        pytest.importorskip("cv2")

    def test_tracked_sample_clip_verifies(self):
        assert fetch_youtube.verify_clip(SQUAT_DEMO) is True

    def test_missing_file_fails(self, tmp_path):
        assert fetch_youtube.verify_clip(tmp_path / "does-not-exist.mp4") is False

    def test_too_few_frames_fails(self, tmp_path):
        cv2 = pytest.importorskip("cv2")
        np = pytest.importorskip("numpy")
        clip = tmp_path / "short.avi"
        writer = cv2.VideoWriter(
            str(clip), cv2.VideoWriter_fourcc(*"MJPG"), 10.0, (64, 48)
        )
        assert writer.isOpened()
        for _ in range(5):
            writer.write(np.zeros((48, 64, 3), dtype="uint8"))
        writer.release()
        assert fetch_youtube.verify_clip(clip) is False
