import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import "./App.css";
import "./hud.css";
import { DebugModeToggle } from "./DebugModeToggle";
import { DebugPanel } from "./DebugPanel";
import { HourglassLock } from "./HourglassLock";
import { WorkoutBigScreen } from "./WorkoutBigScreen";
import type { Action } from "./WorkstationCard";
import { WorkstationCard } from "./WorkstationCard";
import { useSnapshot } from "./useSnapshot";

const COMMANDS: Record<Action, (value?: number) => Promise<unknown>> = {
  start_session: () => invoke("start_session"),
  begin_workout: () => invoke("begin_workout"),
  simulate_rep: (v) => invoke("simulate_progress", { value: v ?? 1, satisfied: false }),
  simulate_done: () => invoke("simulate_progress", { value: 999, satisfied: true }),
  confirm_weight: (v) => invoke("confirm_weight", { weight: v ?? 0 }),
  resume_coding: () => invoke("resume_coding"),
};

export default function App() {
  const snapshot = useSnapshot();
  const [showDebug, setShowDebug] = useState(false);
  if (!snapshot) return <p>Connecting…</p>;

  const onAction = (a: Action, value?: number) => void COMMANDS[a](value);

  // A set is required → full-screen hourglass LOCK (visual only, no OS lock).
  // Doing a set / confirming weight → the gym-TV BIG SCREEN.
  // CODING / UNLOCKED → the compact workstation card + controls.
  let view;
  if (snapshot.phase === "EXERCISE_REQUIRED") {
    view = <HourglassLock snapshot={snapshot} onStart={() => onAction("begin_workout")} />;
  } else if (snapshot.phase === "WORKOUT_ACTIVE" || snapshot.phase === "WEIGHT_CONFIRMATION") {
    view = <WorkoutBigScreen snapshot={snapshot} onAction={onAction} />;
  } else {
    view = (
      <main className="container">
        <WorkstationCard snapshot={snapshot} onAction={onAction} />
        <button onClick={() => setShowDebug((v) => !v)}>
          {showDebug ? "Hide" : "Show"} Detection Debug
        </button>
        {showDebug && <DebugPanel />}
      </main>
    );
  }

  // The debug mode toggle floats above every screen.
  return (
    <>
      {view}
      <DebugModeToggle snapshot={snapshot} />
    </>
  );
}
