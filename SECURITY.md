# Security policy

## Reporting a vulnerability

Please report security issues privately through
[GitHub's private vulnerability reporting](https://github.com/factory-level/reps-for-prompts/security/advisories/new)
rather than opening a public issue. Expect an initial response within a week.

## Threat model

RFP is local-first, and that shapes what counts as a vulnerability here.

**Video never leaves the machine.** Pose estimation runs against a locally
bundled vision hub. Consumers of that hub subscribe to semantic events
(`rep_completed`) rather than frames. Anything that would cause frames to be
transmitted off-device is a security bug, not a feature request.

**Agent detection reads process presence only** — the names of running
executables, scoped to the current Unix user. It does not read prompts,
transcripts, file contents or terminal output. Anything that widens that is a
security bug.

**Sync and public posting are off by default.** They must be explicitly
configured per dataset with a scoped token, credentials live in `upload.json`
and `remote.json` under the data directory at mode `0600`, and they are never
passed as command arguments. Public posting is rate limited to six posts per
hour.

**The hub listens on the LAN over HTTPS** with a locally generated certificate
so a phone can be used as a second camera. Reports about that surface are in
scope.

Workout history in `~/.local/share/rfp` is unencrypted local SQLite, which is
intentional — it is protected by your user account, like the rest of your home
directory.
