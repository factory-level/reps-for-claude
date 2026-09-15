#!/usr/bin/env bash
# Start the local gym capture workspace; --check only validates prerequisites.
set -euo pipefail
REPS_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HUB_ROOT="$(cd "$REPS_ROOT/../usb-mcp-hub" && pwd)"
case "${1:-}" in
  ''|--check) ;;
  *) echo "Usage: bash scripts/capture-studio.sh [--check]" >&2; exit 2 ;;
esac
for program in node pnpm uv; do
  command -v "$program" >/dev/null || { echo "Missing prerequisite: $program" >&2; exit 1; }
done
node -e 'const [major,minor]=process.versions.node.split(".").map(Number); if(major<22 || (major===22 && minor<5)) { console.error("Node >=22.5 required"); process.exit(1); }'
[[ -f "$HUB_ROOT/apps/hubd/node_modules/tsx/dist/cli.mjs" ]] || {
  echo "Install hub dependencies first: cd $HUB_ROOT && pnpm install --frozen-lockfile" >&2
  exit 1
}
export HUB_DATA_DIR="${REPS_CAPTURE_DATA_DIR:-$HOME/.local/share/reps-gym-capture}"
export HUB_VISION_DIR="$HUB_ROOT/vision"
export HUB_BIND_HOST=127.0.0.1
export PORT="${REPS_CAPTURE_PORT:-8447}"
export DEBUG_PORT="${REPS_CAPTURE_STUDIO_PORT:-8087}"
export HUB_GO2RTC_API_PORT=1987
export HUB_RTSP_PORT=8557
export HUB_DEMO_WEBCAM="${REPS_CAPTURE_DEVICE:-/dev/video0}"
export HUB_DEMO_WEBCAM_ID=gym-c920
export HUB_DEMO_WEBCAM_ROTATE="${REPS_CAPTURE_ROTATE:-180}"
export UV_PROJECT_ENVIRONMENT="$HUB_DATA_DIR/vision-env"
export UV_FROZEN=1
export PYTHONDONTWRITEBYTECODE=1
export HUB_PLUGIN_ARGS_JSON
HUB_PLUGIN_ARGS_JSON="$(node -e 'console.log(JSON.stringify(["--plugin-path",process.argv[1],"--plugin","reps_vision.hub_plugin.plugin:RepsVisionPlugin"]))' "$REPS_ROOT/vision/src")"
unset HUB_PLUGINS_FILE HUB_EXIT_ON_STDIN_CLOSE
printf 'Studio: http://127.0.0.1:%s/studio.html\nData: %s\nCamera: %s; rotation: %s degrees\n' "$DEBUG_PORT" "$HUB_DATA_DIR" "$HUB_DEMO_WEBCAM" "$HUB_DEMO_WEBCAM_ROTATE"
if [[ "${1:-}" == --check ]]; then
  echo 'Prerequisites found. No server, camera, recording, or download started.'
  exit 0
fi
mkdir -p "$HUB_DATA_DIR"
echo 'Open the Studio URL after startup. Use Open camera preview, then Record set.'
echo 'Stop recording and close preview before Ctrl+C. Keep the data folder for review.'
echo 'First launch may provision locked Python dependencies with uv.'
cd "$HUB_ROOT"
exec pnpm --filter @hub/hubd start
