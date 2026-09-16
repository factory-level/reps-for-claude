# Adding and tuning movements

Use [Movement Studio](../../production/movement-studio.md) to record examples,
review completion labels, create a candidate, evaluate separate held-out sessions
and activate a passing version. Squat, push-up and curl templates are starting
points and are not automatically qualified. Optional AI proposals require consent;
manual editing and local detection do not need cloud authoring.

## Too sensitive

- **Legacy lift:** lower `downBelow` or raise `upAbove` to require more movement.
  Keep both thresholds reachable in your actual camera view.
- **Versioned lift:** revise phase angles, dwell or smoothing in a new candidate,
  then evaluate and activate it. Live tuning of a pinned version is rejected.
- **Jump rope:** a larger `bounceThreshold` rejects smaller hip movements.
  The desktop now ships 0.008. Generic plugin controls may initially show 0.015;
  inspect the enabled metric's effective configuration before tuning.

Legacy controls can change an unversioned metric while it runs, but do not rewrite
the desktop's embedded defaults. Persist intended preset changes in
`app/src-tauri/resources/exercise_specs.json` and rebuild. The Python standalone
registry is separate. See [configuration precedence](../../architecture/detection.md#configuration-precedence).

For custom movements, use the Studio movement ID in the local `routine.json`.
A custom ID requires an active version. Versioned workouts currently own one
camera stream; legacy multi-camera fusion is a separate path, not qualification
for a versioned movement.

Test real reps, partial reps, stillness and interruptions. Keep calibration and
evaluation sessions separate. See [capture preparation](../../production/capture-session.md)
and the [qualification requirements](../../../../usb-mcp-hub/docs/design/production-target.md).
