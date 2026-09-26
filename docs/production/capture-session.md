# Gym capture session

## Launch when ready

From `reps-for-prompts/`:

```sh
bash scripts/capture-studio.sh --check
bash scripts/capture-studio.sh
```

After startup, open **http://127.0.0.1:8087/studio.html** on this machine.
The launcher registers the C920 at `/dev/video0`. Recording starts when you
press **Record set** in Studio. The local hub uses a dedicated persistent folder:
`~/.local/share/reps-gym-capture`. Keep this folder: it contains recordings,
labels, and evaluation evidence. This is separate from the desktop's usual hub
workspace; candidates here are not automatically activated in that workspace.

First launch can download locked Python dependencies. A pose model may be
provisioned when inference is first used. Leave the terminal open while using
Studio. Stop recording, close the preview, then press **Ctrl+C** to stop the hub.
Close the workout app before capture so it releases the camera.

The rig defaults to 180-degree rotation. If preview is upside down, stop the hub
and relaunch with the appropriate rotation (0, 90, 180, or 270):

```sh
REPS_CAPTURE_ROTATE=0 bash scripts/capture-studio.sh
```

Use `REPS_CAPTURE_DEVICE` to select another capture interface and
`REPS_CAPTURE_DATA_DIR` to choose another persistent folder. If a port is occupied,
stop your other capture instance or set `REPS_CAPTURE_PORT` and
`REPS_CAPTURE_STUDIO_PORT` to unused ports. The launcher does not terminate port
owners. The `--check` command checks installed prerequisites, not camera access
or port availability.

## First session: teach the movements

- [ ] Select **gym-c920** and open camera preview. Confirm orientation and that
  all required joints remain visible through the full movement.
- [ ] Record placement, resolution/frame rate, lighting, and clothing below.
- [ ] Record one short set each of squat, push-up, and curl. Keep each set in its
  own recording. For this setup session, 5–10 deliberate reps per set is enough
  to inspect the view and labeling workflow; this is not qualification data.
- [ ] Stop each recording and review the entire clip. Mark the frame where each
  full rep completes. Incomplete attempts get no completion mark.
- [ ] Set **Use this recording for → Teaching / calibration**. Use one session ID
  for clips recorded together, such as `2026-09-16-am-calibration`.
- [ ] Check the reviewed-label confirmation and save labels.
- [ ] Choose the matching template, describe what counts, review/edit the rules,
  and save a candidate. AI proposals are optional; they require configured
  credentials and explicit consent to send selected example frames.

## Later sessions: measure accuracy

Keep evaluation recordings separate from teaching examples. Do not rename clips
from the same sitting to manufacture independent sessions. Each movement needs:

| Evidence | Minimum / gate |
| --- | --- |
| Reviewed complete repetitions | 200 across at least five held-out sessions |
| Reviewed negatives | 30 minutes with zero false reps |
| Precision / recall | 99% / 98% |
| Sets with exact matching completions | 95% |
| Recording continuity | At least 10 fps; no gap over 500 ms |

Record one set per clip and assign **Held-out evaluation**. Include ordinary
variation in clothing and lighting. Negative clips should include idle time,
partial reps, unrelated activity, entering/leaving frame, and occlusion. Review
all negative clips and save empty completion labels only when no full rep occurs.
Spread the collection across later sessions; do not rush it into the setup visit.

Select a candidate and held-out recordings, then run evaluation. Inspect missed
and false detections. If you use failures to tune rules, collect fresh held-out
examples for the revised candidate. Activate only after the gates pass.

## Session notes — copy for each visit

- Date/time and recording-session ID:
- Purpose: calibration / held-out evaluation
- Camera interface, orientation, placement and distance:
- Resolution and observed frame rate:
- Lighting / clothing / obstructions:
- Movement and recording IDs:
- Reviewed complete-rep counts:
- Negative duration and activities:
- Ambiguous frames or corrected labels:
- Candidate version evaluated, result and follow-up:

Accuracy evaluation does not finish desktop release qualification. Complete the
[release checklist](../../../usb-mcp-hub/docs/production/release-checklist.md)
for display latency, escape, interruptions, delivery recovery, offline workouts,
and the eight-hour soak.
