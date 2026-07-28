// A dev-only floating control to jump between CODING and WORKOUT without waiting
// out the coding timer — flips the session phase and the camera to match, so you
// can exercise live detection on demand. Mounts fixed above every screen.
import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "./snapshot";

export function DebugModeToggle({ snapshot }: { snapshot: Snapshot }) {
  const phase = snapshot.phase;
  const inWorkout =
    phase === "WORKOUT_ACTIVE" || phase === "WEIGHT_CONFIRMATION" || phase === "EXERCISE_REQUIRED";

  return (
    <div className="debug-toggle" role="group" aria-label="debug mode">
      <span className="dbg-tag">DEBUG</span>
      <button className={!inWorkout ? "on" : ""} onClick={() => void invoke("debug_mode", { mode: "coding" })}>
        ⌨ Coding
      </button>
      <button className={inWorkout ? "on" : ""} onClick={() => void invoke("debug_mode", { mode: "workout" })}>
        ▶ Workout
      </button>
      <span className="dbg-phase">{phase.replace(/_/g, " ").toLowerCase()}</span>
    </div>
  );
}
