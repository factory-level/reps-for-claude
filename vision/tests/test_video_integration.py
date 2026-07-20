"""Real MediaPipe over real footage. Run with: uv run pytest -m cvvideo

Fixtures are downloaded by scripts/fetch_fixtures.py into
tests/fixtures/videos/ with a manifest.json of expected counts; these tests
auto-skip when the fixtures or mediapipe are absent.
"""

import json
from pathlib import Path

import pytest

FIXTURES = Path(__file__).parent / "fixtures" / "videos"
MANIFEST = FIXTURES / "manifest.json"

pytestmark = pytest.mark.cvvideo


def load_manifest():
    if not MANIFEST.exists():
        pytest.skip("video fixtures not downloaded (run scripts/fetch_fixtures.py)")
    return json.loads(MANIFEST.read_text())


def clips():
    if not MANIFEST.exists():
        return []
    return [pytest.param(entry, id=entry["file"]) for entry in json.loads(MANIFEST.read_text())]


@pytest.mark.parametrize("entry", clips() or [pytest.param(None, id="no-fixtures")])
def test_counts_match_manifest(entry):
    pytest.importorskip("mediapipe")
    if entry is None:
        pytest.skip("video fixtures not downloaded (run scripts/fetch_fixtures.py)")
    from reps_vision.video import VideoRepCounter

    clip = FIXTURES / entry["file"]
    if not clip.exists():
        pytest.skip(f"{entry['file']} missing (run scripts/fetch_fixtures.py)")
    counter = VideoRepCounter(str(clip))
    total = counter.run(entry["exercise"], lambda n: None)
    expected, tolerance = entry["expected_reps"], entry["tolerance"]
    assert abs(total - expected) <= tolerance, (
        f"{entry['file']}: detected {total} {entry['exercise']} reps, "
        f"expected {expected}±{tolerance}"
    )


def test_video_writes_visualization(tmp_path, monkeypatch):
    """VideoRepCounter(annotate=True) + a frame-writer sink produce a non-empty
    annotated video while rep counting runs over a real fixture clip — the same
    wiring the (now-retired) `reps analyze --output` command used, exercised
    directly against the library API instead of a CLI process."""
    manifest = load_manifest()
    pytest.importorskip("mediapipe")
    cv2 = pytest.importorskip("cv2")
    entry = next((e for e in manifest if e["file"] == "squat_demo.webm"), manifest[0])
    clip = FIXTURES / entry["file"]
    if not clip.exists():
        pytest.skip(f"{entry['file']} missing")

    from reps_vision.video import VideoRepCounter

    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))

    # Probe the input's frame rate up front, same as the legacy writer sink,
    # so the annotated output plays back at the correct speed.
    probe = cv2.VideoCapture(str(clip))
    fps = probe.get(cv2.CAP_PROP_FPS) or 25.0
    probe.release()

    out = tmp_path / "annotated.mp4"
    writer: dict[str, object] = {}

    def on_frame(frame, landmarks, angle, count):
        if "w" not in writer:
            h, w = frame.shape[:2]
            writer["w"] = cv2.VideoWriter(
                str(out), cv2.VideoWriter_fourcc(*"mp4v"), fps, (w, h)
            )
        writer["w"].write(frame)  # type: ignore[union-attr]
        return False

    counter = VideoRepCounter(str(clip), annotate=True)
    total = counter.run(entry["exercise"], lambda n: None, on_frame=on_frame)

    if "w" in writer:
        writer["w"].release()  # type: ignore[union-attr]

    assert out.exists() and out.stat().st_size > 0
    expected, tolerance = entry["expected_reps"], entry["tolerance"]
    assert abs(total - expected) <= tolerance, (
        f"{entry['file']}: detected {total} {entry['exercise']} reps, "
        f"expected {expected}±{tolerance}"
    )
