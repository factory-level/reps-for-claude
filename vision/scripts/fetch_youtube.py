#!/usr/bin/env python3
"""Fetch one short real-world YouTube clip per exercise for detection debugging.

The tracked/Wikimedia fixtures (`fetch_fixtures.py`) don't cover every
exercise in the registry; this script fills the gaps with side-on,
single-person YouTube clips so each exercise can be run through the
detector and visually debugged.

No ffmpeg is available on this host, so downloads use a single-format
selector (no format merging, no `--download-sections`): `-f "b[ext=mp4]
[height<=720]/b[ext=mp4]/b"`. yt-dlp is invoked as a subprocess only — it is
not a project dependency.

Downloads land in `tests/fixtures/videos/youtube/`, which is gitignored;
only this script and the manifest logic are committed.

Usage: uv run python scripts/fetch_youtube.py [--only EXERCISE]
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from reps_vision.exercises import SPECS

FIXTURES = Path(__file__).parent.parent / "tests" / "fixtures" / "videos" / "youtube"
MANIFEST_NAME = "youtube_manifest.json"

CANDIDATES_PER_EXERCISE = 5
MIN_DURATION_S = 20
MAX_DURATION_S = 240
FORMAT_SELECTOR = "b[ext=mp4][height<=720]/b[ext=mp4]/b"

QUERIES: dict[str, str] = {
    "pushup": "pushup exercise side view single person multiple reps",
    "bench": "barbell bench press side view single person multiple reps",
    "overhead": "overhead press side view single person multiple reps",
    "curl": "bicep curl side view single person multiple reps",
    "pullup": "pullup exercise side view single person multiple reps",
    "row": "barbell row side view single person multiple reps",
    "squat": "barbell squat side view single person multiple reps",
    "jumprope": "jump rope side view single person multiple reps",
}

assert set(QUERIES) == set(SPECS) | {"jumprope"}, "QUERIES must cover every exercise"


def verify_clip(path: str | Path) -> bool:
    """True if `path` opens as a video and decodes at least 100 frames."""
    import cv2

    cap = cv2.VideoCapture(str(path))
    try:
        if not cap.isOpened():
            return False
        reported = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
        if reported >= 100:
            return True
        # Frame-count metadata is sometimes wrong/absent; count decoded frames.
        decoded = 0
        while decoded < 100:
            ok, _ = cap.read()
            if not ok:
                break
            decoded += 1
        return decoded >= 100
    finally:
        cap.release()


def write_manifest(directory: str | Path, entries: dict[str, dict]) -> Path:
    """Write `entries` (exercise -> {file, url, title}) to youtube_manifest.json."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / MANIFEST_NAME
    path.write_text(json.dumps(entries, indent=2, sort_keys=True) + "\n")
    return path


def load_manifest(directory: str | Path) -> dict[str, dict]:
    """Load youtube_manifest.json from `directory`, or {} if it isn't there."""
    path = Path(directory) / MANIFEST_NAME
    if not path.exists():
        return {}
    return json.loads(path.read_text())


def _search_candidates(query: str, count: int = CANDIDATES_PER_EXERCISE) -> list[dict]:
    """Cheap metadata-only search: ytsearchN:<query> via --flat-playlist."""
    cmd = [
        "yt-dlp",
        f"ytsearch{count}:{query}",
        "--dump-json",
        "--flat-playlist",
        "--no-warnings",
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, check=True)
    candidates = []
    for line in result.stdout.splitlines():
        line = line.strip()
        if line:
            candidates.append(json.loads(line))
    return candidates


def _candidate_url(candidate: dict) -> str:
    video_id = candidate.get("id")
    return (
        candidate.get("url")
        or candidate.get("webpage_url")
        or f"https://www.youtube.com/watch?v={video_id}"
    )


def _download(url: str, dest: Path) -> bool:
    """Single-format mp4 download with a duration filter; True on success."""
    cmd = [
        "yt-dlp",
        url,
        "-f",
        FORMAT_SELECTOR,
        "--match-filter",
        f"duration > {MIN_DURATION_S} & duration < {MAX_DURATION_S}",
        "-o",
        str(dest),
        "--no-playlist",
        "--no-warnings",
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.returncode == 0 and dest.exists()


def fetch_one(exercise: str, query: str, fixtures_dir: Path = FIXTURES) -> dict | None:
    """Try each search candidate in turn; return the manifest entry for the
    first that downloads and verifies, or None if all candidates fail."""
    dest = fixtures_dir / f"{exercise}.mp4"
    try:
        candidates = _search_candidates(query)
    except subprocess.CalledProcessError as e:
        print(f"  search failed: {e.stderr.strip()[-500:]}")
        return None

    for candidate in candidates:
        title = candidate.get("title", "")
        url = _candidate_url(candidate)
        print(f"  trying: {title!r} ({url})")
        if dest.exists():
            dest.unlink()
        ok = _download(url, dest)
        if ok and verify_clip(dest):
            return {"file": dest.name, "url": url, "title": title}
        if dest.exists():
            dest.unlink()
        print("    rejected (download/duration/frame-count check failed)")
    return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--only", help="fetch a single exercise instead of every QUERIES key"
    )
    args = parser.parse_args(argv)

    if args.only is not None and args.only not in QUERIES:
        known = ", ".join(sorted(QUERIES))
        parser.error(f"unknown exercise {args.only!r}; known: {known}")

    exercises = [args.only] if args.only else list(QUERIES)

    FIXTURES.mkdir(parents=True, exist_ok=True)
    manifest = load_manifest(FIXTURES)
    failures = []
    for exercise in exercises:
        query = QUERIES[exercise]
        print(f"fetching {exercise!r}: {query!r}")
        entry = fetch_one(exercise, query)
        if entry:
            manifest[exercise] = entry
            print(f"  -> {entry['file']} ({entry['title']!r})")
        else:
            failures.append(exercise)
            print(f"  FAILED: no candidate satisfied constraints for {exercise!r}")
        write_manifest(FIXTURES, manifest)  # persist progress after each exercise

    if failures:
        print(f"failed: {', '.join(failures)}")
    print(f"manifest: {FIXTURES / MANIFEST_NAME}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
