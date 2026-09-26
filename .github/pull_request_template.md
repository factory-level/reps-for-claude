## What this changes

<!-- One or two sentences. Link the issue if there is one. -->

## How it was verified

<!-- Which of these you actually ran, and on what hardware. -->

- [ ] `(cd app && pnpm vitest run && pnpm tsc --noEmit)`
- [ ] `(cd app/src-tauri && cargo test --workspace)`
- [ ] `(cd vision && uv run pytest)`
- [ ] `(cd web && pnpm test && pnpm typecheck)`
- [ ] Ran on a real rig with a camera
- [ ] `docs/checklists/e2e-target-machine.md`

## Notes

- [ ] This touches the hub contract, so I re-ran `./scripts/bundle-hub.sh` and
      `node scripts/e2e-latency.mjs --bundle`
- [ ] Docs updated (`docs/wiki/` for guides, `docs/architecture/` for as-built)

<!-- Anything you could not test locally, and why. -->
