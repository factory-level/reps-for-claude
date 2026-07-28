"""Layer C (config-correctness), reps half: the SAME authored-config fixture the
hub's annotation-usefulness.test.ts asserts on must count the labeled reps
correctly. Proves: spoken annotation -> config -> correct rep count. Model-free
(drives the rep state machine directly over a labeled angle sequence, the exact
math the plugin uses); the real-plugin path is exercised by demo-record-replay.
"""
import json
import os

import pytest

from reps_vision.angles import RepStateMachine

# The single source of truth is the hub's proposal fixture (sibling repo).
PROPOSAL = os.path.join(
    os.path.dirname(__file__),
    "..", "..", "..",
    "usb-mcp-hub", "apps", "hubd", "test", "annotation", "proposals", "squat.json",
)
GOLDEN = os.path.join(
    os.path.dirname(__file__),
    "..", "..", "..",
    "usb-mcp-hub", "apps", "hubd", "test", "annotation", "squat-bottom.json",
)


def _labeled_squats(n, down_angle=100.0, up_angle=175.0):
    """A labeled sequence of exactly n squats: each rep dips below the down
    threshold then returns above the up threshold, with in-between jitter that
    must NOT be counted."""
    seq = [up_angle]
    for _ in range(n):
        seq += [up_angle, 140.0, down_angle, 130.0, up_angle]  # jitter around the rep
    return seq


@pytest.mark.skipif(not os.path.exists(PROPOSAL), reason="hub fixture not present (sibling repo)")
def test_authored_squat_config_counts_the_labeled_reps():
    proposal = json.load(open(PROPOSAL))
    golden = json.load(open(GOLDEN))
    exercise = proposal["config"]["exercise"]
    machine = RepStateMachine(down_below=exercise["downBelow"], up_above=exercise["upAbove"])

    expected = golden["expectRepCount"]
    reps = sum(1 for angle in _labeled_squats(expected) if machine.update(angle))

    assert reps == expected


def test_wrong_config_miscounts_the_same_reps():
    """Guard so the assertion above isn't vacuous: a down threshold the movement
    never reaches (90°, below the 100° bottom) counts zero reps — the config
    genuinely determines the count."""
    machine = RepStateMachine(down_below=90.0, up_above=160.0)
    reps = sum(1 for angle in _labeled_squats(10) if machine.update(angle))
    assert reps == 0
