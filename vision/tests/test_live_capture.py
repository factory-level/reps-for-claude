import threading
import time
from reps_vision.hub_plugin.live_capture import LiveCapture


def test_latest_frame_is_bounded_and_stop_releases_reader():
    class Device:
        def __init__(self):
            self.i = 0
            self.released = False
        def read(self):
            time.sleep(.002)
            self.i += 1
            return True, self.i
        def release(self):
            self.released = True
    device = Device()
    capture = LiveCapture(lambda: device)
    try:
        time.sleep(.05)
        ok, frame = capture.read()
        assert ok and frame > 1
        assert capture.time_ms > 0
    finally:
        capture.release()
    assert device.released and not capture._thread.is_alive()


def test_reconnect_changes_epoch_and_invalidates_timing():
    opened = threading.Event()
    class Device:
        def read(self):
            return False, None
        def release(self):
            pass
    def factory():
        opened.set()
        return Device()
    capture = LiveCapture(factory)
    try:
        deadline = time.monotonic() + 1
        while not capture.reconnected and time.monotonic() < deadline:
            time.sleep(.005)
        assert capture.reconnected
    finally:
        capture.release()
