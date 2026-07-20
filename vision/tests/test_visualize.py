"""Overlay drawing tests — draw onto real numpy frames, assert pixels change."""

import pytest

from reps_vision.exercises import get_spec
from reps_vision.visualize import POSE_CONNECTIONS, draw_overlay

np = pytest.importorskip("numpy")
pytest.importorskip("cv2")


def blank(h=240, w=320):
    return np.zeros((h, w, 3), dtype=np.uint8)


def arm(kind="down", vis=0.9):
    wrist = (0.6, 0.5) if kind == "up" else (0.5, 0.7)
    return {
        "left_shoulder": (0.5, 0.3, vis),
        "left_elbow": (0.5, 0.5, vis),
        "left_wrist": (*wrist, vis),
        "left_hip": (0.5, 0.7, vis),
        "left_knee": (0.5, 0.85, vis),
        "left_ankle": (0.5, 0.95, vis),
    }


class TestConnections:
    def test_connection_endpoints_are_real_landmarks(self):
        names = {n for pair in POSE_CONNECTIONS for n in pair}
        # every referenced landmark is a left/right variant we produce
        for n in names:
            assert n.startswith(("left_", "right_"))


class TestDrawOverlay:
    def test_draws_something_when_pose_present(self):
        frame = blank()
        draw_overlay(frame, arm("down"), get_spec("pushup"), 80.0, 3)
        assert frame.any()  # non-black pixels were drawn

    def test_hud_present_even_without_pose(self):
        frame = blank()
        draw_overlay(frame, None, get_spec("pushup"), None, 0)
        # HUD box lives in the top-left corner
        assert frame[:120, :200].any()

    def test_does_not_change_frame_dimensions(self):
        frame = blank(200, 350)
        draw_overlay(frame, arm(), get_spec("squat"), 120.0, 1)
        assert frame.shape == (200, 350, 3)

    def test_low_visibility_landmarks_not_drawn(self):
        clear = blank()
        draw_overlay(clear, arm("down", vis=0.9), get_spec("pushup"), 80.0, 1)
        faint = blank()
        draw_overlay(faint, arm("down", vis=0.1), get_spec("pushup"), None, 1)
        # the faint frame should have fewer lit pixels (only the HUD)
        assert int(faint.astype(bool).sum()) < int(clear.astype(bool).sum())

    def test_tracked_joint_uses_green(self):
        frame = blank()
        draw_overlay(frame, arm("down"), get_spec("pushup"), 80.0, 2)
        # green channel dominant somewhere (tracked elbow/shoulder/wrist)
        green = (frame[:, :, 1] > 150) & (frame[:, :, 2] < 100) & (frame[:, :, 0] < 100)
        assert green.any()
