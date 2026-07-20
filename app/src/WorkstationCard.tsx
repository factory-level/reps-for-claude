import type { Snapshot } from "./snapshot";

export type Action =
  | "start_session"
  | "begin_workout"
  | "simulate_rep"
  | "simulate_done"
  | "confirm_weight"
  | "resume_coding";

function mmss(total: number): string {
  const m = Math.floor(total / 60);
  const s = Math.floor(total % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function WorkstationCard({
  snapshot,
  onAction,
}: {
  snapshot: Snapshot;
  onAction: (a: Action, value?: number) => void;
}) {
  const { phase, prescription, progress } = snapshot;
  const next = snapshot.rotation[snapshot.pointer] ?? "—";
  const target =
    prescription?.kind === "CONTINUOUS"
      ? prescription?.targetSeconds ?? 0
      : prescription?.targetReps ?? 0;
  const done = progress?.value ?? 0;

  if (phase === "CODING") {
    return (
      <section>
        <h1>Coding Session</h1>
        <p className="timer">{mmss(snapshot.remainingSeconds)}</p>
        <p>Next: {next}</p>
        <p>
          Capacity: {snapshot.capacityUsed} / {snapshot.capacityLimit} sets
        </p>
        <button onClick={() => onAction("start_session")}>Restart timer</button>
      </section>
    );
  }
  if (phase === "EXERCISE_REQUIRED") {
    return (
      <section>
        <h1>Movement Required</h1>
        <p className="exercise">{prescription?.exercise}</p>
        <p>
          0 / {target} {prescription?.kind === "REP" ? "reps" : "seconds"}
        </p>
        <button onClick={() => onAction("begin_workout")}>Start workout</button>
      </section>
    );
  }
  if (phase === "WORKOUT_ACTIVE") {
    return (
      <section>
        <h1>{prescription?.exercise}</h1>
        <p>
          {done} / {target} {progress?.unit ?? "reps"}
        </p>
        <button onClick={() => onAction("simulate_rep", done + 1)}>
          Simulate rep (dev)
        </button>
        <button onClick={() => onAction("simulate_done")}>Simulate done (dev)</button>
      </section>
    );
  }
  if (phase === "WEIGHT_CONFIRMATION") {
    return (
      <section>
        <h1>What weight did you use?</h1>
        <p>{prescription?.exercise}</p>
        <button
          onClick={() => onAction("confirm_weight", prescription?.defaultWeight ?? 0)}
        >
          Confirm {prescription?.defaultWeight ?? 0} lbs
        </button>
      </section>
    );
  }
  return (
    <section>
      <h1>Unlocked</h1>
      <p>Coding session available</p>
      <button onClick={() => onAction("resume_coding")}>Back to coding</button>
    </section>
  );
}
