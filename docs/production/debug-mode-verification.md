# Debug mode verification — 2026-09-15

Release: `app/src-tauri/target/release/bundle/deb/Reps for Claude_0.1.0_amd64.deb`.
Built offline with the pinned MediaPipe Full model and bundled squat video.

- Frontend: 18 tests passed, including mode-change pending/error handling and idle/manual screen rendering.
- Rust workspace: 69 tests passed; one preexisting optional supervisor end-to-end test remains ignored.
- Isolation test completes and persists a simulated set into the temporary database while verifying real history and plan state remain unchanged.
- Extracted installer launched on Linux/X11 with `REPS_WORK_MINUTES=0.001`; remained idle in Debug after expiry.
- Native window inspected through X11: 1280×720, viewable, no fullscreen/above/sticky state flags at idle or during a manual workout.
- Packaged squat video played through the provisioned Python environment, rendering pose landmarks and a two-rep legacy preview without workout credit.
- Manual camera start and simulated +1 worked; Stop test returned to idle.
- Main-window close exited the app, hub, and owned Python processes and removed temporary session data.
- Reopened release with the normal preferences location, Debug default, and camera idle.

Mode preference round trips and backend policy are covered by Rust tests. A native switch into enforced Workout mode was not exercised on the user's active desktop. Camera start/stop was checked; live exercise counting accuracy was not measured in this check.
