import { useState } from "react";
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
  if (!snapshot) return <p className="small">Connecting…</p>;

  return (
    <>
      <Screen snapshot={snapshot} variant={variant} />
      {import.meta.env.DEV && variant === "primary" && (
        <>
          <DebugModeToggle snapshot={snapshot} onDetectionDebug={() => setShowDebug((v) => !v)} />
          {showDebug && (
            <div className="debug-host">
              <DebugPanel />
            </div>
          )}
        </>
      )}
    </>
  );
}
