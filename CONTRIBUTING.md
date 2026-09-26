# Contributing to RFP

Thanks for taking a look. This is a desktop app with a Rust core, a Python
vision model and a Next.js site, so the useful thing to know first is which
part you are touching.

## Getting set up

```sh
pnpm install && (cd vision && uv sync)
./scripts/bundle-hub.sh          # stages the pinned vision hub
(cd app && pnpm tauri dev)       # hub auto-starts; REPS_HUB_DISABLED=1 opts out
```

You need Node ≥ 22.5, pnpm, [uv](https://docs.astral.sh/uv/), a Rust toolchain,
and a sibling checkout of
[usb-mcp-hub](https://github.com/factory-level/usb-mcp-hub) (the vision SDK).

## The test loop

```sh
(cd app && pnpm vitest run && pnpm tsc --noEmit)
(cd app/src-tauri && cargo test --workspace)
(cd vision && uv run pytest)
(cd web && pnpm test && pnpm typecheck)
```

**`--workspace` is load-bearing.** A bare `cargo test` silently skips the
`engine` and `hub-client` crates, which is where most of the logic lives.

Slow and hardware-dependent tests are excluded by marker and are opt-in on real
hardware: `(cd vision && uv run pytest -m cvvideo)`.

## When nothing is detected

Run `./scripts/free-hub.sh`. The app supervises its own `hubd`; a `tauri dev`
hot-reload can leak it, port 8443 stays bound, the next launch times out waiting
for the hub and silently degrades to honor mode. This is the fix roughly every
time.

## Changing anything the hub touches

RFP consumes usb-mcp-hub through a **pinned bundle**. After a hub change:

```sh
./scripts/bundle-hub.sh          # re-stage, re-vendor any transcripts it warns about
node scripts/e2e-latency.mjs --bundle
```

Bumping the hub's `API_VERSION` means bumping `API_VERSION` in `bundle-hub.sh`
too.

## Conventions

- `app/src-tauri/engine/` is **hub-free** on purpose. Keep it that way; it is
  the part that can be tested without a camera or a running hub.
- Designs and plans live in `docs/superpowers/{specs,plans}/` as
  `YYYY-MM-DD-topic.md`. Read the relevant spec before changing behavior.
- `docs/wiki/` is for human guides, `docs/architecture/` for what is actually
  built, `docs/design/` for intended behavior. Update wiki and architecture
  alongside behavior changes; change design when the intent changes.
- Verification that cannot be automated lives in
  `docs/checklists/e2e-target-machine.md`. Run it on the target machine before
  claiming a slice is done.
- Adding an exercise needs a known `activity` in
  `app/src-tauri/resources/exercise_specs.json` *and* support in `reps_vision`.
- Brand assets are generated, not hand-edited: change `scripts/make-brand.py`
  and re-run it, then `(cd app && pnpm tauri icon ../brand/icon.png)`.

## Pull requests

Keep them focused, describe what you verified and on what hardware, and note
anything you could not test locally. CI runs the detector and interface suites;
camera-dependent behavior is on you to check on a real rig.
