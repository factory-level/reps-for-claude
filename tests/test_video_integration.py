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
    from reps_for_claude.video import VideoRepCounter

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


def test_analyze_does_not_touch_ledger(tmp_path, monkeypatch):
    """`reps analyze` must never credit the ledger, even on a counted video."""
    manifest = load_manifest()
    pytest.importorskip("mediapipe")
    entry = manifest[0]
    clip = FIXTURES / entry["file"]
    if not clip.exists():
        pytest.skip(f"{entry['file']} missing")

    from typer.testing import CliRunner

    from reps_for_claude.cli import app

    home = tmp_path / "reps-home"
    home.mkdir()
    monkeypatch.setenv("REPS_HOME", str(home))
    runner = CliRunner()
    result = runner.invoke(
        app, ["analyze", str(clip), "--exercise", entry["exercise"]]
    )
    assert result.exit_code == 0
    assert "no credit banked" in result.output
    assert not (home / "state" / "state.json").exists()
