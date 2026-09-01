// Dev-only floating controls (mounted only under `import.meta.env.DEV`): jump
// between CODING and WORKOUT without waiting out the timer, fake reps so the
// whole loop can be walked at the desk without a camera, and open the
// detection debug view.
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import type { Snapshot } from "./snapshot";

export function DebugModeToggle({ snapshot, onDetectionDebug }: { snapshot: Snapshot; onDetectionDebug: () => void }) {
  const phase = snapshot.phase;
  const inWorkout = phase !== "CODING" && phase !== "UNLOCKED";
  const next = (snapshot.progress?.value ?? 0) + 1;

  const coding = () => void invoke("debug_mode", { mode: "coding" });
  const workout = () => void invoke("debug_mode", { mode: "workout" });
  const plusOne = () => void invoke("simulate_progress", { value: next, satisfied: false });
  const done = () => void invoke("simulate_progress", { value: 999, satisfied: true });

  // F1–F4 mirror the buttons so the loop can be driven from the keyboard.
  useEffect(() => {
    const keys: Record<string, () => void> = { F1: coding, F2: workout, F3: plusOne, F4: done };
    const onKey = (e: KeyboardEvent) => keys[e.key]?.();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="debug-toggle" role="group" aria-label="debug mode">
      <span>DEV</span>
      <button className={!inWorkout ? "on" : ""} onClick={coding}>
        F1 Coding
      </button>
      <button className={inWorkout ? "on" : ""} onClick={workout}>
        F2 Workout
      </button>
      <button onClick={plusOne}>F3 +1</button>
      <button onClick={done}>F4 Done</button>
      <button onClick={onDetectionDebug}>Detect</button>
      <span>{phase.replace(/_/g, " ").toLowerCase()}</span>
    </div>
  );
}
