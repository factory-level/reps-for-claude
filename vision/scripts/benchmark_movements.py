#!/usr/bin/env python3
"""Offline public-video regression check for the production temporal detector.

No activation, camera access, cloud calls, or workout credit. Downloads are a
separate explicit step. Counts here are development evidence, not gym approval.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))
from reps_vision.movement import MovementActivity
from reps_vision.pose import PoseEstimator, ensure_model


def match_completions(detections, labels, tolerance_ms=500):
    """Ordered one-to-one temporal matching; equal totals are insufficient."""
    i = j = matched = 0
    while i < len(detections) and j < len(labels):
        if abs(detections[i] - labels[j]) <= tolerance_ms:
            matched += 1
            i += 1
            j += 1
        elif detections[i] < labels[j]:
            i += 1
        else:
            j += 1
    return {"truePositives": matched, "falsePositives": len(detections)-matched,
            "falseNegatives": len(labels)-matched}


def run_clip(entry, directory, model, trace_path=None):
    import cv2
    path = directory / entry["file"]
    if not path.is_file():
        raise ValueError(f"missing video: {path}; run scripts/fetch_fixtures.py")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != entry["sha256"]:
        raise ValueError(f"video checksum mismatch: {path}")
    templates = json.loads((ROOT / "src/reps_vision/movement_templates.json").read_text())
    config = templates[entry["exercise"]]
    activity = MovementActivity(config, target_reps=1_000_000)
    cap = cv2.VideoCapture(str(path))
    estimator = None
    frames, detections, durations, reasons, trace = 0, [], [], Counter(), []
    previous = -1.0
    start, end = entry.get("startMs", 0), entry.get("endMs", math.inf)
    try:
        if not cap.isOpened():
            raise ValueError(f"cannot decode {path}")
        estimator = PoseEstimator(model=model)
        while True:
            ok, frame = cap.read()
            if not ok:
                break
            ms = float(cap.get(cv2.CAP_PROP_POS_MSEC))
            if not math.isfinite(ms) or ms <= previous:
                raise ValueError(f"non-increasing source timestamps at frame {frames}: {ms}")
            previous = ms
            if ms < start:
                continue
            if ms >= end:
                break
            before = time.perf_counter()
            landmarks = estimator.landmarks(frame, timestamp_ms=ms)
            aspect = frame.shape[1] / frame.shape[0]
            corrected = {name: (p[0] * aspect, p[1], p[2]) for name, p in (landmarks or {}).items()}
            old_count = activity.count
            activity.update(corrected, ms / 1000)
            durations.append((time.perf_counter() - before) * 1000)
            if activity.count > old_count:
                detections.append(ms)
            reasons[activity.reason] += 1
            if trace_path:
                trace.append({"timestampMs": ms, "count": activity.count, **activity.diagnostics})
            frames += 1
    finally:
        cap.release()
        if estimator:
            estimator.close()
    if not frames:
        raise ValueError("no frames decoded in selected interval")
    if trace_path:
        trace_path.write_text("".join(json.dumps(row) + "\n" for row in trace))
    expected = entry["expectedReps"]
    labels = entry["completionTimesMs"]
    if len(labels) != expected or labels != sorted(set(labels)) or any(not start <= t < end for t in labels):
        raise ValueError("completion labels must be ordered, unique, in the selected interval and match expectedReps")
    score = match_completions(detections, labels)
    return {"id": entry["id"], "file": entry["file"], "sha256": digest,
            "exercise": entry["exercise"], "frames": frames, "expectedReps": expected,
            "detectedReps": activity.count, "completionTimesMs": detections,
            "passed": score["falsePositives"] == score["falseNegatives"] == 0, **score,
            "required": entry.get("required", True), "reasons": dict(reasons),
            "processingP95Ms": sorted(durations)[min(len(durations)-1, math.ceil(len(durations)*.95)-1)],
            "configSha256": hashlib.sha256(json.dumps(config, sort_keys=True).encode()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=ROOT / "tests/fixtures/videos/benchmark.json")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--traces", action="store_true")
    parser.add_argument("--model", type=Path, help="Explicit experimental model; never changes the installed runtime model")
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"schemaVersion": 1, "evidence": "public-video-regression", "productionQualified": False, "clips": []}
    try:
        model = args.model or ensure_model()
        report["modelSha256"] = hashlib.sha256(model.read_bytes()).hexdigest()
        for entry in json.loads(args.manifest.read_text())["clips"]:
            try:
                trace = args.output.parent / (entry["id"] + ".jsonl") if args.traces else None
                result = run_clip(entry, args.manifest.parent, model, trace)
            except Exception as error:
                result = {"id": entry["id"], "passed": False, "required": entry.get("required", True), "error": str(error)}
            report["clips"].append(result)
            print(json.dumps(result), flush=True)
        required = [c for c in report["clips"] if c["required"]]
        report["passed"] = bool(required) and all(c["passed"] for c in required)
        report["challengeFailures"] = [c["id"] for c in report["clips"] if not c["required"] and not c["passed"]]
    except Exception as error:
        report.update(passed=False, error=str(error))
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
