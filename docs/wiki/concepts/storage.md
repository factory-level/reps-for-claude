# Saved progress and backups

The default data directory is `~/.local/share/reps-for-claude`.
`REPS_APP_HOME` can select another directory.

| File | Contents |
|---|---|
| `reps.sqlite` | Exercise history, settings, rotation and daily-plan progress |
| `publications.sqlite` | Events, commands and results waiting for hub acknowledgement |
| `routine.json` | Optional local routine override |

Back up the app data directory with the app stopped. Hub recordings, reviewed
labels, movement versions and delivery history belong to the hub's own storage;
include that configured storage in backups too. Debug uses temporary state.

If the hub is unavailable, accepted publications remain queued and retry after
reconnection. A persistent error at the front holds later messages in order.
See [publication recovery](../../production/movement-studio.md#publication-recovery).
Local workout updates and publication enqueue are separate operations; the queue
does not make them one atomic transaction.
