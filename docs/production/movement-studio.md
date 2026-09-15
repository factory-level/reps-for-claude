# Movement Studio and production qualification

The cross-repo target and release gates live in
[FieldLab's design target](../../../usb-mcp-hub/docs/production/design-target.md).

1. Start the app or hub with the `reps_vision` plugin. Open the hub's
   `/studio.html` page (the home page links to it). Pair first on the LAN listener.
2. Select/register a camera using the hub's existing camera controls. Open its
   preview and record a set. Stop and review frames. Mark each completed rep.
3. Save reviewed labels and a recording-session ID. Clips from the same sitting
   share a session ID. Empty labels identify a reviewed negative example.
4. Choose squat, push-up, or curl as a starting template. These templates are
   unqualified defaults. Select calibration frames and describe the movement.
5. Optionally consent to cloud authoring. Only selected frames are uploaded.
   Review the suggestion; edit the declarative rules if needed. Save a candidate.
6. Record and label separate evaluation sessions. Select them and run evaluation.
   Review mismatches and collect more examples or save a revised candidate.
7. Activate a passing candidate. The next workout resolves that movement ID to
   the active version. In-progress workouts keep their current configuration.

Calibration recording identities are saved with each candidate. Renaming a
recording's session later will not make that calibration clip eligible for its
held-out evaluation. Candidates created before calibration snapshots existed
must be recreated and evaluated before activation. A different installed model
also requires evaluation against that model; imported metadata cannot override
the stored rules or model identity.

Existing shipped exercises remain available through their legacy presets until
a passing version is activated. No accuracy claim is made for those presets.
Custom movement IDs can also be prescribed by the desktop without adding code.
Create `routine.json` in the app data directory (by default
`~/.local/share/reps-for-claude`, or the directory selected by `REPS_APP_HOME`),
then restart the app. For example:

```json
{
  "lifts": [
    { "exercise": "squat", "sets": 3, "reps": 5 },
    { "exercise": "pushup", "sets": 3, "reps": 8 },
    { "exercise": "my-curl", "label": "My curl", "sets": 2, "reps": 8 }
  ]
}
```

The `exercise` value must match the Studio movement ID. A custom ID such as
`my-curl` requires an active, evaluated version; it has no legacy detector
fallback. Existing shipped IDs retain their legacy fallback until activation.
Omitting the local routine file uses the bundled routine. Invalid targets are
rejected. Routine editing currently uses this file; Studio edits the movement's
detection rules. Automatic exercise identification is not part of this release.

Cloud authoring requires `ANTHROPIC_API_KEY` on the hub. Set
`HUB_AUTHORING_MODEL` to the model available to your account; the existing
default is retained for compatibility. Manual configuration and all live
detection work without cloud authoring.

## Bundle

Run `bash scripts/bundle-hub.sh` after hub/plugin changes. It stages the hub,
Reps plugin sources, companion screens, pinned model, and bootstrap verifier.
The manifest records both hub artifacts and consumer artifact hashes, plus
whether the hub checkout was dirty. A dirty bundle is a development artifact.
Release builds use Tauri resources; debug builds use the sibling checkout.
`pnpm tauri build --bundles deb --ci -- --offline --locked` from `app/` now
restages clean resources automatically before building the Linux installer.
First installation still requires Node >=22.5, uv, and provisioning the locked
Python environment; subsequent workouts use local dependencies and weights.

`node scripts/e2e-latency.mjs --bundle --movement-contract` checks the versioned
detector-to-hub event path with the bundled video fixture. It seeds synthetic
activation state in a disposable test database and checks event ordering and
provenance. Passing this command is not movement accuracy qualification.

The [target-machine checklist](../../../usb-mcp-hub/docs/production/release-checklist.md)
remains required before calling the release production-qualified.
See [package-verification.md](package-verification.md) for the extracted-installer
checks and their limits. The installed executable's `--check-runtime` option
verifies resource lookup and hub startup without opening windows or a camera.
It uses temporary hub state and ephemeral ports, and can provision/reuse the
normal writable Python environment. Use `UV_OFFLINE=1` after provisioning to
check startup without dependency downloads.

An occupied hub port now causes a clear startup error; the app never terminates
an unrelated listener. Owned child groups are cleaned up on startup failure,
and the hub shuts down when the desktop's ownership pipe closes.

## Publication recovery

The desktop keeps publications awaiting hub acknowledgement in
`publications.sqlite` next to `reps.sqlite`. It opens this queue before connecting
to the hub and retries in order after outages and desktop restarts. Events,
commands, and results retain their UUIDs; a lost acknowledgement cannot create a
second hub history record or repeat a command notification.

The `publications` table records `sequence`, `attempts`, `retry_at` (Unix
milliseconds), and `last_error`. Failed messages stay queued, with retry delay
capped at 30 seconds. A persistent failure at the front holds later publications
to preserve ordering. Restore connectivity or fix the reported application
contract error; the publisher then drains automatically. Local queue capacity is
10,000 messages; storage/capacity failures report an error instead of silently
dropping an accepted publication. Include this database in desktop backups.

## Gym capture preparation

Use the [capture-session guide](capture-session.md) for the launch command,
first-session checklist, evaluation collection targets, and session notes.
The launcher supports `--check` without starting services or opening the camera.

Host inspection on 2026-09-15 UTC found a Logitech HD Pro Webcam C920,
`/dev/video0` and `/dev/video1`, and a desktop display on Linux Mint 22.3.
The device nodes are not proof of two cameras or of usable capture: verify
the selected capture interface in preview before recording. The default hub
database contained zero recordings and zero reviewed labels. No camera frames
were captured during this inspection, and cloud authoring was not configured.

For the next attended session:

1. Open Studio and select the C920 capture interface. Check placement, rotation,
   whole-body visibility, lighting, and actual frame rate. The desktop rig
   defaults to 180-degree rotation; confirm the Studio view matches the workout.
2. Record calibration examples for squat, push-up, and curl, review rep-completion
   frames, and save them under calibration session IDs. Save a candidate using
   each template and the reviewed examples.
3. Reserve separate recording sessions for evaluation. Each movement needs at
   least 200 reviewed reps across five held-out sessions and 30 minutes of
   reviewed negatives. Do not relabel calibration clips as held-out examples.
4. Evaluate before activation. Required scores are precision >=99%, recall
   >=98%, exact matched sets >=95%, and zero false reps on the negatives.
   Review failures before changing a candidate; keep evaluation evidence honest
   when examples become part of tuning.
5. Finish the linked target-machine checklist, including actual display latency,
   emergency escape, camera interruption, offline workout, and soak testing.

Pose inference now receives recording/capture timestamps rather than a synthetic
33 ms increment. This fixes timing consistency; it does not establish an accuracy
improvement without the held-out recordings above.
