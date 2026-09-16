import importlib.util
from pathlib import Path
import pytest
spec = importlib.util.spec_from_file_location('timing', Path(__file__).parents[2] / 'scripts/calibrate-camera-timing.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def rows(delay=100, period=40):
    return [{'sourceTimeMs': i * 1000 + delay, 'receivedAtMs': i * 1000 + 10000 + delay,
             'referenceTimeMs': i * 1000 + 10000, 'framePeriodMs': period} for i in range(5)]


def test_calibration_uses_measured_delay_and_short_expiry():
    result = module.calibrate({'usb': rows(30), 'phone': rows(140)}, 15000)
    assert result['phone']['delayMs'] == 140
    assert result['phone']['uncertaintyMs'] == 60
    assert result['phone']['validUntilMs'] == 915000
    assert result['phone']['offsetMs'] == 9860


def test_insufficient_or_slow_sync_evidence_cannot_qualify():
    with pytest.raises(ValueError, match='five'):
        module.calibrate({'usb': rows()[:1], 'phone': rows()}, 0)
    with pytest.raises(ValueError, match='exceeds'):
        module.calibrate({'usb': rows(), 'phone': rows(period=200)}, 0)
