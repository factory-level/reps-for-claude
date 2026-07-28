// Full-screen LOCK bulkhead (visual only — no OS lock this iteration). Shown
// when the routine gates a set before you can continue: an amber caution frame,
// the draining hourglass instrument, the prescribed movement, and an ENGAGE
// button. Mounts fixed over everything.
import type { Snapshot } from "./snapshot";

function Hourglass() {
  return (
    <svg className="hourglass" viewBox="0 0 100 140" width="118" height="165" aria-hidden>
      {/* frame */}
      <rect x="18" y="6" width="64" height="7" rx="3" className="hg-cap" />
      <rect x="18" y="127" width="64" height="7" rx="3" className="hg-cap" />
      {/* glass outline */}
      <path d="M24 13 H76 L54 70 L76 127 H24 L46 70 Z" className="hg-glass" />
      {/* top sand (drains) */}
      <clipPath id="hg-top">
        <path d="M28 17 H72 L52 66 H48 Z" />
      </clipPath>
      <g clipPath="url(#hg-top)">
        <rect x="24" y="17" width="52" height="50" className="hg-sand hg-sand-top" />
      </g>
      {/* bottom sand (fills) */}
      <clipPath id="hg-bot">
        <path d="M48 74 H52 L72 123 H28 Z" />
      </clipPath>
      <g clipPath="url(#hg-bot)">
        <rect x="24" y="123" width="52" height="50" className="hg-sand hg-sand-bot" />
      </g>
      {/* falling stream */}
      <rect x="49" y="66" width="2" height="10" className="hg-stream" />
    </svg>
  );
}

export function HourglassLock({ snapshot, onStart }: { snapshot: Snapshot; onStart: () => void }) {
  const rx = snapshot.prescription;
  const day = snapshot.day ?? null;
  const label = day?.items.find((i) => i.name === rx?.exercise)?.label ?? rx?.exercise ?? "a set";
  const target = rx ? (rx.kind === "CONTINUOUS" ? `${rx.targetSeconds} SEC HOLD` : `${rx.targetReps} REPS`) : "";
  const left = day ? day.setsTotal - day.setsDone : null;

  return (
    <div className="lock-overlay" role="dialog" aria-modal>
      <div className="lock-card seal framed">
        <Hourglass />
        <span className="lock-kicker">Access Locked</span>
        <h1 className="lock-exercise">{label}</h1>
        {target && (
          <p className="lock-target">
            TARGET · <b>{target}</b>
          </p>
        )}
        <button className="hud-btn amber" onClick={onStart}>
          Engage ▸
        </button>
        {left !== null && (
          <p className="lock-remaining">
            {left > 0 ? (
              <>
                <b>{left}</b> SET{left === 1 ? "" : "S"} REMAINING TODAY
              </>
            ) : (
              "FINAL SET OF THE DAY"
            )}
          </p>
        )}
        {rx == null && <p className="muted">preparing your next set…</p>}
      </div>
    </div>
  );
}
