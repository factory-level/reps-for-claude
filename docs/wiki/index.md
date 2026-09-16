# Reps human guide

Reps is a desktop fitness timer. In Workout mode, the timer asks you to complete
one set from your daily routine. Camera detection tracks reps or jump-rope time;
stretches use a timer. Finish the set, confirm weight when prompted, and return
to work. The app starts in Debug mode so you can try detection first.

- [Start and understand the app](concepts/big-picture.md).
- [Workout loop and emergency escape](concepts/the-loop.md).
- [Daily routine](concepts/weekly-goals.md).
- [Rep counting and sensitivity](concepts/detection.md).
- [Saved progress and backups](concepts/storage.md).
- [Configuration](reference/config.md).
- [Adding and tuning movements](reference/adding-exercises.md).
- [Development commands](reference/cli.md).
- [Current limits](about/roadmap.md) and [glossary](about/glossary.md).

For implementation details use [architecture](../architecture/index.md); for
requirements use [design](../design/index.md). The former Python `reps` CLI and
`config.toml` workflow are not the current desktop setup.
