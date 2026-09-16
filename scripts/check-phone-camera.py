#!/usr/bin/env python3
"""Concurrent USB + phone smoke test. No video is saved; credentials are not printed."""
import argparse
import json
import os
from pathlib import Path
import sys
import time
os.environ.setdefault('OPENCV_LOG_LEVEL', 'SILENT')
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'vision/src'))
from reps_vision.hub_plugin.plugin import _cv2_capture

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--phone-url', default='rtsp://192.168.1.253:8554/live')
parser.add_argument('--phone-url-file', type=Path)
parser.add_argument('--usb', default='/dev/video0')
parser.add_argument('--seconds', type=float, default=10)
args = parser.parse_args()
url = args.phone_url_file.read_text().strip() if args.phone_url_file else args.phone_url
captures = {}
try:
    for name, value in [('usb', args.usb), ('phone', url)]:
        captures[name] = _cv2_capture({'source': 'uri', 'value': value})
    start = time.monotonic()
    counts = {name: 0 for name in captures}
    age = {}
    while time.monotonic() - start < args.seconds:
        for name, capture in captures.items():
            ok, frame = capture.read()
            if ok:
                counts[name] += 1
                age[name] = round(time.monotonic() * 1000 - capture.time_ms, 1)
    print(json.dumps({name: {'frames': counts[name], 'fps': round(counts[name]/(time.monotonic()-start), 1),
        'lastFrameAgeMs': age.get(name), 'reconnected': cap.reconnected} for name, cap in captures.items()}, indent=2))
    if not all(counts.values()):
        raise SystemExit('FAIL: both cameras must deliver frames')
finally:
    for capture in captures.values():
        capture.release()
