# Counting reps and reducing sensitivity

The camera estimates joint positions, and movement rules turn those positions
into reps or active seconds. Camera framing and pose quality affect the result.

## Exercise reps

A shipped preset counts when your measured joint angle crosses the bent-position
threshold and then the extended-position threshold. Starting bent can count on
the first extension. Small movements between the two thresholds do not count.

On 2026-09-15, the defaults were made less sensitive for incline bench, curl,
pull-up, row, squat and deadlift. They require a larger movement before counting.
See the [exact before/after settings](../../architecture/detection.md#preset-changes-on-2026-09-15).
These are camera-tuning starting points, not a judgement of exercise technique.

An activated Studio movement uses its own rules, including sustained poses,
smoothing and cycle timing. It overrides the corresponding shipped preset.
If an activated movement feels too sensitive, create and evaluate a revised
candidate in Studio, then activate it for the next workout. Changing a legacy
preset does not change an activated version.

## Jump rope

The timer adds time when your hips move vertically enough between frames. The
shipped threshold is now **0.008**, up from 0.004: twice the displacement is needed.
Short still periods add no time; sustained stillness clears the streak after the
2-second grace period. This measures motion, not actual rope swings.
Frame rate and how large you appear in the image affect sensitivity.

## Check the result

1. In Debug mode, select the exercise and start the camera. Keep the relevant
   joints visible and check camera rotation and placement.
2. Stand still and make small incidental movements: the rep count should stay
   unchanged and the jump-rope timer should not advance.
3. Perform ten full reps at your normal pace; compare the counter with your own
   count. Test partial movements and pauses as well.
4. For jump rope, try real bouncing, short pauses and a pause longer than two
   seconds. Confirm the streak stops advancing and eventually resets.
5. If results are wrong, follow [tuning](../reference/adding-exercises.md) and
   collect reviewed examples before changing rules.

The new defaults passed synthetic checks, but have not had a camera trial.
They require a rebuilt desktop to replace the embedded defaults; existing
installed apps and running workouts do not update from a source edit.

## Stretch

Stretch completion is a timer. It does not verify your posture.
