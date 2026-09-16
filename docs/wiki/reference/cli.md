# Development commands

Run from the Reps repository unless another directory is specified.

```sh
cd app
pnpm tauri dev
```

The desktop supervises its bundled hub. `REPS_HUB_DISABLED=1` disables hub startup
for explicit fallback testing. The former Python `reps init` / `reps session`
commands are not the current desktop workflow.

Verification from the repository root:

```sh
(cd vision && uv run pytest)
(cd app && pnpm vitest run && pnpm tsc --noEmit)
(cd app/src-tauri && cargo test --workspace)
node scripts/e2e-latency.mjs --bundle --movement-contract
uvx --with mkdocs-material mkdocs serve
```

The last command previews the human guide, architecture and design sections. For packaging, offline checks and their
prerequisites, use [software readiness](../../production/software-readiness.md)
and [package verification](../../production/package-verification.md).
