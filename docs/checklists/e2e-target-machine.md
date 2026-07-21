# E2E checklist — target machine (hub-SDK migration M5)

Automated first (from the repo root, hub checkout as sibling or `HUB_DIR`):

- [ ] `node scripts/e2e-latency.mjs` → `E2E PASS`, p95 < 50 ms, reps == 2
      (full pipeline: hubd → vision-host → reps plugin → MediaPipe → client API)
- [ ] `./scripts/bundle-hub.sh` then `node scripts/e2e-latency.mjs --bundle`
      → same result through the staged bundle
- [ ] All suites green:
      `(cd vision && uv run python -m pytest)` ·
      `(cd app && pnpm vitest run && pnpm tsc --noEmit)` ·
      `(cd app/src-tauri && cargo test)` ·
      hub repo: `pnpm -r test` + `(cd vision && uv run python -m pytest)`
- [ ] Camera gating (webcam required):
      start hubd with the reps plugin, then
      `usb-mcp-hub/scripts/verify-camera-gating.sh 8081` → `CAMERA GATING OK`

Manual, real webcam + phone:

- [ ] `pnpm tauri dev` (hub auto-starts; `REPS_HUB_DISABLED=1` to opt out)
- [ ] Timer expires → Begin workout → Operator panel shows live skeleton,
      knee angle updates, squats count into the session, machine reaches
      weight confirmation without `simulate_progress`
- [ ] Kill hubd mid-set (`pkill -f hubd.mjs` or the pnpm process):
      one restart happens (health blip in Operator panel), set continues
- [ ] Kill it again: Operator panel offers **Done (honor)**; pressing it
      completes the set; `exercise_history` row has `verified = 0`
- [ ] Camera LED: on only between Begin workout and unlock — never during
      coding
- [ ] Phone tuning app (`https://<lan-ip>:8443/`): enable `reps_vision`,
      live angle readout, drag a threshold slider mid-preview, capture
      snapshots + description, Save draft
- [ ] Jump rope: prescription streams motion, duration accrues, 2 s pause
      resets the streak
