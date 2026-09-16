#!/usr/bin/env python3
"""Derive a conservative short-lived clock mapping from reviewed sync frames.

Input JSON: {cameraId: [{sourceTimeMs, receivedAtMs, referenceTimeMs, framePeriodMs}, ...]}
referenceTimeMs is a known visible stimulus time on the host's epoch clock;
sourceTimeMs/receivedAtMs come from the corresponding camera frame metadata.
Five or more reviewed stimuli per view are required. Outputs contain no URLs.
"""
import argparse
import json
import math
import statistics
import time
from pathlib import Path


def calibrate(samples, now_ms):
    result = {}
    if len(samples) < 2:
        raise ValueError("at least two cameras required")
    for camera, rows in samples.items():
        if len(rows) < 5:
            raise ValueError(f"{camera}: at least five reviewed sync frames required")
        for row in rows:
            if any(not isinstance(row.get(key), (int, float)) or not math.isfinite(row[key]) for key in ("sourceTimeMs", "receivedAtMs", "referenceTimeMs", "framePeriodMs")):
                raise ValueError("invalid sync-frame timestamps")
            if row['framePeriodMs'] <= 0 or row['receivedAtMs'] < row['referenceTimeMs']:
                raise ValueError("invalid frame period or negative capture delay")
        delays = [r['receivedAtMs'] - r['referenceTimeMs'] for r in rows]
        delay = statistics.median(delays)
        # Include one whole frame period plus host display scheduling margin.
        uncertainty = max(abs(d - delay) for d in delays) + max(r['framePeriodMs'] for r in rows) + 20
        if uncertainty > 100:
            raise ValueError(f"{camera}: uncertainty {uncertainty:.1f}ms exceeds 100ms; improve frame rate/network and repeat")
        result[camera] = {'delayMs': delay, 'uncertaintyMs': uncertainty,
            'offsetMs': statistics.median(r['referenceTimeMs'] - r['sourceTimeMs'] for r in rows),
            'validUntilMs': now_ms + 15 * 60 * 1000,
            'reviewedSamples': len(rows), 'calibratedAtMs': now_ms}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('samples', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = calibrate(json.loads(args.samples.read_text()), time.time() * 1000)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print('Wrote measured timing calibration for ' + ', '.join(result))
