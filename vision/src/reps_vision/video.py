"""Count reps from any cv2-readable source: webcam index or video file.

The estimator and capture are injectable so the frame loop is testable
without MediaPipe or a camera.
"""

from __future__ import annotations

import os
from typing import Callable, Optional

from .detector import DetectorError, OnRep, RepCounter
from .exercises import Landmarks, get_spec
from .angles import RepStateMachine

# Called once per decoded frame: (frame, landmarks_or_None, angle_or_None, count).
# Fires even on frames with no pose so an output video keeps the full length.
# Return True to stop the run early.
FrameHook = Callable[[object, Optional[Landmarks], Optional[float], int], Optional[bool]]


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

    WINDOW = "reps-for-claude"

    def __init__(
        self,
        source: int | str,
        *,
        estimator_factory: Callable[[], object] | None = None,
        cap_factory: Callable[[object], object] | None = None,
        show: bool = False,
        annotate: bool = False,
    ) -> None:
        self._source = source
        self._estimator_factory = estimator_factory or _default_estimator_factory
        self._cap_factory = cap_factory or _default_cap_factory
        self._show = show
        # Draw the overlay onto each frame when displaying a window or when a
        # consumer (e.g. the file writer) wants annotated frames.
        self._annotate = annotate or show
        self.rep_timestamps_ms: list[float] = []

    def run(
        self, exercise: str, on_rep: OnRep, on_frame: FrameHook | None = None
    ) -> int:
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
        # Pace a file preview to its real frame rate; a live camera is already
        # real-time, so poll with the minimum 1ms delay.
        self._key_delay = 1
        if self._show and isinstance(self._source, str):
            self._key_delay = self._frame_delay_ms(cap)
        try:
            while True:
                ok, frame = cap.read()  # type: ignore[attr-defined]
                if not ok:
                    break  # end of file / camera gone
                landmarks = estimator.landmarks(frame)  # type: ignore[attr-defined]
                current = spec.angle_from(landmarks) if landmarks else None
                if current is not None and machine.update(current):
                    count += 1
                    self.rep_timestamps_ms.append(self._position_ms(cap))
                    on_rep(count)
                if self._annotate:
                    self._draw(frame, landmarks, spec, current, count)
                if self._show and self._display(frame):
                    break  # user pressed q
                if on_frame is not None and on_frame(frame, landmarks, current, count):
                    break
        finally:
            cap.release()  # type: ignore[attr-defined]
            if self._show:
                self._destroy_window()
            close = getattr(estimator, "close", None)
            if close:
                close()
        return count

    @staticmethod
    def _draw(frame, landmarks, spec, angle, count) -> None:
        from .visualize import draw_overlay

        draw_overlay(frame, landmarks, spec, angle, count)

    @staticmethod
    def _frame_delay_ms(cap) -> int:
        import cv2

        fps = cap.get(cv2.CAP_PROP_FPS) or 25.0
        return max(1, int(1000.0 / fps))

    def _display(self, frame) -> bool:
        """Show the frame in a window; True when the user pressed q."""
        import cv2

        self._quiet_qt_fonts()
        try:
            cv2.imshow(self.WINDOW, frame)
            return (cv2.waitKey(self._key_delay) & 0xFF) == ord("q")
        except cv2.error as e:
            print(f"warning: cannot open a display window ({e}); ", end="")
            print("continuing without live preview", flush=True)
            self._show = False
            return False

    @staticmethod
    def _quiet_qt_fonts() -> None:
        """Point Qt at system fonts so the highgui window stops warning."""
        if os.environ.get("QT_QPA_FONTDIR"):
            return
        for candidate in ("/usr/share/fonts", "/usr/local/share/fonts"):
            if os.path.isdir(candidate):
                os.environ["QT_QPA_FONTDIR"] = candidate
                break

    def _destroy_window(self) -> None:
        try:
            import cv2

            cv2.destroyWindow(self.WINDOW)
            cv2.waitKey(1)
        except Exception:
            pass

    @staticmethod
    def _position_ms(cap) -> float:
        try:
            import cv2

            return float(cap.get(cv2.CAP_PROP_POS_MSEC))
        except Exception:
            return 0.0
