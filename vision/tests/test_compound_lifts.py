"""Layer C for the big-6 compound lifts (model-free): each lift's shipped
thresholds in exercise_specs.json must count clean reps, and an unreachable
threshold must count zero (so the assertion isn't vacuous). Drives the same
RepStateMachine the plugin uses, over labeled angle sequences — no MediaPipe in
CI. On-camera tuning (tune-lift.ts) sets the real thresholds; this guards that
the config shape stays self-consistent for every lift, including deadlift.
"""
import json
import os

import pytest

from reps_vision.angles import RepStateMachine

SPECS = os.path.join(
    os.path.dirname(__file__),
    "..", "..",
    "app", "src-tauri", "resources", "exercise_specs.json",
)
BIG6 = ["squat", "deadlift", "bench", "overhead", "row", "pullup"]


def _lift(name):
    specs = json.load(open(SPECS))
    ex = specs["exercises"][name]
    assert ex["activity"] == "lift", name
    e = ex["exercise"]
    assert len(e["joints"]) == 3, f"{name} joints must be a triple"
    assert e["downBelow"] < e["upAbove"], f"{name} downBelow must be < upAbove"
    return e


def _labeled_reps(n, down, up):
    """n clean reps: rest above `up`, dip below `down`, return, with in-between
    jitter that must not double-count."""
    lo, hi, mid = down - 15, up + 10, (down + up) / 2
    seq = [hi]
    for _ in range(n):
        seq += [hi, mid, lo, mid, hi]
    return seq


@pytest.mark.parametrize("name", BIG6)
def test_shipped_thresholds_count_clean_reps(name):
    e = _lift(name)
    machine = RepStateMachine(down_below=e["downBelow"], up_above=e["upAbove"])
    reps = sum(1 for a in _labeled_reps(7, e["downBelow"], e["upAbove"]) if machine.update(a))
    assert reps == 7


@pytest.mark.parametrize("name", BIG6)
def test_unreachable_threshold_counts_zero(name):
    e = _lift(name)
    # a down threshold 40deg below the movement's lowest point is never crossed
    machine = RepStateMachine(down_below=e["downBelow"] - 40, up_above=e["upAbove"])
    reps = sum(1 for a in _labeled_reps(7, e["downBelow"], e["upAbove"]) if machine.update(a))
    assert reps == 0


def test_big6_all_present_and_lifts():
    specs = json.load(open(SPECS))
    for name in BIG6:
        assert name in specs["exercises"], f"{name} missing from exercise_specs.json"
        _lift(name)
