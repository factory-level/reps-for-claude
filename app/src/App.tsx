import { invoke } from "@tauri-apps/api/core";
import { AppModeBar, type AppMode } from "./AppModeBar";
import { useEffect, useState } from "react";
import "./retro.css";
import { DebugModeToggle } from "./DebugModeToggle";
import { DebugPanel } from "./DebugPanel";
import { Screen, type Variant } from "./Screen";
import { useSnapshot } from "./useSnapshot";

// Same SPA in both Tauri windows; the gym one loads `index.html?window=gym`.
const variant: Variant = new URLSearchParams(window.location.search).get("window") === "gym" ? "gym" : "primary";

export default function App() {
  const snapshot = useSnapshot();
  const [showDebug, setShowDebug] = useState(false);
  const [mode, setMode] = useState<AppMode | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    invoke<AppMode>("get_app_mode").then(value => { if (active) setMode(value); })
      .catch(reason => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, []);
  if (error) return <p role="alert">Could not load application mode: {error}</p>;
  if (!snapshot || !mode) return <p className="small">Connecting…</p>;

  return (
    <div className="app-shell">
      <AppModeBar mode={mode} primary={variant === "primary"} />
      <main className="app-content">
      <Screen snapshot={snapshot} variant={variant} debug={mode === "debug"} />
      {mode === "debug" && variant === "primary" && showDebug && (
        <div className="debug-host"><DebugPanel /></div>
      )}
      </main>
      {mode === "debug" && variant === "primary" && (
          <DebugModeToggle snapshot={snapshot} onDetectionDebug={() => setShowDebug((v) => !v)} />
      )}
    </div>
  );
}
