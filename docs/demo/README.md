# Detection demo

`squat_demo_detected.mp4` — the pose detector run over
`tests/fixtures/videos/squat_demo.webm` with the visualization overlay
(skeleton, tracked joint in green, live angle + rep count HUD). Generated with:

```sh
uv run reps analyze tests/fixtures/videos/squat_demo.webm -e squat \
  -o docs/demo/squat_demo_detected.mp4
```

Source clip: *"Squat - exercise demonstration video"*, Everkinetic, CC BY 3.0,
via Wikimedia Commons.

## Watch detection live

To pop up a real-time window instead of writing a file (needs a display and
the `cv` extra), add `--show`:

```sh
uv run reps analyze tests/fixtures/videos/squat_demo.webm -e squat --show
```

Press `q` to stop. Live webcam earning (`reps earn <exercise>` with
`detector.name = "mediapipe"`) shows the same window.
