import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { Prescription, Snapshot } from "./snapshot";

export function DebugModeToggle({ snapshot, onDetectionDebug }: { snapshot: Snapshot; onDetectionDebug: () => void }) {
  const [error, setError] = useState<string | null>(null);
  const [exercises, setExercises] = useState<Prescription[]>([]);
  const [exercise, setExercise] = useState("");
  useEffect(() => {
    let active = true;
    invoke<Prescription[]>("debug_exercises").then(options => {
      if (!active) return;
      setExercises(options);
      setExercise(options[0]?.exercise ?? "");
    }).catch(reason => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, []);
  const command = (name: string, args?: Record<string, unknown>) => {
    setError(null);
    void invoke(name, args).catch(reason => setError(String(reason)));
  };
  const phase = snapshot.phase;
  const inWorkout = phase !== "CODING" && phase !== "UNLOCKED";
  const next = (snapshot.progress?.value ?? 0) + 1;

  const coding = () => command("debug_mode", { mode: "coding" });
  const workout = () => { if (exercise) command("debug_mode", { mode: "workout", exercise }); };
  const plusOne = () => command("simulate_progress", { value: next, satisfied: false });
  const done = () => command("simulate_progress", { value: 999, satisfied: true });

  // F1–F4 mirror the buttons so the loop can be driven from the keyboard.
  useEffect(() => {
    const keys: Record<string, () => void> = { F1: coding, F2: workout, F3: plusOne, F4: done };
    const onKey = (e: KeyboardEvent) => {
      if (keys[e.key]) { e.preventDefault(); keys[e.key](); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="debug-toggle" role="group" aria-label="debug mode">
      <label htmlFor="debug-exercise">Exercise</label>
      <select id="debug-exercise" value={exercise} disabled={!exercises.length} onChange={event => {
        const nextExercise = event.target.value;
        setExercise(nextExercise);
        if (inWorkout) command("debug_mode", { mode: "workout", exercise: nextExercise });
      }}>
        {!exercises.length && <option value="">Loading exercises…</option>}
        {exercises.map(rx => <option key={rx.exercise} value={rx.exercise}>
          {rx.exercise.replace(/_/g, " ")} · {rx.kind === "REP" ? `${rx.targetReps} reps` : `${rx.targetSeconds} sec`}
        </option>)}
      </select>
      <button className={!inWorkout ? "on" : ""} onClick={coding}>
        F1 Stop test
      </button>
      <button className={inWorkout ? "on" : ""} disabled={!exercise} onClick={workout}>
        F2 Start camera
      </button>
      <button onClick={plusOne}>F3 +1</button>
      <button onClick={done}>F4 Done</button>
      <button onClick={onDetectionDebug}>Video inspection</button>
      {error && <span role="alert">{error}</span>}
      <span>{phase.replace(/_/g, " ").toLowerCase()}</span>
    </div>
  );
}
