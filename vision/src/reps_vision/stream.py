"""JSON-lines streaming protocol: the sidecar the Tauri debug driver spawns.

Drives the existing detector (VideoRepCounter for rep exercises, a raw
frame loop over JumpRopeActivity for jumprope) over a video file, emitting
one JSON-serializable event dict per decoded frame to a sink. As a CLI
(`python -m reps_vision.stream`), each event is printed as one line of JSON
on stdout — the protocol a Rust/Tauri sidecar driver can read line-by-line.

Event shapes:
    {"event": "open", "exercise": ..., "fps": ..., "frameCount": ...}
    {"event": "progress", "frame": i, "value": n, "unit": "reps"|"seconds", "satisfied": bool}
    {"event": "frame", "frame": i, "jpegB64": "<base64 jpeg, every jpeg_every-th frame>"}
    {"event": "done", "total": n, "satisfied": bool}
"""

from __future__ import annotations

import argparse
import base64
import json
from typing import Callable

from .activities.jumprope import JumpRopeActivity
from .detector import DetectorError
from .exercises import get_spec
from .video import VideoRepCounter

Sink = Callable[[dict], None]

# Jump rope has no per-rep angle spec; it is timed against a fixed target,
# same as the live BreakActivity wiring (JumpRopeActivity default use).
JUMPROPE_TARGET_SECONDS = 60.0

# Fallback frame pacing when a container reports no/zero FPS.
DEFAULT_FPS = 30.0

JPEG_QUALITY = 70


def run_stream(
    video: str,
    exercise: str,
    sink: Sink,
    *,
    estimator: object | None = None,
    jpeg_every: int = 2,
    jpeg_width: int = 640,
) -> int:
    """Stream one video through the detector, emitting protocol events to `sink`.

    Rep exercises run through VideoRepCounter (annotate=True); "jumprope" runs
    its own frame loop feeding JumpRopeActivity. Returns the final count
    (reps, or whole seconds for jumprope) — the same value as the "done"
    event's "total".
    """
    if exercise != "jumprope":
        try:
            get_spec(exercise)
        except KeyError as e:
            raise DetectorError(str(e)) from e

    fps, frame_count = _probe(video)
    sink({"event": "open", "exercise": exercise, "fps": fps, "frameCount": frame_count})

    estimator_factory = (lambda: estimator) if estimator is not None else None
    if exercise == "jumprope":
        return _run_jumprope(
            video, sink, estimator_factory, fps, jpeg_every, jpeg_width
        )
    return _run_reps(
        video, exercise, sink, estimator_factory, jpeg_every, jpeg_width
    )


def _probe(video: str) -> tuple[float, int]:
    import cv2

    cap = cv2.VideoCapture(video)
    if not cap.isOpened():
        raise DetectorError(f"could not open video source {video!r}")
    try:
        fps = float(cap.get(cv2.CAP_PROP_FPS) or 0.0)
        frame_count = int(cap.get(cv2.CAP_PROP_FRAME_COUNT) or 0)
    finally:
        cap.release()
    return fps, frame_count


def _encode_jpeg(frame, width: int) -> str:
    import cv2

    h, w = frame.shape[:2]
    if width and w and width != w:
        frame = cv2.resize(frame, (width, max(1, round(h * width / w))))
    ok, buf = cv2.imencode(".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, JPEG_QUALITY])
    if not ok:
        raise DetectorError("failed to encode frame as JPEG")
    return base64.b64encode(buf.tobytes()).decode("ascii")


def _run_reps(
    video: str,
    exercise: str,
    sink: Sink,
    estimator_factory,
    jpeg_every: int,
    jpeg_width: int,
) -> int:
    kwargs = {} if estimator_factory is None else {"estimator_factory": estimator_factory}
    counter = VideoRepCounter(video, annotate=True, **kwargs)

    def on_frame(frame, landmarks, angle, count) -> bool:
        frame_idx = on_frame.next_idx
        sink(
            {
                "event": "progress",
                "frame": frame_idx,
                "value": count,
                "unit": "reps",
                "satisfied": False,
            }
        )
        if frame_idx % jpeg_every == 0:
            sink(
                {
                    "event": "frame",
                    "frame": frame_idx,
                    "jpegB64": _encode_jpeg(frame, jpeg_width),
                }
            )
        on_frame.next_idx += 1
        return False

    on_frame.next_idx = 0

    total = counter.run(exercise, lambda n: None, on_frame=on_frame)
    sink({"event": "done", "total": total, "satisfied": False})
    return total


def _default_pose_estimator():
    from .pose import PoseEstimator

    return PoseEstimator()


def _run_jumprope(
    video: str,
    sink: Sink,
    estimator_factory,
    fps: float,
    jpeg_every: int,
    jpeg_width: int,
) -> int:
    import cv2

    cap = cv2.VideoCapture(video)
    if not cap.isOpened():
        raise DetectorError(f"could not open video source {video!r}")
    estimator = (estimator_factory or _default_pose_estimator)()
    activity = JumpRopeActivity(JUMPROPE_TARGET_SECONDS)
    tick = 1.0 / (fps or DEFAULT_FPS)

    value, satisfied = 0.0, False
    frame_idx = 0
    try:
        while True:
            ok, frame = cap.read()
            if not ok:
                break
            landmarks = estimator.landmarks(frame)
            progress = activity.update(landmarks, frame_idx * tick)
            value, satisfied = progress.value, progress.satisfied
            sink(
                {
                    "event": "progress",
                    "frame": frame_idx,
                    "value": value,
                    "unit": progress.unit,
                    "satisfied": satisfied,
                }
            )
            if frame_idx % jpeg_every == 0:
                sink(
                    {
                        "event": "frame",
                        "frame": frame_idx,
                        "jpegB64": _encode_jpeg(frame, jpeg_width),
                    }
                )
            frame_idx += 1
    finally:
        cap.release()
        close = getattr(estimator, "close", None)
        if close:
            close()

    total = int(value)
    sink({"event": "done", "total": total, "satisfied": satisfied})
    return total


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m reps_vision.stream")
    parser.add_argument("--video", required=True, help="path to a video file")
    parser.add_argument("--exercise", required=True, help="exercise name, or 'jumprope'")
    parser.add_argument("--jpeg-every", type=int, default=2)
    parser.add_argument("--jpeg-width", type=int, default=640)
    args = parser.parse_args(argv)

    def sink(event: dict) -> None:
        print(json.dumps(event), flush=True)

    try:
        run_stream(
            args.video,
            args.exercise,
            sink,
            jpeg_every=args.jpeg_every,
            jpeg_width=args.jpeg_width,
        )
    except DetectorError as e:
        sink({"event": "error", "message": str(e)})
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
