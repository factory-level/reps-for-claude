# Glossary

**What this page tells you:** plain-English definitions of every term used in these docs.

| Term | Meaning |
|---|---|
| **Activity** | A small counter for one kind of exercise. Fed one camera frame at a time, it reports how much is done and whether the target is met. Three kinds: lift, jump rope, stretch. |
| **Atomic write** | Saving a file all-or-nothing: write to a temp file, then rename it over the real one. A crash mid-save can never leave a half-written file. |
| **Dues** | The workout you currently owe. When dues are owed, the machine locks; paying them (doing the set) unlocks it. |
| **Fail open** | When a safety check breaks, choose the outcome that frees the user. A corrupt dues file reads as "nothing owed" — a bug must never lock you out of your own computer. |
| **ISO week** | The standard calendar week label, Monday through Sunday, written like `2026-W29`. The weekly log resets when the label changes. |
| **Landmarks** | The joint positions (shoulder, hip, knee...) that pose detection finds in a camera frame. |
| **Ledger** | The legacy daily state file from the old bank-then-spend model. |
| **Lock / locker** | The real X11 screen lock (`xsecurelock`) that grabs all monitors and input until dues are paid. |
| **MediaPipe Pose** | Google's free computer-vision model that finds a person's joints in an image. The only external smarts in the project. |
| **Override** | The escape-hatch password that unlocks without exercise. Always logged, so skipped workouts show up in the weekly report. |
| **Prescription** | What the lock screen tells you to do: one set of a specific exercise, e.g. "10 squats." Auto-picked from whichever weekly goal is most behind. |
| **Progress** | The live answer an activity gives every frame: a value, a unit (`reps` or `seconds`), and whether the target is satisfied. |
| **Rep state machine** | The up/down tracker that counts a rep only when a joint angle clearly goes below the "down" threshold and back above the "up" threshold. |
| **REPS_HOME** | An environment variable that relocates all config and state to one directory. The test suite sets it so tests never touch your real files. |
| **Scoreboard** | The second-monitor view while coding: the countdown, weekly goal bars, and today's totals. Plan C. |
| **Session supervisor** | The `reps session` process that runs the whole loop: timer, lock, camera, unlock. Plan B. |
| **Volume** | Reps times weight, in pounds. Ten squats at 45 lbs = 450 lbs of volume. The number your trainer actually reads. |
| **Weekly goals** | Per-exercise rep targets for the week (e.g. `squat = 60`) that drive every prescription. |
| **xsecurelock** | A standard, battle-tested Linux screen locker that this app drives. Installed with `apt install xsecurelock`. |
