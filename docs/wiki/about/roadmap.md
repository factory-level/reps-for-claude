# Current status and remaining work

The Tauri desktop, daily plan, local vision plugin, Movement Studio workflow and
durable publication queue are implemented. The legacy presets remain available
until an evaluated movement version is activated. Both exercise and jump-rope
preset sensitivity were reduced on 2026-09-15.

Real-gym qualification remains pending: camera calibration, separate held-out
sessions, target accuracy, actual display latency, interruption tests and soak.
The changed sensitivity defaults have synthetic checks but no camera trial.
Public-video and software results do not establish production accuracy.

See [design](../../design/index.md), [as-built coverage](../../architecture/index.md),
and [software readiness](../../production/software-readiness.md).

RFP adds gentle passive reminders while local Codex/Claude processes are open,
a private-history CLI, and an account-free public activity site. See
[dogfood setup](../../production/rfp-dogfood.md). Live public sync requires the
site database and scoped credentials; physical-camera qualification remains a
separate gate.
