# Start here

The desktop contains the workout timer and daily plan. A bundled local camera
hub runs the Reps pose detector. Live detection runs locally; optional cloud
assistance is used only when you consent to authoring from selected frames.

The app starts in **Debug**. Select an exercise, then **Start camera** (F2).
Use **Stop test** when finished. Manual **+1** and **Done** controls let you test
the UI. Debug uses temporary workout and hub state and does not change your real
completion history. Video inspection is a legacy-counter preview and awards no
workout credit.

Switch to **Workout** for automatic timer and lock behavior. Switching modes
restarts the app and stops detection first. See [the loop](the-loop.md).
To build locally, see [development commands](../reference/cli.md).
