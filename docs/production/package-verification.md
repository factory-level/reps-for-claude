# Development installer verification

Verified on Linux Mint 22.3, amd64, 2026-09-15 UTC. This is an unpublished
development artifact from modified worktrees, not a qualified gym release.

## Artifact

Build command from `app/`:

```sh
pnpm tauri build --bundles deb --ci -- --offline --locked
```

Output: `app/src-tauri/target/release/bundle/deb/Reps for Claude_0.1.0_amd64.deb`

- Size: 17,046,734 bytes.
- SHA-256: `dc6be3007f9cd791373d30b8c9eb49cfb8d5c818f9ec82d60526eb0565e0bfb8`.
- Source base commits: Reps `1358672666b7fab7c0e76c90c3fc19e75aa0271f`;
  hub `62c50873df0da701fa51e0b3fdbd1c826c35b780`. Both include uncommitted changes.
- Package: `reps-for-claude`, version `0.1.0`, with native executable `app`.
- Packaged resources: about 9.7 MiB. No `.venv` or `__pycache__` entries.

Release builds restage the hub, plugin and model before packaging. Runtime
environments live in writable application data, never inside installed resources.
JSON plugin arguments preserve paths such as `/usr/lib/Reps for Claude`.

## Checks performed

The package was extracted into `/tmp/fieldlab-timing-installed` with `dpkg-deb -x`;
it was not installed into the operating system.

1. Ran the extracted native executable with `--check-runtime`. It used Tauri's
   resource lookup and the real Rust supervisor, reported vision host `up`, camera
   `closed`, and zero enabled metrics, then shut down cleanly.
2. That check ran with `UV_OFFLINE=1`, an empty `UV_CACHE_DIR`, and an already
   provisioned `UV_PROJECT_ENVIRONMENT`. It passed.
3. Ran the packaged plugin and hub against the tracked video, with offline
   dependency resolution and another empty cache:

   ```sh
   UV_CACHE_DIR=/tmp/fieldlab-timing-uv-cache node scripts/e2e-latency.mjs \
     --resources '/tmp/fieldlab-timing-installed/usr/lib/Reps for Claude' \
     --python-env /tmp/fieldlab-installed-python \
     --offline --movement-contract --parent-exit
   ```

   Result: 199 frames, two reps, 18.2 ms p95 frame latency. Progress followed
   committed repetitions and retained version/model/session provenance. Closing
   the supervisor pipe shut down the hub cleanly while detection was enabled.
   The movement activation state was synthetic and isolated to the test database;
   these results do not qualify accuracy or promotion.
4. The supervisor regression tests preserve unrelated listeners on occupied ports
   and release owned descendants when startup fails.

The offline checks disabled dependency downloads; they did not disconnect the
machine's network interfaces. Live detection used the packaged model file.

## Still required

Install and exercise the full desktop UI on the gym machine. Validate emergency
escape, actual camera failures, held-out movement accuracy, rep-to-display
latency, disk-full behavior, and the eight-hour soak. Node >=22.5 and uv remain
runtime prerequisites; the Python environment needs provisioning once.

The installer was not published, signed, or exercised on other operating systems.

This rebuild includes the source-timestamp fix in the pose wrapper. Detector
tests: 129 passed, four video tests deselected. The packaged video test above
ran real MediaPipe separately. Local socket tests ran outside the sandbox after
its read-only default cache and socket restrictions prevented startup.

## Full-model rebuild — 2026-09-15

A newer development installer supersedes the Lite artifact described above:

- Same Debian output path; SHA-256
  `ea193d615c677f57b4404b45e7e3a2efc316979b189bdc5d4cb9aab69ee1ba61`.
- Bundles the SHA-256-pinned MediaPipe Full float16 v1 model and updated hub
  storage error handling. Lite model resources are removed when restaging.
- Built using the same offline/locked Tauri command, then extracted under
  `/tmp/reps-full-installed` without system installation.
- Extracted native `--check-runtime` passed with offline uv resolution, empty
  dependency cache and the provisioned environment: host up, camera closed,
  no enabled metrics. `go2rtc` was unavailable; this verified the direct/file
  runtime path, not RTSP ingest.
- Extracted packaged versioned-video check passed: **198 frames, two reps,
  47.6 ms p95 frame transport latency**, retained provenance and committed-rep
  ordering, and clean parent-pipe shutdown. Other tests were running during this
  measurement. It is not the physical rep-to-display measurement.
- Source worktrees remain dirty and the installer is unpublished. Lite-pinned
  movement candidates require model-compatible recreation/evaluation; see
  [software readiness](software-readiness.md).

Logs for this run: `/tmp/package-final.log`, `/tmp/native-full.log`, and
`/tmp/packaged-full-video.log`. All physical release gates above remain open.
