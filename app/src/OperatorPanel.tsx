// Live vision view during a workout: skeleton overlay from the hub's
// landmark stream, semantic events, and the honor-mode fallback when the
// camera path is down.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

interface LandmarkFrame {
  poseDetected: boolean;
  landmarks: Record<string, [number, number, number]>;
  angle: number | null;
  measuredJoints: string[];
  tsUs: number;
}

interface VisionEventPayload {
  kind: string;
  payload?: Record<string, unknown>;
}

// Stick-figure subset, mirrored from reps_vision.visualize.POSE_CONNECTIONS.
const CONNECTIONS: Array<[string, string]> = [
  ["left_shoulder", "right_shoulder"],
  ["left_shoulder", "left_elbow"],
  ["left_elbow", "left_wrist"],
  ["right_shoulder", "right_elbow"],
  ["right_elbow", "right_wrist"],
  ["left_shoulder", "left_hip"],
  ["right_shoulder", "right_hip"],
  ["left_hip", "right_hip"],
  ["left_hip", "left_knee"],
  ["left_knee", "left_ankle"],
  ["right_hip", "right_knee"],
  ["right_knee", "right_ankle"],
];

const WIDTH = 320;
const HEIGHT = 240;

export function drawSkeleton(
  ctx: CanvasRenderingContext2D,
  frame: LandmarkFrame,
): void {
  ctx.fillStyle = "#111";
  ctx.fillRect(0, 0, WIDTH, HEIGHT);
  if (!frame.poseDetected) return;
  const highlighted = new Set(frame.measuredJoints);
  ctx.strokeStyle = "#6c6";
  ctx.lineWidth = 2;
  for (const [a, b] of CONNECTIONS) {
    const pa = frame.landmarks[a];
    const pb = frame.landmarks[b];
    if (!pa || !pb) continue;
    ctx.beginPath();
    ctx.moveTo(pa[0] * WIDTH, pa[1] * HEIGHT);
    ctx.lineTo(pb[0] * WIDTH, pb[1] * HEIGHT);
    ctx.stroke();
  }
  for (const [name, point] of Object.entries(frame.landmarks)) {
    ctx.fillStyle = highlighted.has(name) ? "#4f4" : "#888";
    ctx.beginPath();
    ctx.arc(point[0] * WIDTH, point[1] * HEIGHT, highlighted.has(name) ? 5 : 3, 0, 2 * Math.PI);
    ctx.fill();
  }
}

export function OperatorPanel() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [angle, setAngle] = useState<number | null>(null);
  const [poseDetected, setPoseDetected] = useState(false);
  const [lastEvent, setLastEvent] = useState<string>("");
  const [fallback, setFallback] = useState<string | null>(null);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    void listen<LandmarkFrame>("vision-landmarks", (event) => {
      const frame = event.payload;
      setPoseDetected(frame.poseDetected);
      setAngle(frame.angle);
      const ctx = canvasRef.current?.getContext("2d");
      if (ctx) drawSkeleton(ctx, frame);
    }).then((u) => unlisteners.push(u));
    void listen<VisionEventPayload>("vision-event", (event) => {
      setLastEvent(event.payload.kind);
      if (event.payload.kind === "rep_completed" || event.payload.kind === "target_reached") {
        setLastEvent(`${event.payload.kind} ✓`);
      }
    }).then((u) => unlisteners.push(u));
    void listen<{ reason: string }>("vision-fallback", (event) => {
      setFallback(event.payload.reason);
    }).then((u) => unlisteners.push(u));
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  return (
    <section className="operator-panel">
      <h3>Operator</h3>
      <canvas ref={canvasRef} width={WIDTH} height={HEIGHT} data-testid="skeleton-canvas" />
      <p data-testid="operator-status">
        {poseDetected
          ? `pose · angle ${angle === null ? "–" : Math.round(angle)}°`
          : "no pose detected"}
        {lastEvent ? ` · ${lastEvent}` : ""}
      </p>
      {fallback && (
        <div data-testid="honor-mode">
          <p>camera unavailable: {fallback}</p>
          <button onClick={() => void invoke("honor_complete")}>Done (honor)</button>
        </div>
      )}
    </section>
  );
}
