// Shared pose/skeleton rendering for the stream visualizer and the big screen.
// Mirrors reps_vision.visualize; landmark x/y are normalized 0..1 so the same
// draw scales to any canvas size.

export interface LandmarkFrame {
  poseDetected: boolean;
  landmarks: Record<string, [number, number, number]>;
  angle: number | null;
  measuredJoints: string[];
  tsUs: number;
}

export const CONNECTIONS: Array<[string, string]> = [
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

// Green-on-dark palette; the feed background matches the cockpit glass so the
// pose view sits inside the HUD rather than floating as its own screen.
export const POSE_BG = "#070e13";
export const POSE_BONE = "#6c6";
export const POSE_JOINT = "#4f4";
export const POSE_IDLE = "#3a5a3a";

export function drawSkeleton(
  ctx: CanvasRenderingContext2D,
  frame: LandmarkFrame,
  w: number,
  h: number,
): void {
  ctx.fillStyle = POSE_BG;
  ctx.fillRect(0, 0, w, h);
  if (!frame.poseDetected) return;
  const highlighted = new Set(frame.measuredJoints);
  const scale = Math.min(w, h) / 240;
  ctx.strokeStyle = POSE_BONE;
  ctx.lineWidth = 3 * scale;
  ctx.lineCap = "round";
  for (const [a, b] of CONNECTIONS) {
    const pa = frame.landmarks[a];
    const pb = frame.landmarks[b];
    if (!pa || !pb) continue;
    ctx.beginPath();
    ctx.moveTo(pa[0] * w, pa[1] * h);
    ctx.lineTo(pb[0] * w, pb[1] * h);
    ctx.stroke();
  }
  for (const [name, point] of Object.entries(frame.landmarks)) {
    const on = highlighted.has(name);
    ctx.fillStyle = on ? POSE_JOINT : POSE_IDLE;
    ctx.beginPath();
    ctx.arc(point[0] * w, point[1] * h, (on ? 6 : 3.5) * scale, 0, 2 * Math.PI);
    ctx.fill();
  }
}
