"""Generate the occluded-front e2e fixture: copy a squat video with the
middle third of its timeline blacked out, so the pose (and thus landmark
visibility) disappears mid-set and best-view election must fail over to the
side camera. MJPG/AVI output — no ffmpeg needed, cv2 reads it back fine.

Usage: uv run python ../scripts/make_occluded_fixture.py SRC DST (from vision/)
"""

import sys

import cv2


def main(src: str, dst: str) -> None:
    capture = cv2.VideoCapture(src)
    if not capture.isOpened():
        raise SystemExit(f"cannot open {src}")
    fps = capture.get(cv2.CAP_PROP_FPS) or 30.0
    total = int(capture.get(cv2.CAP_PROP_FRAME_COUNT))
    writer = None
    index = 0
    while True:
        ok, frame = capture.read()
        if not ok:
            break
        if writer is None:
            height, width = frame.shape[:2]
            writer = cv2.VideoWriter(
                dst, cv2.VideoWriter_fourcc(*"MJPG"), fps, (width, height)
            )
        if total // 3 <= index < 2 * total // 3:
            frame[:] = 0  # occlusion window: camera sees nothing
        writer.write(frame)
        index += 1
    capture.release()
    if writer is None:
        raise SystemExit(f"no frames in {src}")
    writer.release()
    print(f"wrote {dst}: {index} frames, occluded [{total // 3}, {2 * total // 3})")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
