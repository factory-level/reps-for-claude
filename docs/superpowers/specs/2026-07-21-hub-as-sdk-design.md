# reps-for-claude on usb-mcp-hub — Vision SDK Adoption Design

Date: 2026-07-21 (amended same day after codebase exploration and user review)
Status: Approved in brainstorming
Companion documents:
- [Tauri rewrite design](2026-07-19-tauri-rewrite-design.md) (this spec supersedes its §"Python vision sidecar" and the JSON-lines sidecar protocol)
- usb-mcp-hub [product spec](../../../../usb-mcp-hub/docs/product-spec.md) and [MVP roadmap](../../../../usb-mcp-hub/docs/superpowers/specs/2026-07-20-camera-mcp-mvp-roadmap-design.md)

## Decision

reps-for-claude adopts **usb-mcp-hub as its vision SDK**. The hub provides
generic vision infrastructure; reps-for-claude ships the workout model **as a
plugin into that SDK** and consumes semantic events through a client API.

Decisions made during brainstorming and subsequent review:

- **Relationship:** full adoption — hub is the SDK, not an optional backend.
- **Deployment:** bundled — reps-for-claude ships hubd and spawns it as a
  managed child process. The lock screen never depends on an externally
  managed service.
- **Fidelity:** the hub grows a full-rate **streaming evaluation mode**;
  reps-for-claude does not regress to snapshot-cadence polling.
- **Privacy:** hub processes stay warm for the whole reps session; the camera
  device is opened only while a metric is enabled (**camera gating**), i.e.
  only between lock and unlock.
- **API surface:** reps' Rust core consumes a first-class local
  WebSocket/REST **client API**. The hub's Runtime MCP server remains exposed
  alongside it for Claude/agents; the product loop does not route through MCP.
- **Generic/specific split:** the hub contains **no pose or workout code**.
  All workout-specific knowledge (squat/bench/jump-rope/stretch definitions,
  rotation/capacity programming) is packaged by reps-for-claude.
- **The model is a plugin, shipped from reps-for-claude:** the SDK defines a
  formal plugin contract and registry; reps' existing `vision/` package is
  refactored **in place, in the reps repo** into a hub-conformant plugin.
  `vision/` is *not* deleted (amends the original draft of this spec). The
  hub's own `vision/plugins/pose_reps/` and hubd's TypeScript `repCounter.ts`
  are retired once reps' plugin is the source of truth.
- **The SDK ships the snapshot tuning app:** the mobile PWA served by hubd is
  a first-class SDK deliverable — a generic phone app for capturing
  snapshots, previewing live detections from the enabled plugin/metric, and
  interactively tuning metric configuration. reps users calibrate their
  exercise configs (camera placement, angle thresholds) with it before those
  configs drive the workout loop.

## Architecture

### Process model

```
reps-for-claude (Tauri)
└── Rust core
    ├── hub supervisor  ──spawns/monitors──►  hubd (Node)
    │                                         ├── vision-host (Python)
    │                                         │   └── loads reps' plugin (from reps repo/bundle)
    │                                         └── go2rtc (deferred; not in MVP)
    └── hub client (WebSocket/REST) ◄──landmarks, progress, events, health──┘

phone (snapshot tuning app, served by hubd on the LAN)
    ◄── live detection preview / config tuning ──► hubd client API
```

- The Rust core supervises **only `hubd`**; `hubd` supervises `vision-host`.
  One child from reps' perspective, with the drop-safe lifecycle guarantees
  the debug sidecar spawn has today.
- hubd + vision-host launch when the reps app starts and stay up for the
  session. Startup cost is paid once, not on every lock.

### The SDK (usb-mcp-hub) — generic infrastructure only

1. **Plugin contract + registry.** vision-host defines the plugin interface —
   `describe_capabilities()` (including a config schema: parameter names,
   types, ranges, so tooling can render controls generically),
   `configure(params)`, `evaluate(frame)`, and the streaming entries
   `start_stream(config)` / `stop_stream()` — and loads plugins from a
   configured path/entry point instead of a hardcoded import. Multiple
   plugins can coexist; metrics select a plugin by name.
2. **Streaming evaluation mode.** A plugin may own a camera at full frame
   rate and push landmarks + progress over the vision-host WebSocket, instead
   of snapshot-cadence polling. This preserves reps' live skeleton overlay,
   jump-rope counting, and instant per-rep feedback, and becomes a platform
   capability any plugin can use.
3. **Client API.** A versioned local WebSocket/REST surface for product
   consumers:
   - `enable_metric(metric_id, params)` — params carry the plugin name and
     its configuration (for reps: the exercise definition, target, camera
     device selection)
   - `disable_metric(metric_id)`
   - subscriptions: landmark stream, progress events, semantic events
     (`rep_completed`), camera/plugin health
   - `simulate(event)` for developer mode
4. **Camera gating.** Enabling a metric opens the camera device; disabling it
   releases the device. Verifiable externally (no process holds `/dev/video*`
   while no metric is enabled). This is how "camera on only while locked"
   survives the move to a long-running daemon.
5. **Snapshot tuning app.** The phone PWA, generalized: live observation
   preview and config tuning for whatever plugin/metric is enabled, rendered
   from the plugin's declared config schema. No workout knowledge baked in.

### The plugin (shipped from reps-for-claude)

reps' `vision/` package refactors into a plugin implementing the SDK
contract. Internally it keeps a generic-engine vs. config-data split:

- **Generic engine:** pose estimation (`PoseEstimator`, MediaPipe Tasks —
  the ML model, its file management and variant selection stay internal to
  the plugin behind the detector seam: "users bring plugins, not models"),
  angle geometry, the parameterized `RepStateMachine`, and the activity
  engines (rep counting, motion-streak with grace period, timed hold).
- **Shipped configuration:** the workout definitions — squat/bench/etc. as
  joint triples + thresholds, jump rope's bounce/grace parameters, stretch
  hold times — packaged in reps-for-claude and passed to the plugin via
  `enable_metric`/`configure`. The rotation/capacity programming model stays
  in reps' Rust core.

The plugin's existing pytest suite (76 tests, pure core importable without
CV dependencies) moves with the refactor and must stay green.

### reps-for-claude app changes

1. **Vision trait (new).** The Rust core gains a trait abstraction for the
   vision backend (exploration confirmed none exists today — only
   `Clock`/`FakeClock`): enable/disable metric, subscribe to
   landmarks/progress/events/health, simulate. A fake implementation serves
   engine tests; the real implementation is the hub client.
2. **Hub supervisor.** Spawns bundled hubd with config, health checks, one
   auto-restart, drop-safe kill on exit/panic.
3. **Hub client.** WebSocket/REST client translating the reps loop:
   lock → `enable_metric`; landmark frames → Operator/Gym TV overlay;
   progress → live rep count into `Session::report_progress` (replacing the
   simulate-only path); unlock/abort → `disable_metric`.
4. **Packaging.** reps-for-claude bundles a pinned hub release (hubd,
   vision-host environment, the reps plugin) in its installer. The hub stays
   a private monorepo; "SDK" means the versioned client API + plugin
   contract plus the bundled artifact.

## Data flow

```
[calibration, any time]  phone tuning app ◄─► hubd client API ◄─► plugin preview

lock fires
→ core: enable_metric("rep_metric", {plugin: "reps_vision", exercise config, target, camera})
→ plugin opens webcam, streams at frame rate:
    landmarks  → core → Operator / Gym TV overlay
    progress   → core → live count, per-rep feedback
    rep_completed → hub event bus → webhooks / Runtime MCP (agents)
→ target met → weight logged → unlock
→ core: disable_metric → camera released
```

## Failure handling

| Failure | Response |
|---|---|
| hubd crash while locked | Supervisor restarts once; on second failure, honor-mode "press Done" fallback, logged `verified = false`. Never stranded, never phantom reps. |
| vision-host or plugin crash | hubd's internal supervision restarts it; reps sees a health blip, shows "reconnecting" on the overlay. |
| Camera absent / dies mid-set | Honor-mode fallback (unchanged). |
| Core crash/panic while locked | Drop guards release xsecurelock **and kill hubd** (camera cannot stay hot). |
| Hub API version mismatch | Startup check; clear error, refuse to guess. |

## Testing

- **Hub:** existing unit/integration/e2e suites stay green; gains plugin
  registry, streaming-mode, camera-gating, and client API tests.
- **Plugin:** reps' 76-test pytest suite survives the refactor; new tests for
  the plugin contract surface (fake host, config schema).
- **reps Rust engines:** unit tests against a fake vision-trait
  implementation (fake clock, fake locker unchanged).
- **Contract tests:** recorded WebSocket transcripts (landmark frames,
  progress, events) replayed against both the hub server and the Rust
  client, pinning the client API version — the primary guard against hub
  churn.
- **End to end:** reps' e2e spawns the real bundled hub with the real plugin;
  latency budget assertion (landmark frame end-to-end < 50 ms) so jump rope
  stays reliable; one manual script per milestone on the target machine.

## Risks

- **Platform maturity.** The hub is days old; its API will churn. Mitigation:
  versioned client API + contract tests; reps pins a hub release and
  upgrades deliberately.
- **Cross-repo plugin loading.** The plugin lives in a different repo from
  the host that loads it. Mitigation: the plugin contract is versioned with
  the client API; the bundle pins both; CI in reps runs the plugin against
  the pinned hub.
- **Coupled schedules.** Hub SDK capabilities block reps' migration steps.
  Accepted consciously: reps-for-claude is the hub's flagship use case and
  drives its roadmap.

## Migration order

1. **Hub:** plugin contract + registry + external plugin loading; retire the
   hardcoded `pose_reps` import path.
2. **Hub:** streaming evaluation mode, versioned client API, camera gating;
   generalize the phone PWA into the snapshot tuning app; retire
   `repCounter.ts` (rep logic moves behind the plugin boundary).
3. **reps:** refactor `vision/` into the SDK plugin (contract surface +
   config-driven exercise definitions), keeping all tests green.
4. **reps:** vision trait + hub supervisor + hub client in the Rust core;
   wire progress into the state machine; contract tests; Operator view fed
   by the landmark stream; bundle the pinned hub artifact.
5. **Both:** e2e on the target machine with latency budget; then point
   Claude at the Runtime MCP and confirm workout events are visible.

The lock driver itself remains a separate reps-for-claude milestone (per the
Tauri rewrite spec); this migration only requires the vision trait to be
ready for it.

## Implementation notes (2026-07-21, migration executed)

Deviations and refinements recorded as built:

- **Observation taxonomy:** every plugin observation is declared in
  `observationSchema` as an `event` (`rep_completed`, `target_reached`,
  `stream_ended`), a `duration` (time spent doing the action), or a min/max
  `range` (joint angle 0–180). Push envelopes carry `cameraId` — metrics,
  events, and snapshot actions are addressable per camera.
- **`SPECS` retained as library data:** the plugin itself is fully
  config-driven (`ExerciseSpec.from_config`; nothing in `hub_plugin` reads
  `SPECS`), but the registry stays for the remaining library modules
  (`video.py`, `detector.py`) and their tests. It no longer feeds the
  product path — `app/src-tauri/resources/exercise_specs.json` (which also
  declares the model reps-for-claude uses) is the source of truth.
- **DebugPanel / `stream.py` deferred, not deleted:** the hub landmark
  stream carries no JPEG frames, so migrating the video-debug view onto it
  would regress its core value (annotated video playback). Both stay as
  dev-only tooling until the hub grows frame streaming.
- **Bundle layout:** `scripts/bundle-hub.sh` stages
  `resources/hub-bundle/{hubd.mjs, public/, vision/}` (esbuild single file;
  `HUB_VISION_DIR`/`HUB_PUBLIC_DIR`/`HUB_CERT_DIR` env overrides) and pins
  `resources/hub-manifest.json` (hub commit, apiVersion, artifact +
  transcript sha256s). The reps plugin ships from this repo's `vision/src`
  via `HUB_PLUGIN_ARGS`; the Python env is provisioned by uv on first run.
  The staged bundle is gitignored; the manifest is committed.
- **Verification results:** full-pipeline e2e (`scripts/e2e-latency.mjs`)
  counts the squat fixture's 2 reps through real MediaPipe with landmark
  latency p50 ≈ 17 ms / p95 ≈ 20 ms — inside the 50 ms budget. Camera
  gating is verifiable with `usb-mcp-hub/scripts/verify-camera-gating.sh`;
  the remaining manual checks live in
  `docs/checklists/e2e-target-machine.md`.
