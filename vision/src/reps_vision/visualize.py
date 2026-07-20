"""Draw detection overlays onto video frames: skeleton, tracked joint, reps.

Pure drawing over a BGR numpy frame using OpenCV. Kept separate from the
counting loop so the counter stays testable and headless; consumers feed the
per-frame output of VideoRepCounter.run(on_frame=...) through draw_overlay.
"""

from __future__ import annotations

from .exercises import ExerciseSpec, Landmarks

# A stick-figure subset of the 33-point pose topology — enough for the
# exercises we track without cluttering the frame with hands/face points.
POSE_CONNECTIONS: list[tuple[str, str]] = [
    ("left_shoulder", "right_shoulder"),
    ("left_shoulder", "left_elbow"),
    ("left_elbow", "left_wrist"),
    ("right_shoulder", "right_elbow"),
    ("right_elbow", "right_wrist"),
    ("left_shoulder", "left_hip"),
    ("right_shoulder", "right_hip"),
    ("left_hip", "right_hip"),
    ("left_hip", "left_knee"),
    ("left_knee", "left_ankle"),
    ("right_hip", "right_knee"),
    ("right_knee", "right_ankle"),
]

WHITE = (255, 255, 255)
DIM = (120, 120, 120)
JOINT = (0, 200, 255)      # amber landmark dots
TRACK = (0, 255, 0)        # green: the joint being measured
HUD_BG = (0, 0, 0)


def draw_overlay(
    frame,
    landmarks: Landmarks | None,
    spec: ExerciseSpec,
    angle: float | None,
    count: int,
    *,
    min_visibility: float = 0.5,
) -> None:
    """Annotate `frame` in place with the pose and the counter's state."""
    import cv2

    h, w = frame.shape[:2]

    def px(name: str) -> tuple[int, int] | None:
        p = landmarks.get(name) if landmarks else None
        if p is None or p[2] < min_visibility:
            return None
        return (int(p[0] * w), int(p[1] * h))

    measured = spec.measured_joints(landmarks) if landmarks else None
    tracked = set(measured) if measured else set()

    if landmarks:
        for a, b in POSE_CONNECTIONS:
            pa, pb = px(a), px(b)
            if pa and pb:
                on_track = a in tracked and b in tracked
                cv2.line(frame, pa, pb, TRACK if on_track else WHITE,
                         4 if on_track else 2)
        for name, p in landmarks.items():
            if p[2] < min_visibility:
                continue
            point = (int(p[0] * w), int(p[1] * h))
            if name in tracked:
                cv2.circle(frame, point, 8, TRACK, -1)
            else:
                cv2.circle(frame, point, 4, JOINT, -1)

    _draw_hud(frame, spec, angle, count, tracked_ok=bool(tracked))


def _draw_hud(frame, spec: ExerciseSpec, angle, count, *, tracked_ok: bool) -> None:
    import cv2

    lines = [f"{spec.name.upper()}   reps: {count}"]
    if angle is None:
        lines.append("no joint visible" if not tracked_ok else "angle: --")
        state = "--"
    else:
        lines.append(f"angle: {angle:5.1f} deg")
        state = "DOWN" if angle < spec.down_below else (
            "UP" if angle > spec.up_above else "mid"
        )
    lines.append(f"state: {state}")

    font, scale, thick = cv2.FONT_HERSHEY_SIMPLEX, 0.7, 2
    box_w = max(cv2.getTextSize(t, font, scale, thick)[0][0] for t in lines) + 20
    box_h = 30 * len(lines) + 10
    overlay = frame.copy()
    cv2.rectangle(overlay, (10, 10), (10 + box_w, 10 + box_h), HUD_BG, -1)
    cv2.addWeighted(overlay, 0.45, frame, 0.55, 0, frame)
    for i, text in enumerate(lines):
        color = TRACK if (i == 2 and state == "DOWN") else WHITE
        cv2.putText(frame, text, (20, 40 + i * 30), font, scale, color, thick,
                    cv2.LINE_AA)
