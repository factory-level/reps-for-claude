// Live stream-tracking visualizer: the hub's pose skeleton framed as a targeting
// view, with a joint-angle attitude readout, a radial "set reticle" that fills
// toward the target and pulses when a rep locks in, and a scrolling telemetry
// log. Consumes the vision-* events the Rust hub emits; optional `snapshot`
// supplies the current target/progress for the count.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { drawSkeleton, type LandmarkFrame } from "./pose";
import type { Snapshot } from "./snapshot";

const W = 640;
const H = 480;
const R = 46; // reticle ring radius (viewBox 0..120, center 60)
const CIRC = 2 * Math.PI * R;

interface VisionEventPayload {
  kind: string;
  payload?: Record<string, unknown>;
}

export function StreamVisualizer({ snapshot, compact }: { snapshot?: Snapshot | null; compact?: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [angle, setAngle] = useState<number | null>(null);
  const [poseDetected, setPoseDetected] = useState(false);
  const [feed, setFeed] = useState<string[]>([]);
  const [fallback, setFallback] = useState<string | null>(null);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    void listen<LandmarkFrame>("vision-landmarks", (event) => {
      const frame = event.payload;
      setPoseDetected(frame.poseDetected);
      setAngle(frame.angle);
      const ctx = canvasRef.current?.getContext("2d");
      if (ctx) drawSkeleton(ctx, frame, W, H);
    }).then((u) => unlisteners.push(u));
    void listen<VisionEventPayload>("vision-event", (event) => {
      const k = event.payload.kind;
      if (k === "rep_completed" || k === "target_reached") {
        setFeed((f) => [`${k === "target_reached" ? "TARGET REACHED" : "rep confirmed"}`, ...f].slice(0, 6));
      }
    }).then((u) => unlisteners.push(u));
    void listen<{ reason: string }>("vision-fallback", (event) => setFallback(event.payload.reason)).then((u) =>
      unlisteners.push(u),
    );
    return () => {
      for (const u of unlisteners) u();
    };
  }, []);

  const rx = snapshot?.prescription;
  const prog = snapshot?.progress;
  const unit = rx?.kind === "CONTINUOUS" ? "sec" : "reps";
  const target = rx ? (rx.kind === "CONTINUOUS" ? rx.targetSeconds : rx.targetReps) : null;
  const value = prog ? Math.round(prog.value) : 0;

  // Pulse the reticle for one beat each time the counted value climbs.
  const [locked, setLocked] = useState(false);
  const prevValue = useRef(value);
  useEffect(() => {
    if (value > prevValue.current) {
      setLocked(true);
      const t = setTimeout(() => setLocked(false), 560);
      prevValue.current = value;
      return () => clearTimeout(t);
    }
    prevValue.current = value;
  }, [value]);

  const pct = target && target > 0 ? Math.min(1, value / target) : 0;
  const attitude = angle === null ? 0 : Math.max(0, Math.min(1, angle / 180));

  return (
    <div className={`stream-viz ${compact ? "compact" : ""}`}>
      <div className="stream-stage framed">
        <canvas ref={canvasRef} width={W} height={H} data-testid="skeleton-canvas" />
        <div className={`stream-status ${poseDetected ? "live" : "off"}`}>
          <span className="lamp" />
          {poseDetected ? "TRACKING" : "SIGNAL LOST"}
        </div>
        <div className="stream-angle">
          <div className="val">{angle === null ? "––" : `${Math.round(angle)}°`}</div>
          <div className="lbl">JOINT ANGLE</div>
          <div className="stream-attitude">
            <i style={{ width: `${attitude * 100}%` }} />
          </div>
        </div>
        {fallback && <div className="stream-fallback">CAMERA DOWN · HONOR MODE</div>}
      </div>

      {!compact && (
        <div className="stream-instruments">
          {target !== null && (
            <div className={`reticle ${locked ? "locked" : ""}`} aria-label={`${value} of ${Math.round(target)} ${unit}`}>
              <svg viewBox="0 0 120 120" aria-hidden>
                <circle className="ring-track" cx="60" cy="60" r={R} />
                <circle className="ring-ticks" cx="60" cy="60" r={R} />
                <circle
                  className="ring-fill"
                  cx="60"
                  cy="60"
                  r={R}
                  strokeDasharray={CIRC}
                  strokeDashoffset={CIRC * (1 - pct)}
                />
              </svg>
              <div className="reticle-core">
                <span className="count">{value}</span>
                <span className="of">/ {Math.round(target)}</span>
                <span className="unit">{unit}</span>
              </div>
            </div>
          )}
          <ul className="stream-feed">
            <li className="log-head">▸ TELEMETRY</li>
            {feed.length === 0 ? (
              <li className="muted">awaiting movement…</li>
            ) : (
              feed.map((f, i) => (
                <li key={`${f}-${i}`} className="line">
                  {f}
                </li>
              ))
            )}
          </ul>
        </div>
      )}
    </div>
  );
}
