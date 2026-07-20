import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Snapshot } from "./snapshot";

export function useSnapshot(): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    invoke<Snapshot>("get_snapshot").then((s) => {
      if (!cancelled) setSnapshot(s);
    });
    listen<Snapshot>("snapshot", (e) => setSnapshot(e.payload)).then((u) => {
      if (cancelled) {
        u();
        return;
      }
      unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return snapshot;
}
