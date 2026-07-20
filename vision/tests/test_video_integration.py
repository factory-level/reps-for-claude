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


def test_analyze_writes_visualization(tmp_path, monkeypatch):
    """`reps analyze --output` produces a non-empty annotated video."""
    manifest = load_manifest()
    pytest.importorskip("mediapipe")
    entry = manifest[0]
    clip = FIXTURES / entry["file"]
    if not clip.exists():
        pytest.skip(f"{entry['file']} missing")

    from typer.testing import CliRunner

    from reps_vision.cli import app

    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))
    out = tmp_path / "annotated.mp4"
    result = CliRunner().invoke(
        app,
        ["analyze", str(clip), "--exercise", entry["exercise"], "--output", str(out)],
    )
    assert result.exit_code == 0, result.output
    assert out.exists() and out.stat().st_size > 0
    assert "visualization written" in result.output
