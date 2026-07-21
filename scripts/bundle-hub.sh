#!/usr/bin/env bash
# Stage the pinned usb-mcp-hub build into app/src-tauri/resources/hub-bundle/
# and write hub-manifest.json (commit + apiVersion + artifact hashes).
#
# The bundle layout the supervisor expects:
#   hub-bundle/
#     hubd.mjs          single-file hubd (node >= 20)
#     public/           snapshot tuning app assets
#     vision/           vision-host sources + pyproject + uv.lock (frame_stats included)
# The reps plugin is NOT staged here — it ships with this repo at
# vision/src and is loaded via HUB_PLUGIN_ARGS. Python env is provisioned
# at first run by uv (network required once).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HUB_DIR="${HUB_DIR:-$REPO_ROOT/../usb-mcp-hub}"
OUT="$REPO_ROOT/app/src-tauri/resources/hub-bundle"
API_VERSION="1.0"

if [[ ! -d "$HUB_DIR" ]]; then
  echo "usb-mcp-hub checkout not found at $HUB_DIR (set HUB_DIR)" >&2
  exit 1
fi

HUB_COMMIT="$(git -C "$HUB_DIR" rev-parse HEAD)"
if [[ -n "$(git -C "$HUB_DIR" status --porcelain)" ]]; then
  echo "warning: $HUB_DIR has uncommitted changes; manifest pins $HUB_COMMIT anyway" >&2
fi

echo "building hubd bundle from $HUB_DIR @ ${HUB_COMMIT:0:12}"
(cd "$HUB_DIR/apps/hubd" && node scripts/build-bundle.mjs >/dev/null)

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$HUB_DIR/apps/hubd/dist/hubd.mjs" "$OUT/hubd.mjs"
cp -r "$HUB_DIR/apps/hubd/public" "$OUT/public"
mkdir -p "$OUT/vision"
cp -r "$HUB_DIR/vision/host" "$OUT/vision/host"
cp -r "$HUB_DIR/vision/plugins" "$OUT/vision/plugins"
cp "$HUB_DIR/vision/pyproject.toml" "$HUB_DIR/vision/uv.lock" "$OUT/vision/"
find "$OUT" -name __pycache__ -type d -exec rm -rf {} + 2>/dev/null || true
find "$OUT" -name .pytest_cache -type d -exec rm -rf {} + 2>/dev/null || true

# Transcript hashes pin the contract version the vendored Rust tests use.
TRANSCRIPTS_DIR="$HUB_DIR/apps/hubd/test/contracts/v1"

manifest="$REPO_ROOT/app/src-tauri/resources/hub-manifest.json"
{
  echo "{"
  echo "  \"hubCommit\": \"$HUB_COMMIT\","
  echo "  \"apiVersion\": \"$API_VERSION\","
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
    rel="$(basename "$file")"
    hash="$(sha256sum "$file" | cut -d' ' -f1)"
    [[ $first -eq 0 ]] && echo ","
    first=0
    printf '    "%s": "%s"' "$rel" "$hash"
  done < <(find "$TRANSCRIPTS_DIR" -type f -name '*.json' | sort)
  echo ""
  echo "  }"
  echo "}"
} > "$manifest"

echo "staged $(find "$OUT" -type f | wc -l) files into $OUT"
echo "manifest: $manifest (hub @ ${HUB_COMMIT:0:12}, api v$API_VERSION)"

# Verify the vendored contract transcripts match the pinned hub's.
for file in "$TRANSCRIPTS_DIR"/*.json; do
  vendored="$REPO_ROOT/app/src-tauri/hub-client/tests/contracts/v1/$(basename "$file")"
  if [[ ! -f "$vendored" ]] || ! cmp -s "$file" "$vendored"; then
    echo "warning: vendored transcript $(basename "$file") differs from hub — re-vendor it" >&2
  fi
done
