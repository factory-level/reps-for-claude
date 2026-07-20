"""Pure joint-angle geometry and the rep-counting state machine. No IO."""

from __future__ import annotations

import math

Point = tuple[float, float]


def angle(a: Point, b: Point, c: Point) -> float:
    """Angle in degrees at joint `b` formed by segments b->a and b->c (0..180)."""
    v1 = (a[0] - b[0], a[1] - b[1])
    v2 = (c[0] - b[0], c[1] - b[1])
    if (v1[0] == 0 and v1[1] == 0) or (v2[0] == 0 and v2[1] == 0):
        raise ValueError("degenerate angle: coincident points")
    dot = v1[0] * v2[0] + v1[1] * v2[1]
    cross = v1[0] * v2[1] - v1[1] * v2[0]
    return abs(math.degrees(math.atan2(cross, dot)))


class RepStateMachine:
    """Two-threshold hysteresis counter.

    A rep completes when the angle crosses below `down_below` and then back
    above `up_above`. Jitter between the two thresholds can never double-count,
    and a partial movement that misses either threshold counts nothing.
    """

    def __init__(self, down_below: float, up_above: float) -> None:
        if not down_below < up_above:
            raise ValueError("down_below must be less than up_above")
        self.down_below = down_below
        self.up_above = up_above
        self._in_down = False

    def update(self, current_angle: float) -> bool:
        """Feed one angle sample; True when this sample completes a rep."""
        if current_angle < self.down_below:
            self._in_down = True
            return False
        if self._in_down and current_angle > self.up_above:
            self._in_down = False
            return True
        return False
