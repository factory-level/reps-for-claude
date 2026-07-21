// Full-pipeline e2e + latency budget (M5): spawns hubd (dev sibling or the
// staged bundle), loads the reps plugin, streams a real squat fixture video
// through MediaPipe at full rate, and asserts:
//   - rep_completed events arrive (event taxonomy)
//   - progress carries reps (duration/range covered by unit tests)
//   - landmark p95 latency (capture tsUs -> client receive) < 50 ms
// Usage: node scripts/e2e-latency.mjs [--bundle]
import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const useBundle = process.argv.includes("--bundle");
const hubDir = process.env.HUB_DIR ?? join(repoRoot, "..", "usb-mcp-hub");
const fixture = join(repoRoot, "vision", "tests", "fixtures", "videos", "squat_demo.webm");
const pluginArgs = `--plugin-path ${join(repoRoot, "vision", "src")} --plugin reps_vision.hub_plugin.plugin:RepsVisionPlugin`;

const LATENCY_BUDGET_MS = 50;
const MIN_FRAMES = 100;

function fail(message) {
  console.error(`E2E FAIL: ${message}`);
  process.exit(1);
}

// --- spawn hubd -----------------------------------------------------------
const env = {
  ...process.env,
  HUB_PLUGIN_ARGS: pluginArgs,
  PORT: "8446",
  DEBUG_PORT: "8084",
  HUB_CERT_DIR: "/nonexistent-e2e",
};
let child;
if (useBundle) {
  const bundle = join(repoRoot, "app", "src-tauri", "resources", "hub-bundle");
  child = spawn("node", [join(bundle, "hubd.mjs")], {
    env: {
      ...env,
      HUB_VISION_DIR: join(bundle, "vision"),
      HUB_PUBLIC_DIR: join(bundle, "public"),
    },
    stdio: ["ignore", "pipe", "inherit"],
    detached: true,
  });
} else {
  child = spawn("pnpm", ["--filter", "@hub/hubd", "start"], {
    cwd: hubDir,
    env,
    stdio: ["ignore", "pipe", "inherit"],
    detached: true,
  });
}
const kill = () => {
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    child.kill("SIGTERM");
  }
};
process.on("exit", kill);
setTimeout(() => fail("timed out"), 180_000).unref();

// --- wait for READY -------------------------------------------------------
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

// --- drive the client API -------------------------------------------------
const ws = new WebSocket(`ws://127.0.0.1:${debugPort}/v1/ws`);
const landmarks = [];
const progress = [];
const events = [];
let nextId = 1;
const pending = new Map();

const rpc = (method, params) =>
  new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });

const done = new Promise((resolveDone) => {
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.type === "hello") {
      if (!String(data.apiVersion).startsWith("1.")) fail(`api version ${data.apiVersion}`);
      return;
    }
    if (data.id !== undefined && pending.has(data.id)) {
      const { resolve, reject } = pending.get(data.id);
      pending.delete(data.id);
      data.error ? reject(new Error(data.error.message)) : resolve(data.result);
      return;
    }
    if (data.stream === "landmarks") {
      landmarks.push({ tsUs: data.tsUs, receivedUs: Date.now() * 1000 });
    } else if (data.stream === "progress") {
      progress.push(data.data);
    } else if (data.stream === "event") {
      events.push(data.data);
      if (data.data.type === "stream_ended" || data.data.type === "target_reached") {
        resolveDone();
      }
    }
  };
});

await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = () => reject(fail("cannot connect to client API"));
});
await rpc("subscribe", { streams: ["landmarks", "progress", "event", "health"] });
await rpc("enable_metric", {
  metricId: "e2e",
  pluginId: "reps_vision",
  config: {
    activity: "lift",
    targetReps: 2,
    exercise: { name: "squat", joints: ["hip", "knee", "ankle"], downBelow: 110, upAbove: 160 },
    camera: { source: "file", value: fixture, id: "fixture" },
  },
});
console.log("metric enabled; streaming fixture through mediapipe…");

// Wait for target_reached, or fall back to a fixed window if the fixture
// has fewer clean reps than the target.
await Promise.race([done, new Promise((resolve) => setTimeout(resolve, 90_000))]);
await rpc("disable_metric", { metricId: "e2e" }).catch(() => {});
ws.close();
kill();

// --- verdicts -------------------------------------------------------------
if (landmarks.length < MIN_FRAMES) {
  fail(`only ${landmarks.length} landmark frames (< ${MIN_FRAMES})`);
}
const reps = events.filter((event) => event.type === "rep_completed");
if (reps.length === 0) fail("no rep_completed events from the squat fixture");
if (!progress.some((entry) => entry.unit === "reps")) fail("no reps progress");

const latencies = landmarks
  .map((frame) => (frame.receivedUs - frame.tsUs) / 1000)
  .sort((a, b) => a - b);
const p50 = latencies[Math.floor(latencies.length * 0.5)];
const p95 = latencies[Math.floor(latencies.length * 0.95)];
console.log(
  `frames=${landmarks.length} reps=${reps.length} latency p50=${p50.toFixed(1)}ms p95=${p95.toFixed(1)}ms`,
);
if (p95 >= LATENCY_BUDGET_MS) {
  fail(`landmark p95 latency ${p95.toFixed(1)}ms exceeds ${LATENCY_BUDGET_MS}ms budget`);
}
console.log("E2E PASS");
process.exit(0);
