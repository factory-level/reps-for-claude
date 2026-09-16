import json
import math
from pathlib import Path

import pytest
from reps_vision.movement import MovementActivity, validate_movement
from reps_vision.hub_plugin.plugin import RepsVisionPlugin

TEMPLATES = json.loads((Path(__file__).parents[1] / 'src/reps_vision/movement_templates.json').read_text())


def pose(degrees, side='left', visibility=.95):
    r = math.radians(degrees)
    return {f'{side}_hip': (1., 0., visibility), f'{side}_knee': (0., 0., visibility),
            f'{side}_ankle': (math.cos(r), math.sin(r), visibility)}


def activity(**overrides):
    return MovementActivity({**TEMPLATES['squat'], 'smoothingMs': 0, **overrides}, 3)


def feed(a, degrees, start, duration=.2, side='left', visibility=.95):
    for i in range(round(duration/.02)):
        a.update(None if degrees is None else pose(degrees, side, visibility), start + i*.02)
    return start + duration


def test_full_start_flex_return_required_and_no_double_counts():
    a = activity()
    t = feed(a, 90, 0)  # starting at the bottom is not a complete rep
    t = feed(a, 175, t)
    assert a.count == 0
    t = feed(a, 90, t)
    t = feed(a, 175, t)
    assert a.count == 1
    feed(a, 175, t, 1)
    assert a.count == 1


def test_phase_dwell_rejects_single_frame_jitter():
    a = activity()
    t = feed(a, 175, 0)
    a.update(pose(90), t)
    feed(a, 175, t+.02, 1)
    assert a.count == 0


def test_tracking_loss_and_large_timestamp_gap_require_new_start():
    for missing_frames in (True, False):
        a = activity()
        t = feed(a, 175, 0)
        t = feed(a, 90, t)
        if missing_frames:
            t = feed(a, None, t, 1)
        else:
            t += 1
        t = feed(a, 175, t)
        assert a.count == 0
        t = feed(a, 90, t)
        feed(a, 175, t)
        assert a.count == 1


def test_side_does_not_switch_mid_cycle_when_other_side_becomes_more_visible():
    a = activity()
    t = feed(a, 175, 0)
    for i in range(20):
        a.update({**pose(175, visibility=.8), **pose(90, 'right')}, t+i*.02)
    assert a.side == 'left'
    assert a.next_phase == 1
    assert a.count == 0


def test_sustained_occlusion_can_reacquire_other_side_without_phantom_count():
    a = activity()
    t = feed(a, 175, 0)
    t = feed(a, 90, t)
    t = feed(a, None, t, 1)
    t = feed(a, 175, t, side='right')
    assert a.side == 'right'
    assert a.count == 0


def test_all_features_must_match_and_low_visibility_does_not_count():
    m = {**TEMPLATES['squat'], 'smoothingMs': 0}
    m['features'] = {**m['features'], 'elbow': ['shoulder', 'elbow', 'wrist']}
    a = MovementActivity(m, 1)
    feed(a, 175, 0)
    assert a.diagnostics['reason'] == 'insufficient_visibility'
    assert a.count == 0


def test_invalid_numeric_geometry_and_nonmonotonic_time_are_rejected():
    a = activity()
    a.update(pose(float('nan')), 0)
    assert a.diagnostics['reason'] == 'insufficient_visibility'
    with pytest.raises(ValueError, match='timestamps'):
        a.update(pose(175), 0)
    for field in ['trackingLossMs', 'smoothingMs', 'minVisibility']:
        with pytest.raises(ValueError):
            validate_movement({**TEMPLATES['squat'], field: float('nan')})


def test_plugin_accepts_temporal_config_without_legacy_exercise():
    p = RepsVisionPlugin()
    config = {'activity': 'lift', 'movement': TEMPLATES['squat'], 'targetReps': 3}
    assert p.configure(config)['movement'] == TEMPLATES['squat']
    assert isinstance(p._build_activity(), MovementActivity)
    assert set(p.describe_capabilities()['movementTemplates']) == {'squat', 'pushup', 'curl'}


def test_same_recording_has_identical_outcomes_independent_of_wall_clock_origin():
    counts = []
    for origin in [0, 900000]:
        a = activity()
        t = origin
        for degrees in [175, 90, 175, 90, 175]:
            t = feed(a, degrees, t)
        counts.append(a.count)
    assert counts == [2, 2]


def test_estimator_failure_releases_capture():
    class Capture:
        released = False
        def release(self):
            self.released = True
    capture = Capture()
    def broken():
        raise RuntimeError('model unavailable')
    plugin = RepsVisionPlugin(estimator_factory=broken, capture_factory=lambda _: capture)
    plugin.configure({'activity':'lift','movement':TEMPLATES['squat']})
    with pytest.raises(RuntimeError, match='model unavailable'):
        plugin.start_stream({}, lambda *_: None)
    assert capture.released


def test_pinned_model_rejects_a_corrupted_local_override(tmp_path, monkeypatch):
    from reps_vision.pose import ensure_model
    from reps_vision.detector import DetectorError
    path = tmp_path / 'model.task'
    path.write_bytes(b'not the pinned model')
    monkeypatch.setenv('REPS_POSE_MODEL', str(path))
    with pytest.raises(DetectorError, match='checksum mismatch'):
        ensure_model()


def test_completed_cycle_exposes_ordered_capture_phase_evidence():
    a = activity()
    t = feed(a, 175, 0)
    t = feed(a, 90, t, visibility=.8)
    feed(a, 175, t)
    evidence = a.completed_cycle
    assert evidence['cycleId'] == '1'
    assert [p['name'] for p in evidence['phases']] == ['start', 'flexed', 'start']
    assert evidence['visibility'] == .8
    times = [p['atMs'] for p in evidence['phases']]
    assert all(b > a for a, b in zip(times, times[1:]))
