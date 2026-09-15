import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

export type AppMode = "debug" | "workout";

export function AppModeBar({ mode, primary }: { mode: AppMode; primary: boolean }) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function change(next: AppMode) {
    if (pending || next === mode) return;
    setPending(true);
    setError(null);
    try { await invoke("set_app_mode", { mode: next }); }
    catch (reason) { setError(String(reason)); setPending(false); }
  }
  return (
    <header className="app-mode-bar">
      <div role="group" aria-label="Application mode">
        <button aria-pressed={mode === "debug"} disabled={pending} onClick={() => void change("debug")}>Debug</button>
        <button aria-pressed={mode === "workout"} disabled={pending} onClick={() => void change("workout")}>Workout</button>
      </div>
      <span role="status">{pending ? "Restarting in your chosen mode…" : mode === "debug"
        ? "Manual testing · no screen takeover"
        : "Workout mode · screen locks when a set is due"}</span>
      {primary && mode === "debug" && <button onClick={() => void invoke("show_gym").catch(reason => setError(String(reason)))}>Open gym window</button>}
      {!pending && <small>Mode changes restart the app.</small>}
      {error && <span role="alert">{error}</span>}
    </header>
  );
}
