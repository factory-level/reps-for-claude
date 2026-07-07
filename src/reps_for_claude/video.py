"""Count reps from any cv2-readable source: webcam index or video file.

The estimator and capture are injectable so the frame loop is testable
without MediaPipe or a camera.
"""

from __future__ import annotations

from typing import Callable

from .detector import DetectorError, OnRep, RepCounter
from .exercises import get_spec
from .angles import RepStateMachine


def _default_estimator_factory():
    from .pose import PoseEstimator

    return PoseEstimator()


def _default_cap_factory(source):
    try:
        import cv2
    except ImportError as e:
        from .pose import INSTALL_HINT

        raise DetectorError(f"opencv not available — {INSTALL_HINT}") from e
    return cv2.VideoCapture(source)


class VideoRepCounter(RepCounter):
    """One rep counter for live cameras (int source) and files (str source)."""

    name = "video"

    def __init__(
        self,
        source: int | str,
        *,
        estimator_factory: Callable[[], object] | None = None,
        cap_factory: Callable[[object], object] | None = None,
        show: bool = False,
    ) -> None:
        self._source = source
        self._estimator_factory = estimator_factory or _default_estimator_factory
        self._cap_factory = cap_factory or _default_cap_factory
        self._show = show
        self.rep_timestamps_ms: list[float] = []

    def run(self, exercise: str, on_rep: OnRep) -> int:
        try:
            spec = get_spec(exercise)
        except KeyError as e:
            raise DetectorError(str(e)) from e

        cap = self._cap_factory(self._source)
        if not cap.isOpened():  # type: ignore[attr-defined]
            raise DetectorError(f"could not open video source {self._source!r}")
        estimator = self._estimator_factory()
        machine = RepStateMachine(spec.down_below, spec.up_above)
        self.rep_timestamps_ms = []
        count = 0
        try:
            while True:
                ok, frame = cap.read()  # type: ignore[attr-defined]
                if not ok:
                    break  # end of file / camera gone
                landmarks = estimator.landmarks(frame)  # type: ignore[attr-defined]
                if landmarks is None:
                    continue
                current = spec.angle_from(landmarks)
                if current is None:
                    continue
                if machine.update(current):
                    count += 1
                    self.rep_timestamps_ms.append(self._position_ms(cap))
                    on_rep(count)
                if self._show and self._preview(frame, exercise, count):
                    break
        finally:
            cap.release()  # type: ignore[attr-defined]
            close = getattr(estimator, "close", None)
            if close:
                close()
        return count

    @staticmethod
    def _position_ms(cap) -> float:
        try:
            import cv2

            return float(cap.get(cv2.CAP_PROP_POS_MSEC))
        except Exception:
            return 0.0

    def _preview(self, frame, exercise: str, count: int) -> bool:
        """Show a live preview; True when the user pressed q to stop."""
        import cv2

        cv2.putText(
            frame,
            f"{exercise}: {count}  (q to finish)",
            (10, 30),
            cv2.FONT_HERSHEY_SIMPLEX,
            1.0,
            (0, 255, 0),
            2,
        )
        cv2.imshow("reps-for-claude", frame)
        return (cv2.waitKey(1) & 0xFF) == ord("q")
