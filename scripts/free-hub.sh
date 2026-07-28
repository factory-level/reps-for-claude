#!/usr/bin/env bash
# Free the vision hub: kill any leaked hubd + vision-host and release port 8443.
#
# The reps-for-claude app supervises its OWN hubd (it spawns `pnpm --filter
# @hub/hubd start` and connects to it). When `tauri dev` hot-reloads or the app
# is hard-killed, that supervised hubd can leak and keep port 8443 bound — the
# next launch then times out waiting for HUBD READY and silently falls back to
# honor mode (camera never opens → "nothing is detected"). Run this to clear it.
set -uo pipefail

PORT="${HUB_PORT:-8443}"
echo "freeing vision hub (port ${PORT})…"

# Kill by port first (the authoritative holder), then by process signature.
if command -v fuser >/dev/null 2>&1; then
  fuser -k "${PORT}/tcp" 2>/dev/null || true
fi
pkill -f "tsx src/index.ts"   2>/dev/null || true
pkill -f "@hub/hubd"          2>/dev/null || true
pkill -f "host.server"        2>/dev/null || true

sleep 1
if ss -ltn 2>/dev/null | grep -q ":${PORT} "; then
  echo "  ⚠ port ${PORT} still in use — check: ss -ltnp | grep ${PORT}"
  exit 1
fi
echo "  ✅ port ${PORT} free — you can relaunch: (cd app && npm run tauri dev)"
