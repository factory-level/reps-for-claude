# Repeatable software qualification

From the Reps repository, with the sibling hub checkout available:

```sh
# Provision once, using committed locks.
(cd vision && uv sync --frozen --extra cv)
(cd ../usb-mcp-hub/vision && uv sync --frozen)
(cd ../usb-mcp-hub && pnpm install --frozen-lockfile)
(cd app && npm ci)

# Explicit download of attributed, checksum-pinned public footage.
(cd vision && uv run --frozen --extra cv python scripts/fetch_fixtures.py --benchmark)
bash scripts/bundle-hub.sh
node scripts/qualify-software.mjs --output /tmp/reps-qualification --with-video \
  --with-bundle --python-env /path/to/provisioned/hub-python-environment
```

The runner uses offline dependency resolution, disposable hub/test state, and
bounded command timeouts. It writes `report.json`, `summary.md`, and one log per
check. Missing dependencies and failures stay visible; it continues independent
checks. It never starts camera capture, activates movements, or contacts a paid
provider. Commands require local socket access. Omit `--with-bundle` when the hub
Python environment has not been provisioned; the report records that omission.

Reports include revisions, dirty status, source fingerprints (including untracked
files), bundle/model hashes, commands, durations, and pending release gates.
An eight-hour physical soak and installed desktop UI behavior remain manual.

## Failure coverage

- Real SQLite `SQLITE_FULL` from page-limited temporary databases verifies that
  the hub preserves the original error, rejects publication without notification,
  retains earlier events, and recovers. Desktop outbox tests retain the committed
  head and recover after reopening. This does not fill the machine's disk and
  does not replace full-volume operating-system qualification.
- Delivery capacity exhaustion verifies atomic event/receipt/outbox rollback.
- The actual signed example receiver is killed and restarted after accepting a
  delivery whose acknowledgement was not committed by the hub. Lease recovery
  delivers the same ID again; the receiver retains exactly one inbox task.
- Desktop crash/reopen and retry checks exercise ordered publication persistence.
  Existing hub tests cover deduplication after lost acknowledgements. Receiver
  inbox acceptance is not evidence that an external agent completed its work.

## Public videos and model comparison

`vision/tests/fixtures/videos/benchmark.json` records source URLs, authors,
licenses, exact video hashes, intervals, and approximate completion-time labels.
Labels were reviewed visually by the coding agent, with 500 ms matching tolerance;
they are development evidence, not independent human-reviewed gym qualification.
The runner matches events one-to-one, so a false positive cannot hide a missed rep.

Sources: [FitnessScape squat](https://commons.wikimedia.org/wiki/File:Squat_-_exercise_demonstration_video.webm)
(CC BY 3.0), [Taco Fleur push-ups](https://commons.wikimedia.org/wiki/File:Interval_Push-ups.webm)
and [racked squats](https://commons.wikimedia.org/wiki/File:Kettlebell_Racked_Squats_(side_view).webm)
(CC BY-SA 4.0), and [Colossus Fitness curls](https://commons.wikimedia.org/wiki/File:Video_of_EZ_Bar_Curl_and_Straight_Bar_Curl.webm)
(CC BY 3.0). Source videos are unmodified; benchmark intervals select parts of them.
Downloaded footage is excluded from Git except the previously committed squat clip.

The development runtime now pins Google's existing
[MediaPipe Pose Landmarker Full](https://developers.google.com/edge/mediapipe/solutions/vision/pose_landmarker#models),
float16 version 1, SHA-256
`5134a3aad27a58b93da0088d431f366da362b44e3ccfbe3462b3827a839011b1`.
On the side-view squat footage, Lite counted equipment pickup and missed a later
rep; Full matched the six actual cycles. The push-up template now accepts a
150-degree starting angle instead of 155; the traced full-extension estimates
were often below 155. These templates still require calibration and evaluation.

The steady-camera curl interval is required regression evidence. A separate
camera-cut/cropped-shoulder interval remains exploratory (`required: false`);
its failures appear in `challengeFailures` and must not be interpreted as passing.
Changing camera viewpoints and missing required joints remain detection limits.
The short standing-negative clip does not meet the 30-minute negative gate.

To compare another already downloaded model without changing runtime configuration:

```sh
cd vision
uv run --frozen --extra cv python scripts/benchmark_movements.py \
  --model /path/to/model.task --output /tmp/comparison/report.json --traces
```

## Model compatibility and release gates

Existing movement versions remain immutable. Candidates pinned to Lite will fail
installed-model identity checks under the Full build: recreate candidates using
their reviewed calibration recordings, then evaluate before activating. No stored
candidate is silently rewritten or automatically promoted. Rollback to a Lite
candidate requires its matching application/model build as well as passing evidence.
API 1.5 and earlier frozen transcripts are unchanged.

CI now runs the public-video benchmark and uploads its report, includes the desktop Rust application, engine and hub client, installs
Linux desktop build libraries, and uses the committed Python lock with the CV
extra. Its Rust unit-test configuration omits staged bundle resources; packaged
runtime checks run separately. Hosted CI remains pending until these worktrees
are published and the jobs actually run.

## Latest local results — 2026-09-15

All nine requested checks passed in the qualification runner. Exact captured
results: [software report](software-results.json) and
[public-video report](public-video-results.json). Logs remain under
`/tmp/reps-software-verified`; source fingerprints describe the worktrees at the
start of that run. Subsequent documentation and cancellation-cleanup edits were
syntax/whitespace checked; the report is a snapshot, not a release signature.

| Required footage | Matched / reviewed completions | False reps |
| --- | ---: | ---: |
| Interval push-ups | 6 / 6 | 0 |
| Weighted squat demo | 2 / 2 | 0 |
| Racked squats, including equipment pickup | 6 / 6 | 0 |
| Steady-camera curl interval | 2 / 2 | 0 |
| Standing negative interval | 0 / 0 | 0 |

The separate camera-cut curl challenge matched **4 / 9**, with five missed reps
and zero extra reps. It is excluded from the fixed-camera regression gate and
explicitly listed in the report. This small, development-selected sample does
not establish general accuracy or production promotion eligibility.

The final staged bundle check processed 198 frames and two committed reps at
39.8 ms p95 frame transport latency, with clean parent-pipe shutdown. Timing
varied between runs under other machine activity; public-video processing p95
is reported separately and is not a release latency guarantee. The extracted
installer also passed native startup and offline video checks; see
[package verification](package-verification.md).
