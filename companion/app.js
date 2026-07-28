// Reps companion (V1): Workout / Calibrate / History over the hub client API.
// One WS connection: subscribe landmarks + progress + hub_event, rpc for
// config and history. Answers "why did this rep fail to count?" live.

// Pairing token: same contract as the hub tuning app — QR carries #t=<token>
// once; keep it in localStorage.
(function stashToken() {
  const match = location.hash.match(/[#&]t=([^&]+)/);
  if (match) {
    localStorage.setItem("pairingToken", decodeURIComponent(match[1]));
    location.hash = location.hash.replace(/[#&]t=[^&]+/, "") || "#workout";
  }
})();
const token = localStorage.getItem("pairingToken");
const withToken = (url) =>
  token ? `${url}${url.includes("?") ? "&" : "?"}token=${encodeURIComponent(token)}` : url;

// ---- tabs ----------------------------------------------------------------
function showTab() {
  const name = (location.hash || "#workout").slice(1).split("&")[0] || "workout";
  for (const section of document.querySelectorAll("main section")) {
    section.classList.toggle("active", section.id === name);
  }
  for (const link of document.querySelectorAll("nav a")) {
    link.classList.toggle("active", link.id === `tab-${name}`);
  }
  if (name === "history") refreshHistory();
}
window.addEventListener("hashchange", showTab);

// ---- ws + rpc ------------------------------------------------------------
let ws, nextId = 1;
const pending = new Map();

function rpc(method, params = {}) {
  return new Promise((resolve, reject) => {
    if (!ws || ws.readyState !== 1) return reject(new Error("not connected"));
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => {
      if (pending.delete(id)) reject(new Error(`timeout: ${method}`));
    }, 10_000);
  });
}

function connect() {
  const proto = location.protocol === "https:" ? "wss" : "ws";
  ws = new WebSocket(withToken(`${proto}://${location.host}/v1/ws`));
  ws.onopen = () => setHealth("h-hub", "connected", "ok");
  ws.onclose = () => {
    setHealth("h-hub", "disconnected — retrying", "bad");
    setTimeout(connect, 2000);
  };
  ws.onmessage = (raw) => {
    const msg = JSON.parse(raw.data);
    if (msg.type === "hello") {
      rpc("subscribe", { streams: ["landmarks", "progress", "hub_event"] }).catch(() => {});
      loadPrescriptionAndConfig();
      pollHealth();
      return;
    }
    if (msg.id !== undefined && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message || msg.error.code));
      else resolve(msg.result);
      return;
    }
    if (msg.stream === "landmarks") onLandmarks(msg.data || {});
    else if (msg.stream === "progress") onProgress(msg.data || {});
    else if (msg.stream === "hub_event") onHubEvent(msg.event || {});
  };
}

// ---- workout tab ---------------------------------------------------------
const $ = (id) => document.getElementById(id);
function setHealth(id, text, cls) {
  const el = $(id);
  el.textContent = text;
  el.className = cls || "";
}

function onProgress(data) {
  $("live-count").textContent =
    data.unit === "seconds" ? `${Math.round(data.value)}s` : Math.round(data.value);
  $("live-unit").textContent = data.satisfied ? "target reached 🎉" : data.unit || "";
}

function onHubEvent(event) {
  if (event.type === "workout_prescribed") {
    const p = event.payload || {};
    $("rx-exercise").textContent = p.exercise || "—";
    $("rx-target").textContent =
      p.targetSeconds > 0 ? `${p.targetSeconds}s` : `${p.targetReps} reps`;
  }
  if (event.type === "detector_started") {
    setHealth("h-detector", "running", "ok");
    loadPrescriptionAndConfig();
  }
  if (event.type === "detector_stopped") setHealth("h-detector", "stopped", "muted");
  if (event.type === "system_error") setHealth("h-detector", "error — honor mode", "bad");
  prependEvent(event);
}

async function pollHealth() {
  try {
    const health = await rpc("health");
    setHealth("h-camera", health.camera, health.camera === "open" ? "ok" : "muted");
    if ($("h-detector").textContent === "—") {
      setHealth(
        "h-detector",
        health.enabledMetrics.length ? "running" : "idle",
        health.enabledMetrics.length ? "ok" : "muted",
      );
    }
  } catch { /* next poll */ }
  setTimeout(pollHealth, 5000);
}

// ---- calibrate tab -------------------------------------------------------
const canvas = $("skeleton");
const ctx = canvas.getContext("2d");
// MediaPipe pose connections we care about (subset; measured joints pop).
const BONES = [
  ["left_shoulder", "right_shoulder"], ["left_hip", "right_hip"],
  ["left_shoulder", "left_elbow"], ["left_elbow", "left_wrist"],
  ["right_shoulder", "right_elbow"], ["right_elbow", "right_wrist"],
  ["left_shoulder", "left_hip"], ["right_shoulder", "right_hip"],
  ["left_hip", "left_knee"], ["left_knee", "left_ankle"],
  ["right_hip", "right_knee"], ["right_knee", "right_ankle"],
];
let thresholds = { down: null, up: null };

function onLandmarks(data) {
  setHealth("h-pose", data.poseDetected ? "in frame" : "no pose", data.poseDetected ? "ok" : "warn");
  const angle = data.angle;
  $("c-angle").textContent = angle == null ? "—" : `${Math.round(angle)}°`;
  if (angle != null && thresholds.down != null) {
    $("c-phase").textContent =
      angle < thresholds.down ? "DOWN" : angle > thresholds.up ? "UP" : "between";
  }
  if (Array.isArray(data.measuredJoints)) $("c-joints").textContent = data.measuredJoints.join(" · ");

  // Landmark points are [x, y, visibility] with x/y normalized 0..1
  // (mirrors app/src/pose.ts).
  const points = data.landmarks || {};
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "#2a4a33";
  ctx.lineWidth = 3;
  for (const [a, b] of BONES) {
    const pa = points[a], pb = points[b];
    if (!pa || !pb) continue;
    ctx.beginPath();
    ctx.moveTo(pa[0] * canvas.width, pa[1] * canvas.height);
    ctx.lineTo(pb[0] * canvas.width, pb[1] * canvas.height);
    ctx.stroke();
  }
  const measured = new Set(data.measuredJoints || []);
  for (const [name, p] of Object.entries(points)) {
    ctx.fillStyle = measured.has(name) ? "#6ee787" : "#3a4f3d";
    ctx.beginPath();
    ctx.arc(p[0] * canvas.width, p[1] * canvas.height, measured.has(name) ? 7 : 4, 0, Math.PI * 2);
    ctx.fill();
  }
}

async function loadPrescriptionAndConfig() {
  try {
    const { items } = await rpc("query_history", {
      kind: "event", types: ["workout_prescribed"], limit: 500,
    });
    const last = items.at(-1);
    if (last) onHubEvent(last);
  } catch { /* no history yet */ }
  try {
    const { config } = await rpc("get_metric_config", { metricId: "workout" });
    const exercise = config.exercise || {};
    thresholds = { down: exercise.downBelow ?? null, up: exercise.upAbove ?? null };
    if (thresholds.down != null) $("t-down").value = thresholds.down;
    if (thresholds.up != null) $("t-up").value = thresholds.up;
    const camera = config.camera || {};
    $("c-camera").textContent = camera.id ? `${camera.id} (rotate ${camera.rotate ?? 0}°)` : "—";
    $("t-status").textContent = "";
  } catch {
    $("t-status").textContent = "no active workout metric — thresholds appear during a set";
  }
}

$("t-save").onclick = async () => {
  try {
    const down = Number($("t-down").value);
    const up = Number($("t-up").value);
    await rpc("update_metric_config", {
      metricId: "workout",
      config: { exercise: { downBelow: down, upAbove: up } },
    });
    thresholds = { down, up };
    $("t-status").textContent = `saved ${down}/${up} to the running metric`;
  } catch (err) {
    $("t-status").textContent = `save failed: ${err.message}`;
  }
};

// ---- history tab ---------------------------------------------------------
function eventLine(item) {
  const li = document.createElement("li");
  const time = document.createElement("time");
  time.textContent = (item.timestamp || "").replace("T", " ").slice(5, 19);
  const label = document.createElement("span");
  label.textContent = item.status ? `${item.type} (${item.status})` : item.type;
  if (item.type === "override_used" || item.type === "system_error" || item.status === "failed") {
    label.className = item.type === "override_used" ? "warn" : "bad";
  }
  li.append(time, label);
  return li;
}

function prependEvent(event) {
  const list = $("hist-events");
  if (list.firstChild?.classList?.contains("muted")) list.textContent = "";
  list.prepend(eventLine(event));
}

async function refreshHistory() {
  try {
    const workouts = await rpc("query_history", {
      kind: "event", types: ["workout_completed", "weight_logged"], limit: 200,
    });
    render("hist-workouts", workouts.items.reverse());
    const overrides = await rpc("query_history", {
      kind: "event", types: ["override_used", "system_error"], limit: 200,
    });
    const failures = await rpc("query_history", { kind: "action_result", status: "failed", limit: 200 });
    render("hist-problems", [...overrides.items, ...failures.items].sort(
      (a, b) => String(b.timestamp).localeCompare(String(a.timestamp)),
    ));
    const events = await rpc("query_history", { kind: "event", limit: 300 });
    render("hist-events", events.items.reverse());
  } catch { /* not connected yet */ }
}

function render(id, items) {
  const list = $(id);
  list.textContent = "";
  if (!items.length) {
    const li = document.createElement("li");
    li.className = "muted";
    li.textContent = "nothing yet";
    list.append(li);
    return;
  }
  for (const item of items) list.append(eventLine(item));
}

showTab();
connect();
