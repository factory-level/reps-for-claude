"""Full-rate capture loop: frames → landmarks → activity → emitted streams.

Camera-gating: the capture device is handed in already-open by the plugin's
``start_stream`` and released here when the loop stops — it is never open
while no stream runs.
"""

from __future__ import annotations

import threading
import time


class StreamLoop:
    def __init__(self, *, activity, spec, estimator, capture, emit, frame_times_ms=None, provenance=None):
        self._frame_times_ms = frame_times_ms
        self._provenance = provenance or {}
        self._activity = activity
        self._spec = spec
        self._estimator = estimator
        self._capture = capture
        self._emit = emit
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="reps-vision-stream", daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join()

    def _run(self) -> None:
        last_value = 0.0
        was_satisfied = False
        source_ended = False
        frame_index = 0
        try:
            while not self._stop.is_set():
                ok, frame = self._capture.read()
                if not ok:
                    source_ended = True
                    break
                frame_index += 1
                ts_us = time.time_ns() // 1000
                source_ms = (self._frame_times_ms[frame_index - 1] if self._frame_times_ms is not None
                             else getattr(self._capture, "time_ms", None))
                if source_ms is None:
                    source_ms = time.monotonic() * 1000
                landmarks = self._estimator.landmarks(frame, timestamp_ms=source_ms)
                angle = (
                    self._spec.angle_from(landmarks)
                    if (self._spec is not None and landmarks is not None)
                    else None
                )
                visibility = (
                    sum(point[2] for point in landmarks.values()) / len(landmarks)
                    if landmarks
                    else 0.0
                )
                self._emit(
                    "landmarks",
                    {
                        **self._provenance,
                        "frameIndex": frame_index,
                        "sourceTimeMs": source_ms,
                        "poseDetected": landmarks is not None,
                        "visibility": visibility,
                        "landmarks": landmarks or {},
                        "angle": angle,
                        "measuredJoints": list(self._spec.measured_joints(landmarks) or ())
                        if (self._spec is not None and landmarks is not None)
                        else [],
                        "tsUs": ts_us,
                    },
                )
                activity_landmarks = landmarks
                # Pose coordinates normalize x and y independently. Temporal
                # geometry needs isotropic coordinates; retain normalized points
                # for overlays and preserve legacy tuned angle behavior.
                if landmarks and hasattr(self._activity, "movement") and hasattr(frame, "shape"):
                    aspect = frame.shape[1] / frame.shape[0]
                    activity_landmarks = {name: (p[0] * aspect, p[1], p[2]) for name, p in landmarks.items()}
                progress = self._activity.update(activity_landmarks, source_ms / 1000)
                if progress.unit == "reps" and progress.value > last_value:
                    self._emit("event", {"type": "rep_completed", "count": int(progress.value), "frameIndex": frame_index, "sourceTimeMs": source_ms, **self._provenance})
                self._emit(
                    "progress",
                    {
                        "value": progress.value,
                        "unit": progress.unit,
                        "satisfied": progress.satisfied,
                        "diagnostics": getattr(self._activity, "diagnostics", None),
                        "frameIndex": frame_index,
                        "sourceTimeMs": source_ms,
                        **self._provenance,
                        "tsUs": ts_us,
                    },
                )
                last_value = progress.value
                if progress.satisfied and not was_satisfied:
                    self._emit("event", {"type": "target_reached", "value": progress.value, **self._provenance})
                was_satisfied = progress.satisfied
            if source_ended:
                # File finished or camera died — consumers must not wait
                # forever for frames that will never come.
                self._emit("event", {"type": "stream_ended", "finalValue": last_value, **self._provenance})
        except Exception as exc:
            self._emit("event", {"type": "detector_error", "message": str(exc), **self._provenance})
        finally:
            self._capture.release()
            close = getattr(self._estimator, "close", None)
            if close is not None:
                close()
