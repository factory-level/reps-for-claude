# Reps design

The current cross-repo acceptance requirements are the hub's
[production target](../../../usb-mcp-hub/docs/design/production-target.md).
Reps owns workouts, desktop behavior, pose features, and movement rules. The hub
owns recording, immutable versions, evaluation, activation, and event delivery.

Supporting design history:

- [Hub as SDK](../superpowers/specs/2026-07-21-hub-as-sdk-design.md).
- [Tauri rewrite](../superpowers/specs/2026-07-19-tauri-rewrite-design.md).

## Sensitivity intent — 2026-09-15

Exercise reps and jump-rope timing should ignore small incidental movements.
Legacy presets require a wider joint-angle excursion for the six affected lifts
and a larger frame-to-frame hip displacement for jump rope. These defaults are
starting points for camera calibration. Activated movement versions remain
immutable and require a new evaluated candidate to change sensitivity.

[As-built detection](../architecture/detection.md) records exact settings,
configuration precedence, verification, and limitations. [Architecture coverage](../architecture/index.md)
connects the broader production target to implementation and remaining gates.
