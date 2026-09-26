# Debug mode

The desktop app opens in **Debug** on first launch. The **Debug / Workout**
switch stays visible in release builds and remembers your choice. Switching
modes restarts the app and stops the detector first.

Debug uses temporary workout and hub data, so simulated sets never change your
real history or daily completion, and they are never uploaded.

## Testing manually

Debug starts idle in a normal movable window. Use **Start camera**, **+1**,
**Done** and **Stop test** to drive a set by hand, or open **Video inspection**
for the bundled squat clip. Video inspection displays the legacy counter
preview; it does not award workout credit.

**Open gym window** opens an optional second window, which can be closed and
reopened. Closing the main window exits and releases owned camera processes.

Choose an **Exercise** in the bottom toolbar, then press **F2 Start camera**.
All shipped detectors are available regardless of the daily routine — changing
the selection during a test restarts detection with a fresh count. Lift tests
target 10 reps; jump rope targets 60 seconds and stretching 30 seconds.

The same surface is available from the CLI:

```sh
rfp debug exercises|videos|history
rfp debug start --exercise squat
rfp debug step --value 3
rfp debug done|stop
rfp debug video --exercise squat --file PATH
rfp debug video-stop
```

## Workout mode

Workout mode enables the automatic timer and the window lock behavior. The
emergency release is **Ctrl+Shift+Backspace held for three seconds**.

## Movement Studio

The hub's `/studio.html` provides reviewed recordings, AI-assisted
configuration, held-out evaluation and version activation. New workouts use the
active version of their movement; existing presets remain available until
qualification. See
[setup and release qualification](../production/movement-studio.md).
