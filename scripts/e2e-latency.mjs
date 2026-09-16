// Full-pipeline e2e + latency budget (M5): spawns hubd (dev sibling or the
// staged bundle), loads the reps plugin, streams a real squat fixture video
// through MediaPipe at full rate, and asserts:
//   - rep_completed events arrive (event taxonomy)
//   - progress carries reps (duration/range covered by unit tests)
//   - landmark p95 latency (capture tsUs -> client receive) < 50 ms
// Usage: node scripts/e2e-latency.mjs [--bundle | --resources DIR]
//        [--movement-contract] [--python-env DIR] [--offline] [--parent-exit]
// The movement-contract mode seeds an INTERNAL activation fixture in the fresh
// test database. It verifies wiring, not accuracy or production qualification.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
function option(name) {
  const index = process.argv.indexOf(name);
  if (index === -1) return undefined;
  const value = process.argv[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${name} requires a directory`);
  return resolve(value);
}
const resources = option("--resources") ?? join(repoRoot, "app", "src-tauri", "resources");
const useBundle = process.argv.includes("--bundle") || process.argv.includes("--resources");
const useMovement = process.argv.includes("--movement-contract");
const testParentExit = process.argv.includes("--parent-exit");
const hubDir = process.env.HUB_DIR ?? join(repoRoot, "..", "usb-mcp-hub");
const fixture = join(repoRoot, "vision", "tests", "fixtures", "videos", "squat_demo.webm");
const pluginArgs = ["--plugin-path", useBundle ? join(resources, "reps-vision") : join(repoRoot, "vision", "src"), "--plugin", "reps_vision.hub_plugin.plugin:RepsVisionPlugin"];

const LATENCY_BUDGET_MS = 50;
const MIN_FRAMES = 100;

function fail(message) {
  console.error(`E2E FAIL: ${message}`);
  process.exit(1);
}

// --- spawn hubd -----------------------------------------------------------
// Never restore a user's enabled metrics or deliver their webhook backlog in a test.
const testDataDir = mkdtempSync(join(tmpdir(), "reps-bundle-e2e-"));
const env = {
  ...process.env,
  HUB_PLUGIN_ARGS_JSON: JSON.stringify(pluginArgs),
  HUB_DATA_DIR: testDataDir,
  UV_PROJECT_ENVIRONMENT: option("--python-env") ?? join(testDataDir, "vision-env"),
  ...(process.argv.includes("--offline") ? { UV_OFFLINE: "1", UV_FROZEN: "1" } : {}),
  PYTHONDONTWRITEBYTECODE: "1",
  HUB_BIND_HOST: "127.0.0.1",
  HUB_EXIT_ON_STDIN_CLOSE: "1",
  PORT: "0", // The fixture must not collide with a running local Hub.
  DEBUG_PORT: "8084",
  HUB_CERT_DIR: "/nonexistent-e2e",
};
let child;
if (useBundle) {
  const bundle = join(resources, "hub-bundle");
  child = spawn("node", [join(bundle, "..", "boot-hub.mjs")], {
    env: {
      ...env,
      HUB_VISION_DIR: join(bundle, "vision"),
      HUB_PUBLIC_DIR: join(bundle, "public"),
      REPS_POSE_MODEL: join(bundle, "..", "models", "pose_landmarker_full.task"),
    },
    stdio: ["pipe", "pipe", "inherit"],
    detached: true,
  });
} else {
  child = spawn("pnpm", ["--filter", "@hub/hubd", "start"], {
    cwd: hubDir,
    env,
    stdio: ["pipe", "pipe", "inherit"],
    detached: true,
  });
}
const kill = () => {
  if (child.exitCode !== null || child.signalCode !== null) return;
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    child.kill("SIGTERM");
  }
};
const childExited = new Promise(resolve => child.once("exit", (code, signal) => resolve({ code, signal })));
process.on("exit", () => {
  kill();
  rmSync(testDataDir, { recursive: true, force: true });
});
// Let an enclosing qualification runner cancel us without orphaning the
// separately owned hub process group.
process.once("SIGTERM", () => process.exit(143));
process.once("SIGINT", () => process.exit(130));
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
const durableEvents = [];
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
    if (data.stream === "hub_event" && data.event?.payload?.metricId === "e2e") {
      durableEvents.push(data.event);
      return;
    }
    if (data.metricId !== "e2e") return;
    if (data.stream === "landmarks") {
      landmarks.push({ tsUs: data.tsUs, receivedUs: Date.now() * 1000 });
    } else if (data.stream === "progress") {
      if (useMovement && data.data.value !== durableEvents.filter(e => e.type === "rep_completed").length) {
        fail("workout progress preceded its committed repetitions");
      }
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
await rpc("subscribe", { streams: ["landmarks", "progress", "event", "health", "hub_event"] });
if (useMovement) {
  const response = await fetch(`http://127.0.0.1:${debugPort}/v1/studio/templates`);
  if (!response.ok) fail("cannot read movement capabilities");
  const plugin = (await response.json()).plugins?.reps_vision;
  if (!plugin?.movementTemplates?.squat || !plugin.model?.sha256) fail("movement capabilities missing");
  const row = { movementId: "e2e-squat", version: "e2e-version", name: "Integration fixture", description: "NOT qualification evidence",
    status: "active", createdAt: new Date().toISOString(), pluginId: "reps_vision", model: plugin.model,
    config: plugin.movementTemplates.squat, calibrationSessionIds: ["test-calibration"],
    calibrationRecordings: [{ streamId: "test-calibration-clip", recordingSessionId: "test-calibration", role: "calibration", reviewed: true, completions: [] }],
  };
  const db = new DatabaseSync(join(testDataDir, "hub.db"));
  try {
    db.exec("BEGIN IMMEDIATE");
    db.prepare("INSERT INTO movement_versions VALUES(?,?,?,?)").run(row.version, row.movementId, row.status, JSON.stringify(row));
    db.prepare("INSERT INTO movement_evaluations VALUES(?,?,?)").run("test-approval", row.version, JSON.stringify({
      evaluationId: "test-approval", version: row.version, movementId: row.movementId, createdAt: row.createdAt,
      passed: true, metrics: {}, failures: [], recordings: [], fixtureOnly: true,
    }));
    db.exec("COMMIT");
  } finally { db.close(); }
  console.log("using synthetic activation fixture: contract check only, not movement qualification");
}
await rpc("enable_metric", {
  metricId: "e2e",
  pluginId: "reps_vision",
  config: {
    activity: "lift",
    sessionId: "bundle-e2e",
    targetReps: 2,
    ...(useMovement ? { movementId: "e2e-squat" } : {
      exercise: { name: "squat", joints: ["hip", "knee", "ankle"], downBelow: 110, upAbove: 160 },
    }),
    camera: { source: "file", value: fixture, id: "fixture" },
  },
});
console.log("metric enabled; streaming fixture through mediapipe…");

// Wait for target_reached, or fall back to a fixed window if the fixture
// has fewer clean reps than the target.
await Promise.race([done, new Promise((resolve) => setTimeout(resolve, 90_000))]);
if (!testParentExit) await rpc("disable_metric", { metricId: "e2e" }).catch(() => {});
ws.close();
if (testParentExit) {
  // Closing this pipe is the OS notification the hub gets on desktop death.
  // Leave detection enabled to check cleanup while the capture is still owned.
  child.stdin.end();
  const exited = await Promise.race([childExited, new Promise((_, reject) => {
    setTimeout(() => reject(new Error("hub did not exit after its parent pipe closed")), 10_000).unref();
  })]);
  if (exited.code !== 0) fail("parent-pipe shutdown was not clean");
  console.log("parent-pipe shutdown PASS");
} else {
  kill();
}

// --- verdicts -------------------------------------------------------------
if (landmarks.length < MIN_FRAMES) {
  fail(`only ${landmarks.length} landmark frames (< ${MIN_FRAMES})`);
}
const reps = events.filter((event) => event.type === "rep_completed");
if (reps.length === 0) fail("no rep_completed events from the squat fixture");
if (!progress.some((entry) => entry.unit === "reps")) fail("no reps progress");
if (progress.some((entry) => entry.sessionId !== "bundle-e2e")) fail("progress lost its workout session identity");
if (useMovement) {
  if (progress.some(entry => entry.movementVersion !== "e2e-version")) fail("progress lost its pinned movement version");
  const committed = durableEvents.filter(event => event.type === "rep_completed");
  if (committed.length !== reps.length || committed.some(event => event.sessionId !== "bundle-e2e" || !event.payload.modelIdentity?.sha256)) {
    fail("durable repetition provenance differs from the live detector");
  }
}

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
