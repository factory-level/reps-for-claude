import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import "./App.css";
import { DebugPanel } from "./DebugPanel";
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
  return (
    <main className="container">
      <WorkstationCard
        snapshot={snapshot}
        onAction={(a, value) => void COMMANDS[a](value)}
      />
      <button onClick={() => setShowDebug((v) => !v)}>
        {showDebug ? "Hide" : "Show"} Detection Debug
      </button>
      {showDebug && <DebugPanel />}
    </main>
  );
}
