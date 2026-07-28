// The cockpit "big screen": a full-viewport targeting HUD for an active set —
// the live pose stream + set reticle, the current exercise + day-progress
// instrument, and the whole day's routine as a flight manifest grouped by
// discipline ("one set per lock" ticks each line off).
import { StreamVisualizer } from "./StreamVisualizer";
import type { Action } from "./WorkstationCard";
import type { DayItem, Snapshot } from "./snapshot";

function itemIcon(kind: DayItem["kind"]): string {
  return kind === "jumprope" ? "🪢" : kind === "stretch" ? "🧘" : "🏋";
}

const GROUPS: Array<{ key: DayItem["kind"]; title: string }> = [
  { key: "lift", title: "LIFTS" },
  { key: "jumprope", title: "CONDITIONING" },
  { key: "stretch", title: "MOBILITY" },
];

export function WorkoutBigScreen({
  snapshot,
  onAction,
}: {
  snapshot: Snapshot;
  onAction: (a: Action, value?: number) => void;
}) {
  const rx = snapshot.prescription;
  const day = snapshot.day ?? null;
  const dayPct = day && day.setsTotal > 0 ? Math.round((100 * day.setsDone) / day.setsTotal) : 0;
  const exerciseLabel =
    day?.items.find((i) => i.name === rx?.exercise)?.label ?? rx?.exercise ?? "—";
  const target = rx ? (rx.kind === "CONTINUOUS" ? `${rx.targetSeconds}` : `${rx.targetReps}`) : "";
  const targetUnit = rx?.kind === "CONTINUOUS" ? "SEC HOLD" : "REPS";

  return (
    <div className="bigscreen">
      <header className="bigscreen-head">
        <div className="bigscreen-title">
          <span className="kicker">Current Set · Locked On</span>
          <h1>{exerciseLabel}</h1>
          {rx && (
            <span className="target">
              TARGET · <b>{target}</b> {targetUnit}
            </span>
          )}
        </div>
        {day && (
          <div className={`bigscreen-daymeter daymeter ${day.complete ? "complete" : ""}`}>
            <span className="kicker">Day Progress</span>
            <div className="daymeter-row">
              <span className="pct">{dayPct}%</span>
              <span className="frac">
                <b>{day.setsDone}</b> / {day.setsTotal} SETS{day.complete ? " · CLEAR" : ""}
              </span>
            </div>
            <div className="daybar">
              <div className="daybar-fill" style={{ width: `${dayPct}%` }} />
            </div>
          </div>
        )}
      </header>

      <main className="bigscreen-body">
        <section className="bigscreen-stage">
          <StreamVisualizer snapshot={snapshot} />
        </section>

        <aside className="bigscreen-plan">
          <span className="kicker">Mission Manifest</span>
          {day ? (
            GROUPS.map(({ key, title }) => {
              const items = day.items.filter((i) => i.kind === key);
              if (items.length === 0) return null;
              const doneCount = items.filter((i) => i.done >= i.total).length;
              return (
                <div className="plan-group" key={key}>
                  <div className="plan-group-head">
                    <span>{title}</span>
                    <span className="tally">
                      {doneCount}/{items.length}
                    </span>
                  </div>
                  <ul>
                    {items.map((it, idx) => {
                      const active = it.name === rx?.exercise && it.done < it.total;
                      const done = it.done >= it.total;
                      return (
                        <li key={`${key}-${idx}`} className={`${active ? "active" : ""} ${done ? "done" : ""}`}>
                          <span className="plan-icon">{done ? "✓" : itemIcon(it.kind)}</span>
                          <span className="plan-label">{it.label}</span>
                          <span className="plan-count">
                            {it.done}/{it.total}
                            <em>
                              {" "}
                              · {it.target}
                              {it.unit === "seconds" ? "s" : ""}
                            </em>
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              );
            })
          ) : (
            <p className="muted">
              Routine not loaded — running rotation.
              <br />
              Next: {snapshot.rotation[snapshot.pointer] ?? "—"}
            </p>
          )}
        </aside>
      </main>

      <footer className="bigscreen-foot">
        {snapshot.phase === "WEIGHT_CONFIRMATION" ? (
          <>
            <span className="foot-status">
              <span className="lamp" /> SET COMPLETE · LOG THE LOAD
            </span>
            <button className="hud-btn" onClick={() => onAction("confirm_weight", rx?.defaultWeight ?? 0)}>
              Confirm {rx?.defaultWeight ?? 0} lbs ▸
            </button>
          </>
        ) : (
          <>
            <span className="foot-status">
              <span className="lamp" /> REPS COUNTED FROM CAMERA · AUTO
            </span>
            <span className="foot-dev">
              <button onClick={() => onAction("simulate_rep", 1)}>+1 rep</button>
              <button onClick={() => onAction("simulate_done")}>complete set</button>
            </span>
          </>
        )}
      </footer>
    </div>
  );
}
