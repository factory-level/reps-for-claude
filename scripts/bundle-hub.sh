#!/usr/bin/env bash
# Stage the pinned usb-mcp-hub build into app/src-tauri/resources/hub-bundle/
# and write hub-manifest.json (commit + apiVersion + artifact hashes).
#
# The bundle layout the supervisor expects:
#   hub-bundle/
#     hubd.mjs          single-file hubd (node >= 22.5 — node:sqlite)
#     public/           snapshot tuning app assets
#     vision/           vision-host sources + pyproject + uv.lock (frame_stats included)
# The reps plugin is staged alongside hub-bundle in resources/reps-vision,
# and loaded via HUB_PLUGIN_ARGS. Python env is provisioned at first run
# by uv (network required once).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HUB_DIR="${HUB_DIR:-$REPO_ROOT/../usb-mcp-hub}"
OUT="$REPO_ROOT/app/src-tauri/resources/hub-bundle"
API_VERSION="$(sed -n 's/^export const API_VERSION = "\([^"]*\)";.*/\1/p' "$HUB_DIR/apps/hubd/src/clientApi.ts")"
[[ -n "$API_VERSION" ]] || { echo "cannot read hub API version" >&2; exit 1; }

if [[ ! -d "$HUB_DIR" ]]; then
  echo "usb-mcp-hub checkout not found at $HUB_DIR (set HUB_DIR)" >&2
  exit 1
fi

HUB_COMMIT="$(git -C "$HUB_DIR" rev-parse HEAD)"
HUB_DIRTY=false
if [[ -n "$(git -C "$HUB_DIR" status --porcelain)" ]]; then
  HUB_DIRTY=true
  echo "warning: staging a development bundle with uncommitted hub changes" >&2
fi

echo "building hubd bundle from $HUB_DIR @ ${HUB_COMMIT:0:12}"
(cd "$HUB_DIR/apps/hubd" && node scripts/build-bundle.mjs >/dev/null)

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$HUB_DIR/apps/hubd/dist/hubd.mjs" "$OUT/hubd.mjs"
cp "$REPO_ROOT/scripts/boot-hub.mjs" "$REPO_ROOT/app/src-tauri/resources/boot-hub.mjs"
rm -rf "$REPO_ROOT/app/src-tauri/resources/reps-vision"
mkdir -p "$REPO_ROOT/app/src-tauri/resources/reps-vision"
cp -r "$REPO_ROOT/vision/src/reps_vision" "$REPO_ROOT/app/src-tauri/resources/reps-vision/"
find "$REPO_ROOT/app/src-tauri/resources/reps-vision" -name __pycache__ -type d -exec rm -rf {} +
rm -rf "$REPO_ROOT/app/src-tauri/resources/models"
mkdir -p "$REPO_ROOT/app/src-tauri/resources/models"
MODEL_PATH="$(cd "$REPO_ROOT/vision" && PYTHONPATH=src .venv/bin/python -c 'from reps_vision.pose import ensure_model; print(ensure_model())')"
cp "$MODEL_PATH" "$REPO_ROOT/app/src-tauri/resources/models/pose_landmarker_full.task"
mkdir -p "$REPO_ROOT/app/src-tauri/resources/debug-videos"
cp "$REPO_ROOT/vision/tests/fixtures/videos/squat_demo.webm" "$REPO_ROOT/app/src-tauri/resources/debug-videos/"
cp "$REPO_ROOT/docs/production/debug-video-attribution.txt" "$REPO_ROOT/app/src-tauri/resources/debug-videos/ATTRIBUTION.txt"
cp -r "$HUB_DIR/apps/hubd/public" "$OUT/public"
mkdir -p "$OUT/vision"
cp -r "$HUB_DIR/vision/host" "$OUT/vision/host"
cp -r "$HUB_DIR/vision/plugins" "$OUT/vision/plugins"
# hubd's whisper transcriber spawns `uv run python -m transcribe.whisper_cli`
# from the vision dir, so the module has to ship with the bundle.
cp -r "$HUB_DIR/vision/transcribe" "$OUT/vision/transcribe"
cp "$HUB_DIR/vision/pyproject.toml" "$HUB_DIR/vision/uv.lock" "$OUT/vision/"
find "$OUT" -name __pycache__ -type d -exec rm -rf {} + 2>/dev/null || true
find "$OUT" -name .pytest_cache -type d -exec rm -rf {} + 2>/dev/null || true

# Reps' companion screens (Workout/Calibrate/History) — hubd serves them at
# /app/ via HUB_APP_UI_DIR (the bundled supervisor points at this copy).
rm -rf "$REPO_ROOT/app/src-tauri/resources/companion"
cp -r "$REPO_ROOT/companion" "$REPO_ROOT/app/src-tauri/resources/companion"

# Transcript hashes pin the contract version the vendored Rust tests use.
TRANSCRIPTS_DIR="$HUB_DIR/apps/hubd/test/contracts"

manifest="$REPO_ROOT/app/src-tauri/resources/hub-manifest.json"
{
  echo "{"
  echo "  \"hubCommit\": \"$HUB_COMMIT\","
  echo "  \"hubDirty\": $HUB_DIRTY,"
  echo "  \"apiVersion\": \"$API_VERSION\","
  echo "  \"nodeEngine\": \">=22.5\","
  echo "  \"artifactSha256s\": {"
  first=1
  while IFS= read -r file; do
    rel="${file#"$OUT/"}"
    hash="$(sha256sum "$file" | cut -d' ' -f1)"
    [[ $first -eq 0 ]] && echo ","
    first=0
    printf '    "%s": "%s"' "$rel" "$hash"
  done < <(find "$OUT" -type f | sort)
  echo ""
  echo "  },"
  echo "  \"transcriptSha256s\": {"
  first=1
  while IFS= read -r file; do
    rel="${file#"$TRANSCRIPTS_DIR/"}"
    hash="$(sha256sum "$file" | cut -d' ' -f1)"
    [[ $first -eq 0 ]] && echo ","
    first=0
    printf '    "%s": "%s"' "$rel" "$hash"
  done < <(find "$TRANSCRIPTS_DIR" -type f -name '*.json' | sort)
  echo ""
  echo "  }"
  echo "}"
} > "$manifest"
python3 - "$manifest" <<'PYHASH'
import hashlib, json, pathlib, sys
manifest = pathlib.Path(sys.argv[1])
body = json.loads(manifest.read_text())
root = manifest.parent
files = [p for directory in ("reps-vision", "models", "companion", "debug-videos") for p in (root / directory).rglob("*") if p.is_file()]
body["consumerSha256s"] = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)}
manifest.write_text(json.dumps(body, indent=2) + "\n")
PYHASH

echo "staged $(find "$OUT" -type f | wc -l) files into $OUT"
echo "manifest: $manifest (hub @ ${HUB_COMMIT:0:12}, api v$API_VERSION)"

# Verify the vendored contract transcripts match the pinned hub's (only
# versions the Rust client vendors are checked).
while IFS= read -r vendored; do
  rel="${vendored#"$REPO_ROOT/app/src-tauri/hub-client/tests/contracts/"}"
  if ! cmp -s "$TRANSCRIPTS_DIR/$rel" "$vendored"; then
    echo "warning: vendored transcript $rel differs from hub — re-vendor it" >&2
  fi
done < <(find "$REPO_ROOT/app/src-tauri/hub-client/tests/contracts" -type f -name '*.json' | sort)
