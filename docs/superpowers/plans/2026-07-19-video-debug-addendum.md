# Video-Debug Addendum (extends Milestone 1 plan)

> **For agentic workers:** executed via superpowers:subagent-driven-development, same conventions as `2026-07-19-tauri-foundation.md` (commit trailer, branch `tauri-foundation`).

**Goal (user-set):** get the Tauri app running and able to stream a video of each exercise through the detector for debugging. YouTube is an approved source for missing exercise clips.

**Ordering note:** Tasks A and B are Python-only and run while the webkit2gtk system libraries are pending. Foundation Tasks 10 + 11(step 8) + 12 and Task C run once webkit is installed.

## Task A: YouTube fixture fetcher

**Files:** Create `vision/scripts/fetch_youtube.py`, `vision/tests/test_fetch_youtube.py`; modify `.gitignore` (ignore `vision/tests/fixtures/videos/youtube/`).

**Interfaces:**
- `QUERIES: dict[str, str]` — one search phrase per exercise: every key of `reps_vision.exercises.SPECS` plus `"jumprope"`. Side-on/single-person phrasing (e.g. `"barbell squat side view single person"`).
- `verify_clip(path) -> bool` — cv2-opens and has ≥ 100 frames (importorskip-guarded in tests).
- `write_manifest(dir, entries)` / `load_manifest(dir)` — `youtube_manifest.json`: `{exercise: {"file", "url", "title"}}`.
- CLI: `uv run python scripts/fetch_youtube.py [--only EXERCISE]` — for each exercise, `yt-dlp "ytsearch5:<query>"` candidates; download first that satisfies: mp4 single-format ≤720p (`-f "b[ext=mp4][height<=720]/b[ext=mp4]/b"` — no ffmpeg on this host, so no merged formats), duration 20–240s (`--match-filter`), then `verify_clip`; on failure try next candidate. Save to `vision/tests/fixtures/videos/youtube/<exercise>.mp4`.

**Verify:** hermetic pytest for QUERIES completeness + manifest round-trip (no network in tests). Then actually run the fetcher for all exercises; report per-exercise url/title/frames.

## Task B: streaming analyze module (the debug stream)

**Files:** Create `vision/src/reps_vision/stream.py`, `vision/tests/test_stream.py`.

**Interfaces:**
- `run_stream(video: str, exercise: str, sink: Callable[[dict], None], *, estimator=None, jpeg_every: int = 2, jpeg_width: int = 640) -> int` — drives the existing `VideoRepCounter` (annotate=True, injectable estimator, existing pattern in `test_video.py`) for rep exercises; for `"jumprope"` runs the frame loop feeding `JumpRopeActivity` (60s target) with landmarks. Emits dict events to `sink`:
  - `{"event":"open","exercise":...,"fps":...,"frameCount":...}`
  - every frame: `{"event":"progress","frame":i,"value":n,"unit":"reps"|"seconds","satisfied":bool}`
  - every `jpeg_every`-th frame: `{"event":"frame","frame":i,"jpegB64": <annotated, downscaled to jpeg_width, quality 70>}`
  - `{"event":"done","total":n,"satisfied":bool}`
  Returns final count/seconds as int.
- CLI: `python -m reps_vision.stream --video PATH --exercise NAME [--jpeg-every N]` → one JSON object per line on stdout (this is the sidecar protocol the Tauri driver consumes in Task C).

**Verify:** hermetic tests with a fake estimator: protocol shape, jpeg cadence, done event, jumprope path. Then a real-mediapipe demo run over `squat_demo.webm` and each fetched YouTube clip; record per-clip rep counts in the report (this IS the detection-debug data).

## Task C: app debug streaming view (webkit-gated)

**Files:** Modify `app/src-tauri/src/lib.rs`; create `app/src/DebugPanel.tsx` + test.

**Interfaces:**
- Tauri commands: `debug_videos() -> Vec<{exercise, path}>` (reads `vision/tests/fixtures/videos/` + youtube manifest); `debug_stream_start(video: String, exercise: String)` — spawns `uv run python -m reps_vision.stream ...` (cwd `vision/`), forwards each stdout JSON line as Tauri event `"debug-stream"`; kills any prior child. `debug_stream_stop()`.
- React `DebugPanel`: exercise/video picker, `<img>` fed from `debug-stream` frame events, live progress + satisfied readout. vitest with canned events.

**Verify:** vitest; manual `npm run tauri dev` — pick each exercise, watch annotated stream + live counts.
