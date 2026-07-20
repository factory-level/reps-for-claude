import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

interface DebugVideo {
  exercise: string;
  path: string;
}

type DebugStreamEvent =
  | { event: "open"; exercise: string; fps: number; frameCount: number }
  | { event: "progress"; frame: number; value: number; unit: string; satisfied: boolean }
  | { event: "frame"; frame: number; jpegB64: string }
  | { event: "done"; total: number; satisfied: boolean }
  | { event: "exited"; code: number | null; stderrTail?: string[] }
  | { event: "error"; message: string };

/** Live detection-debug view: streams a fixture exercise video through the
 * Python sidecar (via the `debug_stream_start`/`debug_stream_stop` Tauri
 * commands) and renders the annotated frames + live rep/hold count as
 * `debug-stream` events arrive. */
export function DebugPanel() {
  const [videos, setVideos] = useState<DebugVideo[]>([]);
  const [selectedPath, setSelectedPath] = useState("");
  const [running, setRunning] = useState(false);
  const [frameSrc, setFrameSrc] = useState<string | null>(null);
  const [progress, setProgress] = useState<{
    value: number;
    unit: string;
    satisfied: boolean;
  } | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [stderrTail, setStderrTail] = useState<string[] | null>(null);
  const [startError, setStartError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<DebugVideo[]>("debug_videos").then((vs) => {
      if (cancelled) return;
      setVideos(vs);
      if (vs.length > 0) setSelectedPath(vs[0].path);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<DebugStreamEvent>("debug-stream", (e) => {
      const payload = e.payload;
      switch (payload.event) {
        case "open":
          setStatus(null);
          setFrameSrc(null);
          setProgress(null);
          break;
        case "progress":
          setProgress({
            value: payload.value,
            unit: payload.unit,
            satisfied: payload.satisfied,
          });
          break;
        case "frame":
          setFrameSrc(`data:image/jpeg;base64,${payload.jpegB64}`);
          break;
        case "done":
          setRunning(false);
          setStatus(
            `Done: ${payload.total} (${payload.satisfied ? "satisfied" : "not satisfied"})`,
          );
          break;
        case "exited":
          setRunning(false);
          setStatus(`Exited (code ${payload.code ?? "unknown"})`);
          setStderrTail(payload.code !== 0 ? payload.stderrTail ?? null : null);
          break;
        case "error":
          setRunning(false);
          setStatus(`Error: ${payload.message}`);
          break;
      }
    }).then((u) => {
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

  const selected = videos.find((v) => v.path === selectedPath);

  function start() {
    if (!selected) return;
    setRunning(true);
    setStatus(null);
    setFrameSrc(null);
    setProgress(null);
    setStderrTail(null);
    setStartError(null);
    invoke("debug_stream_start", {
      video: selected.path,
      exercise: selected.exercise,
    }).catch((e) => {
      setRunning(false);
      setStartError(`Failed to start: ${String(e)}`);
    });
  }

  function stop() {
    setRunning(false);
    void invoke("debug_stream_stop");
  }

  return (
    <section>
      <h2>Detection Debug</h2>
      <select value={selectedPath} onChange={(e) => setSelectedPath(e.target.value)}>
        {videos.map((v) => (
          <option key={v.path} value={v.path}>
            {v.exercise}
          </option>
        ))}
      </select>
      <div>
        <button onClick={start} disabled={running || !selected}>
          Start
        </button>
        <button onClick={stop} disabled={!running}>
          Stop
        </button>
      </div>
      {frameSrc && <img src={frameSrc} alt="debug stream frame" />}
      {progress && (
        <p>
          {progress.value} {progress.unit} ({progress.satisfied ? "✓" : "✗"})
        </p>
      )}
      {status && <p>{status}</p>}
      {startError && <p role="alert">{startError}</p>}
      {stderrTail && stderrTail.length > 0 && (
        <pre>{stderrTail.join("\n")}</pre>
      )}
    </section>
  );
}
