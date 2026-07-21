// Two-camera fusion e2e (N3): registers a "front" camera whose video goes
// black mid-set (generated occlusion fixture) and a clean "side" camera,
// enables the reps metric across both with best-view election on landmark
// visibility, and asserts:
//   - best-view election fails over (primary_camera_changed away from the
//     occluded camera arrives)
//   - the rep count survives the occlusion (>= TARGET rep_completed events)
// Usage: node scripts/e2e-two-camera.mjs
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const hubDir = process.env.HUB_DIR ?? join(repoRoot, "..", "usb-mcp-hub");
const sideFixture = join(repoRoot, "vision", "tests", "fixtures", "videos", "squat_demo.webm");
const pluginArgs = `--plugin-path ${join(repoRoot, "vision", "src")} --plugin reps_vision.hub_plugin.plugin:RepsVisionPlugin`;
const TARGET_REPS = 2;

function fail(message) {
  console.error(`E2E FAIL: ${message}`);
  process.exit(1);
}

if (!existsSync(sideFixture)) fail(`missing fixture ${sideFixture}`);

// --- generate the occluded front fixture ----------------------------------
const workDir = mkdtempSync(join(tmpdir(), "reps-two-cam-"));
const frontFixture = join(workDir, "squat_front_occluded.avi");
execFileSync(
  "uv",
  ["run", "python", join(repoRoot, "scripts", "make_occluded_fixture.py"), sideFixture, frontFixture],
  { cwd: join(repoRoot, "vision"), stdio: "inherit" },
);

// --- spawn hubd ------------------------------------------------------------
const child = spawn("pnpm", ["--filter", "@hub/hubd", "start"], {
  cwd: hubDir,
  env: {
    ...process.env,
    HUB_PLUGIN_ARGS: pluginArgs,
    HUB_DATA_DIR: workDir,
    PORT: "8449",
    DEBUG_PORT: "8087",
    HUB_CERT_DIR: "/nonexistent-e2e",
  },
  stdio: ["ignore", "pipe", "inherit"],
  detached: true,
});
const kill = () => {
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    child.kill("SIGTERM");
  }
};
process.on("exit", kill);
setTimeout(() => fail("timed out"), 180_000).unref();

const debugPort = await new Promise((resolve, reject) => {
  let buffer = "";
  child.stdout.on("data", (chunk) => {
    buffer += String(chunk);
    const match = buffer.match(/HUBD READY (\{.*\})/);
    if (match) resolve(JSON.parse(match[1]).debugPort);
  });
  child.once("exit", (code) => reject(new Error(`hubd exited early (${code})`)));
});
console.log(`hubd ready on debug port ${debugPort}`);

// --- drive the client API ---------------------------------------------------
const ws = new WebSocket(`ws://127.0.0.1:${debugPort}/v1/ws`);
const events = [];
const elections = [];
let fusedLandmarks = 0;
let nextId = 1;
const pending = new Map();

const rpc = (method, params) =>
  new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });

let endedStreams = 0;
const done = new Promise((resolveDone) => {
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.type === "hello") return;
    if (data.id !== undefined && pending.has(data.id)) {
      const { resolve, reject } = pending.get(data.id);
      pending.delete(data.id);
      data.error ? reject(new Error(data.error.message)) : resolve(data.result);
      return;
    }
    if (data.stream === "landmarks" && data.fused) fusedLandmarks += 1;
    if (data.stream === "event") {
      if (data.data.type === "primary_camera_changed") {
        elections.push(data.data);
        console.log(`election: ${data.data.from} -> ${data.data.to}`);
        return;
      }
      events.push(data.data);
      if (data.data.type === "stream_ended" && ++endedStreams >= 1) {
        // one camera finishing is enough to grade the run
        setTimeout(resolveDone, 2000);
      }
    }
  };
});

await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = () => reject(fail("cannot connect to client API"));
});
await rpc("subscribe", { streams: ["landmarks", "progress", "event", "health"] });
await rpc("add_camera", {
  camera: { cameraId: "front", kind: "file", source: frontFixture, label: "front (occluded)" },
});
await rpc("add_camera", {
  camera: { cameraId: "side", kind: "file", source: sideFixture, label: "side" },
});
await rpc("enable_metric", {
  metricId: "e2e-two-cam",
  pluginId: "reps_vision",
  cameras: ["front", "side"],
  config: {
    activity: "lift",
    targetReps: TARGET_REPS,
    exercise: { name: "squat", joints: ["hip", "knee", "ankle"], downBelow: 110, upAbove: 160 },
    fusion: { policy: "best", scoreField: "visibility", windowMs: 1000, hysteresisPct: 15, minScore: 0.2 },
  },
});
console.log("camera-set metric enabled; streaming both fixtures through mediapipe…");

await Promise.race([done, new Promise((resolve) => setTimeout(resolve, 120_000))]);
await rpc("disable_metric", { metricId: "e2e-two-cam" }).catch(() => {});
ws.close();
kill();

// --- grade ------------------------------------------------------------------
const reps = events.filter((e) => e.type === "rep_completed").length;
const failover = elections.some((e) => e.from === "front" && e.to === "side");
console.log(
  `fusedLandmarks=${fusedLandmarks} reps=${reps} elections=${elections
    .map((e) => `${e.from}->${e.to}`)
    .join(",")}`,
);
if (fusedLandmarks === 0) fail("no fused landmark frames");
if (elections.length === 0) fail("no election ever happened");
if (!failover) fail("election never failed over from occluded front to side");
if (reps < TARGET_REPS) fail(`rep count did not survive occlusion (${reps} < ${TARGET_REPS})`);
console.log("E2E TWO-CAMERA PASS");
