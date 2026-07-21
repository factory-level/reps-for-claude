# Implementation Plan: reps-for-claude adopts usb-mcp-hub as its Vision SDK

Date: 2026-07-21
Spec: [`../specs/2026-07-21-hub-as-sdk-design.md`](../specs/2026-07-21-hub-as-sdk-design.md)
Repos: `usb-mcp-hub` (hub) and `reps-for-claude` (reps), siblings under `~/Projects/agentic-vision/`.

## Guiding principles

- **Hub stays generic.** No pose/workout knowledge in hub code after M2. Hub
  keeps a trivial generic demo plugin (`frame_stats`) so it remains
  self-testable and the tuning app works standalone.
- **Plugin ships from reps.** `reps-for-claude/vision/` is refactored in place
  into a hub-conformant plugin; never deleted or ported into the hub.
- **The phone PWA is a first-class deliverable** — the generic snapshot tuning
  app. `/ws/session` evolves; it is not frozen.
- **Both test suites green after every milestone.** Legacy paths (`pose_reps`,
  `repCounter.ts`, `stream.py`, DebugPanel) retire only when their replacement
  passes tests.
- go2rtc stays deferred. The lock driver is out of scope (M5 only proves the
  trait is ready for it).

## Protocol and interface reference (target end state)

### A. vision-host internal protocol v2 (hubd ↔ Python, existing WS)

Requests/responses keep today's id-correlated framing
(`{"id","method","params"}` → `{"id","result"|"error"}`). **Push messages are
frames without an `id`, discriminated by a `stream` key** — the existing TS
client's dispatch change is ~3 lines and old/new clients never misparse each
other.

```jsonc
// request methods (all gain "pluginId"; omitted pluginId routes to the sole plugin for back-compat)
{"id":1,"method":"describe_capabilities","params":{}}
  // → {"id":1,"result":{"protocolVersion":"2.0","plugins":{"reps_vision":{"supports":["evaluate","stream"],"configSchema":{...},"observationSchema":{...}}}}}
{"id":2,"method":"configure","params":{"pluginId":"reps_vision","config":{...}}}   // → effective config echoed
{"id":3,"method":"evaluate","params":{"pluginId":"reps_vision","framePath":"/tmp/.../frame.jpg"}}
{"id":4,"method":"start_stream","params":{"pluginId":"reps_vision","config":{"camera":{"source":"index","value":0,"width":640}}}}
{"id":5,"method":"stop_stream","params":{"pluginId":"reps_vision"}}

// push frames (no id)
{"stream":"landmarks","pluginId":"reps_vision","seq":417,"tsUs":1789000000000,"data":{"poseDetected":true,"landmarks":{"left_knee":[0.5,0.6,0.98]},"measuredJoints":["left_hip","left_knee","left_ankle"],"angle":97.2}}
{"stream":"progress","pluginId":"reps_vision","seq":418,"tsUs":0,"data":{"value":4,"unit":"reps","satisfied":false}}
{"stream":"event","pluginId":"reps_vision","seq":419,"tsUs":0,"data":{"type":"rep_completed","count":5}}
```

`tsUs` = epoch microseconds stamped at frame capture (`time.time_ns()//1000`)
— comparable across processes on one machine; the M5 latency budget measures
against it.

### B. Python plugin contract (new in hub: `vision/host/contract.py`)

```python
class VisionPlugin(ABC):
    plugin_id: str
    def describe_capabilities(self) -> dict: ...        # MUST include configSchema
    def configure(self, params: dict) -> dict: ...      # returns effective config
    def evaluate(self, frame_path: str) -> dict: ...    # single-frame; used by tuning app
    def start_stream(self, config: dict, emit: Emit) -> None: ...  # non-blocking; plugin owns camera + thread
    def stop_stream(self) -> None: ...                  # MUST release camera before returning

Emit = Callable[[str, dict], None]   # emit(stream_name, data) — host-provided, thread-safe
```

Deliberate deviation from the spec's `start_stream(config)`: the host injects
`emit` (bridged via `loop.call_soon_threadsafe` onto the websockets asyncio
loop) because the plugin's camera loop runs in its own thread. Camera-gating
semantics live in the contract docstring: *the capture device is opened inside
`start_stream` and closed by `stop_stream`/stream-end — never at import,
construction, or configure time.*

**configSchema** (drives the tuning app; deliberately a flat field list, not
full JSON Schema, so the PWA renders controls with no library):

```jsonc
"configSchema": {"fields": [
  {"name":"activity","type":"enum","values":["lift","jumprope","stretch"],"default":"lift"},
  {"name":"spec.down_below","type":"number","min":0,"max":180,"step":1,"default":110,"label":"Down threshold (°)"},
  {"name":"spec.up_above","type":"number","min":0,"max":180,"step":1,"default":160},
  {"name":"spec.min_visibility","type":"number","min":0,"max":1,"step":0.05,"default":0.5},
  {"name":"reset_after","type":"number","min":0,"max":10,"step":0.5,"default":2.0,"label":"Jump-rope grace (s)"}
]}
```

### C. hubd client API v1 (Rust core + tuning app ↔ hubd)

- WS `GET /v1/ws` and REST `GET /v1/health`, registered in `buildServer` so
  they exist on **both** listeners: localhost:8081 (reps' Rust client) and TLS
  0.0.0.0:8443 (tuning app). Same id-correlated framing + `stream` pushes as
  protocol A — one mental model; contract-test transcripts exercise one shape.
- On connect, server sends `{"type":"hello","apiVersion":"1.0","hubVersion":"<git sha>","capabilities":{...}}`.
  Clients MUST refuse to proceed on major-version mismatch.
- Methods: `enable_metric {metricId, pluginId, config}` (config carries
  activity + full exercise spec + target + camera source),
  `disable_metric {metricId}`, `update_metric_config {metricId, config}`
  (tuning app live-tweaks → plugin `configure`),
  `subscribe {streams:["landmarks","progress","event","health"]}` /
  `unsubscribe`, `simulate {metricId, event}`, `describe`, `health`.
- Pushes: `{stream, metricId, seq, tsUs, data}` fanned out to subscribers;
  `health` pushes on vision-host restart / camera state change
  (`{"visionHost":"up"|"restarting","camera":"open"|"closed","enabledMetrics":[...]}`).
- `/ws/session` end state: pure **frame ingest** — binary JPEG up, per-frame
  observation JSON down (`{"type":"observation","metricId","data":<evaluate result>}`);
  all control happens over `/v1/ws`. No metric enabled → `{"type":"idle"}`.

### D. Rust trait (created new — no sidecar trait exists today)

New workspace crate `app/src-tauri/hub-client/` (engine crate stays pure).
**No tokio** — blocking `tungstenite` + reader thread + `std::sync::mpsc`,
matching the app's existing thread-based style.

```rust
pub enum VisionEvent {
    Landmarks(serde_json::Value),                            // passed through to UI overlay
    Progress { value: f64, unit: String, satisfied: bool },  // maps 1:1 onto engine::types::Progress
    Semantic { kind: String, payload: serde_json::Value },
    Health(HubHealth),
    ConnectionLost,
}

pub trait VisionHub: Send {
    fn enable_metric(&mut self, req: &EnableMetric) -> Result<(), HubError>;
    fn disable_metric(&mut self, metric_id: &str) -> Result<(), HubError>;
    fn update_metric_config(&mut self, metric_id: &str, config: &serde_json::Value) -> Result<(), HubError>;
    fn simulate(&mut self, metric_id: &str, event: &serde_json::Value) -> Result<(), HubError>;
    fn health(&mut self) -> Result<HubHealth, HubError>;
    fn take_receiver(&mut self) -> Option<mpsc::Receiver<VisionEvent>>;
}
```

Impls: `FakeHub` (scripted events), `HubClient` (real WS), plus
`HubSupervisor` (owns the hubd child + a `Box<dyn VisionHub>`; restart-once;
`Drop` kills the child — camera cannot stay hot).

### E. Exercise specs → reps-shipped JSON config

**JSON shipped by Rust.** New `app/src-tauri/resources/exercise_specs.json`,
embedded with `include_str!`, validated at startup:

```jsonc
{"version":1,"exercises":{
  "squat":   {"activity":"lift","joints":["hip","knee","ankle"],"downBelow":110,"upAbove":160,"minVisibility":0.5},
  "bench":   {"activity":"lift","joints":["shoulder","elbow","wrist"],"downBelow":95,"upAbove":150},
  "jumprope":{"activity":"jumprope","targetSeconds":60,"bounceThreshold":0.015,"resetAfter":2.0},
  "stretch": {"activity":"stretch","holdSeconds":30}
}}
```

`exercises.py` gains `ExerciseSpec.from_config(dict)`; the hardcoded `SPECS`
registry survives until `stream.py` retires (M4), then shrinks to test-fixture
data. The Python plugin engine never hardcodes an exercise name.

## M1 — Hub SDK core (usb-mcp-hub only; reps untouched)

Ordered so the phone demo works unmodified until task 6, degraded-legacy after.

1. **Plugin contract + registry + external loading** — `vision/host/contract.py`
   (ABC above), `vision/host/registry.py` (`load_plugins(specs) -> dict[str, VisionPlugin]`).
   Loading: CLI args `--plugin-path DIR` (repeatable, appended to `sys.path`) +
   `--plugin module:ClassName` (repeatable). Rejected: entry-points (would force
   pip-installing the reps plugin into the hub's uv env) and bare `-m` (no class
   name, no multi-plugin). hubd forwards these args from config (task 4).
   `vision/host/server.py`: handler over a registry; `pluginId` param on all
   methods, defaulting to the sole plugin when exactly one is loaded. Default
   plugin remains `pose_reps` for now. Tests: extend
   `vision/tests/test_server.py` FakePlugin to the new contract; registry/loader
   tests (load a FakePlugin from a tmp dir via `--plugin-path`).
2. **Streaming mode in the host** — thread-safe `emit` bridging, broadcast of
   push frames to all connected WS clients, `seq`/`tsUs` stamping, stream
   lifecycle state per plugin (reject double `start_stream`; `stop_stream`
   idempotent). Add `vision/plugins/frame_stats/` — a tiny generic demo plugin
   (mean brightness + motion delta, both modes, opens `cv2.VideoCapture` in
   `start_stream`) so hub e2e and the tuning app never depend on reps, and
   camera-gating tests have a subject. Tests: streaming FakePlugin test
   asserting push framing + gating (capture factory injected; open happens in
   start_stream, release in stop_stream).
3. **TS client v2** — `apps/hubd/src/visionClient.ts`: public
   `describeCapabilities/configure/evaluate/startStream/stopStream`; add
   `onStream(handler)` dispatching frames with a `stream` key; keep
   `evaluate(framePath)` signature so `server.ts` compiles untouched. Extend
   `apps/hubd/test/visionClient.test.ts` ws-stub with pushed frames.
4. **Client API + metric registry in hubd** — new `apps/hubd/src/clientApi.ts`
   (routes `/v1/ws`, `/v1/health`, hello frame, subscription fan-out) and
   `apps/hubd/src/metrics.ts` (MetricRegistry: `enable_metric` → `configure` +
   `start_stream`; `disable_metric` → `stop_stream`; **camera-gating invariant
   lives here**: no enabled metrics ⇒ every plugin stopped). Register in
   `buildServer` so both listeners get it. Bridge plugin `event` pushes onto
   the existing `EventBus`. New `apps/hubd/test/clientApi.test.ts` with a fake
   VisionClient.
5. **Supervision + handshake** — `visionHost.ts`: restart vision-host on
   unexpected exit (once, short backoff; `health` push + bus event;
   re-`configure`+`start_stream` enabled metrics after restart). `index.ts`:
   print machine-readable `HUBD READY {"port":8443,"debugPort":8081,"apiVersion":"1.0"}`
   for reps' supervisor handshake.
6. **PWA → tuning app, plumbing phase** — `server.ts` `/ws/session`: if a
   metric is enabled, route frames to that plugin's `evaluate` and return
   generic observations; else fall back to today's pose_reps + `RepCounter`
   path (legacy fallback, deleted in M2). `public/app.js` + `index.html`:
   connect `/v1/ws`, render capability list, enable/disable metric buttons,
   generic observation readout; keep the frame capture loop as-is.
7. **Tuning UI** — generic form renderer over `configSchema` fields →
   `update_metric_config` on change; live observation/angle display for
   calibration. May slip into M2 without blocking.

Exit: hub pytest + vitest + e2e green; phone demo works (legacy fallback);
`frame_stats` streamable from the tuning app.

## M2 — reps plugin (reps repo + small hub deltas)

1. **Plugin package** — new `vision/src/reps_vision/hub_plugin/` (`plugin.py`,
   `stream_loop.py`). `configure()` builds the activity from passed config —
   `LiftActivity(ExerciseSpec.from_config(...))` /
   `JumpRopeActivity(target_seconds, bounce_threshold, reset_after)` /
   `StretchActivity` — no `SPECS` lookups. `describe_capabilities()` returns
   the configSchema above. All cv2/mediapipe imports stay lazy (pure core
   preserved; hub_plugin unit-testable with fake estimator/capture factories,
   reusing `video.py`'s injection pattern).
2. **Streaming loop** — `stream_loop.py`: thread, `cv2.VideoCapture` with
   camera config `{"source":"index","value":0}` **and**
   `{"source":"file","value":"/path.webm"}` (files reuse committed fixtures and
   later let DebugPanel ride the hub path); per frame:
   `PoseEstimator.landmarks` → `activity.update(landmarks, now)` →
   `emit("landmarks"/"progress"/"event")` (`rep_completed` on count increment);
   `stop_stream` sets a threading.Event, joins, releases capture. `pose.py`:
   add an IMAGE-mode/monotonic-timestamp option for single-frame `evaluate`.
3. **exercises.py** — add `from_config`; keep `SPECS`/`get_spec` (stream.py +
   76 tests still depend on them). Add `app/src-tauri/resources/exercise_specs.json`
   now with values copied from `SPECS`.
4. **Hub loads the reps plugin (dev)** — `HUB_PLUGIN_PATHS`/`HUB_PLUGINS`
   env/config documented in hub README; dev invocation:
   `--plugin-path ../reps-for-claude/vision/src --plugin reps_vision.hub_plugin:RepsVisionPlugin`.
   Runtime env is hub's uv env (Python 3.12 satisfies both).
   **Declare `opencv-python` explicitly in hub `vision/pyproject.toml`.**
   One-time deliberate diff of hub `pose_reps/angles.py` (acos) vs reps
   `angles.py` (atan2) to confirm no fix is lost — all that remains of the
   spec's fork-reconciliation.
5. **Retire hub workout code** — delete `vision/plugins/pose_reps/`,
   `apps/hubd/src/repCounter.ts`, the `/ws/session` legacy fallback, and their
   tests; hub e2e + tuning-app default switch to `frame_stats`. Tuning app now
   renders reps' real configSchema; finish tuning UI here if M1.7 slipped.
6. **stream.py / DebugPanel** — keep both untouched through M2–M3; migrate
   DebugPanel onto the hub file-source path in M4, then retire `stream.py`,
   `test_stream.py`, and demote `SPECS` to fixtures.

Exit: reps' 76 tests green plus new hub_plugin tests; hub suites green with
and without the reps plugin; tuning app calibrates squat thresholds end-to-end
against the reps plugin.

## M3 — reps Rust integration

1. **`app/src-tauri/hub-client/` crate** — trait + types (§D), `fake.rs`,
   `client.rs` (blocking tungstenite; reader thread: hello → version check →
   pushes to mpsc; request/response correlation with timeout), `supervisor.rs`
   (spawn hubd — dev: via `$HUB_DIR`, default `../../usb-mcp-hub`; prod:
   bundled paths from M4 — parse `HUBD READY`, poll `/v1/health`, restart-once
   with `ConnectionLost` in between, second failure → `HubHealth::Failed` →
   honor-mode; `Drop` kills child). New deps: `tungstenite`, `serde`,
   `serde_json`.
2. **Wire into `lib.rs`** — `SharedHub`; on `begin_workout` (and later lock):
   look up exercise in embedded `exercise_specs.json` →
   `enable_metric("workout", {...})`; event-pump thread: `Progress` →
   `session.report_progress` (replacing simulate-only; `simulate_progress`
   stays for dev), `Landmarks` → Tauri `vision-landmarks` event,
   `Semantic`/`Health` → `vision-event`; on `confirm_weight`/`resume_coding`/
   session end → `disable_metric`. Honor-mode: `HubHealth::Failed` →
   `vision-fallback` event; Operator "Done (honor)" button → new
   `honor_complete` command → `report_progress(satisfied)`. Add
   `verified: bool` to `SetRecord` in `engine/src/store.rs` + `types.rs`.
3. **Contract tests** — canonical transcripts live in the hub:
   `apps/hubd/test/contracts/v1/*.json`
   (`[{"dir":"c2s"|"s2c","msg":{...}}]`; scenarios: hello+version check,
   enable→landmarks/progress/rep_completed→disable, simulate, health blip,
   unknown-metric error). Hub side: replay c2s against `buildServer` +
   FakeVisionClient, assert s2c. Reps side: transcripts vendored (hash-pinned
   by the M4 manifest) into `app/src-tauri/hub-client/tests/contracts/v1/`;
   test spins a local WS acceptor, feeds s2c, asserts requests and emitted
   `VisionEvent`s. Version-bump procedure: regenerate in hub → re-vendor.

Exit: `cargo test` green (engine untouched + hub-client unit/contract tests);
manual dev run against sibling hub counts real squats into the session.

## M4 — reps UX + bundling

1. **Operator view** — new `app/src/OperatorPanel.tsx`: skeleton overlay +
   live count from `vision-landmarks`/`vision-event` (canvas drawing done
   client-side, porting what `visualize.py` renders); reconnecting/honor-mode
   states.
2. **DebugPanel migration** — "via hub" mode: `enable_metric` with
   `camera:{"source":"file","value":<fixture>}` instead of spawning
   `reps_vision.stream`; once verified, delete `debug_stream_start/stop` +
   `SharedDebugProcess` from `lib.rs`, retire `stream.py` + `test_stream.py`,
   demote `SPECS` to `vision/tests/` fixtures.
3. **Bundling** (riskiest item — **prototype first**; Linux-only simplifies):
   - hubd: esbuild-bundle to single `hubd.mjs` (new hub script
     `apps/hubd/scripts/build-bundle.mjs`); ship pinned Node runtime as a
     Tauri resource. Rejected: `pkg` (deprecated), Node SEA (experimental).
   - Python: ship `uv` binary + locked requirements + hub `vision/` sources +
     reps `vision/src/reps_vision`; first-run provisioning into
     `$XDG_DATA_HOME/reps/hub-env` (`uv python install` + locked install).
     First-run network accepted and surfaced in UI; fully-offline env deferred.
   - `app/src-tauri/resources/hub-manifest.json`:
     `{hubCommit, apiVersion, artifactSha256s, transcriptSha256s}`; new
     `scripts/bundle-hub.sh` builds from the sibling checkout at the pinned
     commit and verifies hashes; supervisor cross-checks manifest `apiVersion`
     against the hello frame.
4. **Tuning-app product flow** — document calibrating via phone against the
   bundled hubd before workouts; tuned values manually copied into
   `exercise_specs.json` for MVP (persistence is an open question).

## M5 — e2e, latency, lock readiness, MCP

1. **Full-loop e2e** (`#[ignore]`d cargo integration test + manual script
   `docs/checklists/e2e-target-machine.md`): real supervisor → bundled hubd →
   vision-host → reps plugin with a file-source fixture; session reaches
   satisfied via real progress events.
2. **Latency budget**: landmark `tsUs` (stamped at `cv2.read`) vs Rust receive
   time; assert p95 < 50 ms over ≥300 frames. Camera-gating verification
   script (hub `scripts/verify-camera-gating.sh`): `fuser /dev/video0` empty
   while no metric enabled, non-empty while enabled.
3. **Lock readiness (not built here)**: the lock milestone calls exactly
   `enable_metric` on lock / `disable_metric` on unlock via `VisionHub`;
   honor-mode fallback and drop-kill already in place from M3.
4. **Runtime MCP (later)**: bus already carries `rep_completed`; MCP server is
   a hub roadmap item — note only.

## Test strategy summary

| Milestone | Suites kept green | New coverage |
|---|---|---|
| M1 | hub pytest (FakePlugin upgraded), all vitest, e2e (legacy fallback) | registry/loader, streaming push framing, gating via injected capture, clientApi, restart supervision |
| M2 | reps 76 pytest untouched; hub suites | hub_plugin unit (fake estimator/capture, no cv), frame_stats e2e, tuning-app-vs-configSchema |
| M3 | cargo engine tests untouched | hub-client unit (mock WS), FakeHub app tests, contract transcripts both sides |
| M4 | all three suites | bundle smoke script; DebugPanel-via-hub replaces test_stream coverage |
| M5 | all | e2e, latency assertion, gating script |

## Git / branching

- Hub: M1 branches off `feature/mobile-rep-counter`
  (`feature/sdk-m1-client-api`); merging to `main` is the user's call. One
  branch + PR per milestone afterwards (`feature/sdk-m2-retire-pose-reps`).
- Reps: `feature/hub-sdk-m2-plugin`, `-m3-rust`, `-m4-bundle` off `main`.
- Cross-repo coupling (M2.4/M2.5, M3 transcripts, M4 manifest): land the hub
  PR first, record its merge commit in reps (`hub-manifest.json` from M4;
  pinned-commit note in the PR before that). Sibling checkout always via
  `HUB_DIR` env, never hardcoded relative paths in committed code.

## Risks & open questions

- **Python env bundling** is highest-risk → prototype first in M4; accept
  first-run network for MVP.
- **Client API on the LAN listener** (needed by the tuning app): fine for MVP;
  hub backlog item — pairing token before anything beyond LAN-trusted use.
- **Spec-value drift** between `exercise_specs.json` (source of truth) and
  Python defaults: mitigated by demoting `SPECS` to fixtures in M4; until then
  a comment cross-references them.
- **Tuned-config persistence**: MVP is manual copy from the tuning app into
  JSON; later, a `store.rs`-backed override table + `update_metric_config`
  replay at enable time.
- **MediaPipe VIDEO-mode timestamps vs single-frame `evaluate`**: small
  `pose.py` mode option (M2.2); low risk, flagged early.
- **Latency**: JSON-encoding 33 landmarks/frame at 30 fps is the main cost; if
  p95 fails, decimate landmark pushes (progress/events stay full-rate) —
  decide only if the M5 assertion fails.
