"""Per-exercise rep specs: which joint angle to track and its thresholds.

Landmarks are side-less joint names ("shoulder", "elbow", "wrist"); the side
(left/right) is chosen per frame by landmark visibility, so a side-on camera
works with either orientation.
"""

from __future__ import annotations

from dataclasses import dataclass

from .angles import angle

Landmarks = dict[str, tuple[float, float, float]]  # name -> (x, y, visibility)

SIDES = ("left", "right")


@dataclass(frozen=True)
class ExerciseSpec:
    name: str
    joints: tuple[str, str, str]  # angle measured at the middle joint
    down_below: float
    up_above: float
    min_visibility: float = 0.5

    def measured_joints(self, landmarks: Landmarks) -> tuple[str, str, str] | None:
        """The three landmark names on the most visible body side, or None.

        This is the joint triple the angle is (or would be) measured from, so
        the visualizer can highlight exactly what the counter is watching.
        """
        best: tuple[float, tuple[str, str, str]] | None = None
        for side in SIDES:
            names = tuple(f"{side}_{joint}" for joint in self.joints)
            points = [landmarks.get(name) for name in names]
            if any(p is None for p in points):
                continue
            visibility = min(p[2] for p in points)  # type: ignore[index]
            if visibility < self.min_visibility:
                continue
            if best is None or visibility > best[0]:
                best = (visibility, names)  # type: ignore[arg-type]
        return best[1] if best else None

    def angle_from(self, landmarks: Landmarks) -> float | None:
        """Joint angle from the most visible body side, or None if neither is."""
        names = self.measured_joints(landmarks)
        if names is None:
            return None
        a, b, c = (landmarks[name] for name in names)
        try:
            return angle((a[0], a[1]), (b[0], b[1]), (c[0], c[1]))
        except ValueError:
            return None


ELBOW = ("shoulder", "elbow", "wrist")
KNEE = ("hip", "knee", "ankle")

SPECS: dict[str, ExerciseSpec] = {
    spec.name: spec
    for spec in (
        # elbow extends at the top of the movement
        ExerciseSpec("pushup", ELBOW, down_below=95.0, up_above=155.0),
        ExerciseSpec("bench", ELBOW, down_below=95.0, up_above=150.0),
        ExerciseSpec("overhead", ELBOW, down_below=95.0, up_above=155.0),
        # elbow flexes at the top; same crossing pattern, thresholds swapped roles
        ExerciseSpec("curl", ELBOW, down_below=60.0, up_above=150.0),
        ExerciseSpec("pullup", ELBOW, down_below=75.0, up_above=160.0),
        ExerciseSpec("row", ELBOW, down_below=90.0, up_above=150.0),
        # knee tracks the squat
        ExerciseSpec("squat", KNEE, down_below=110.0, up_above=160.0),
    )
}


def get_spec(exercise: str) -> ExerciseSpec:
    try:
        return SPECS[exercise]
    except KeyError:
        known = ", ".join(sorted(SPECS))
        raise KeyError(f"unknown exercise {exercise!r}; known: {known}") from None
