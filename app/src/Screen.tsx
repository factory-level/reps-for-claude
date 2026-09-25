// The one screen, two flavours. Both windows render the same three bands —
// title / character / status — from the same snapshot; only the status band
// differs: the primary monitor shows the padlock + debt (and takes the weight
// and the escape keys), the gym display shows the live count.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { Snapshot } from "./snapshot";

export type Variant = "primary" | "gym";


export const mmss = (s: number) =>
  `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export const debtOf = (snapshot: Snapshot): number | null =>
  snapshot.day ? snapshot.day.setsTotal - snapshot.day.setsDone : null;

function WeightEntry({ initial }: { initial: number }) {
 return <><span className="medium">Set complete</span><span className="small">Log from your terminal</span><span className="small">rfp finish --weight {initial}</span></>;
}

function PrimaryStatus({ snapshot, fallback, debug }: { snapshot: Snapshot; fallback: boolean; debug: boolean }) {
  switch (snapshot.phase) {
    case "CODING":
      return <span className="small">{debug ? "Idle · start a test when you’re ready" : `Next workout in ${mmss(snapshot.remainingSeconds)}`}</span>;
    case "EXERCISE_REQUIRED":
      return <span className="small">Time for a little movement · start when ready: rfp start</span>;
    case "WEIGHT_CONFIRMATION":
      return <WeightEntry key={snapshot.prescription?.exercise} initial={snapshot.prescription?.defaultWeight ?? 0} />;
    default: {
      const debt = debtOf(snapshot);
      const rx = snapshot.prescription;
      const label = snapshot.day?.items.find((i) => i.name === rx?.exercise)?.label ?? rx?.exercise ?? "";
      const reps = rx?.kind === "REP";
      const target = reps ? rx?.targetReps ?? 0 : rx?.targetSeconds ?? 0;
      return (
        <>
          <span className="medium">
            {label} · {Math.floor(snapshot.progress?.value ?? 0)} / {target} {reps ? "reps" : "sec"}
          </span>
          <div className="lockrow">

            <span className="big">{debt ?? "!"}</span>
          </div>
          <span className="small">{fallback ? "Camera down · finish via rfp finish --honor" : debug ? "Test progress · not saved to your workouts" : "A little movement between prompts"}</span>
        </>
      );
    }
  }
}

function GymStatus({ snapshot, debug }: { snapshot: Snapshot; debug: boolean }) {
  const rx = snapshot.prescription;
  const label = snapshot.day?.items.find((i) => i.name === rx?.exercise)?.label ?? rx?.exercise ?? "";
  const reps = rx?.kind === "REP";
  const target = reps ? rx?.targetReps ?? 0 : rx?.targetSeconds ?? 0;
  const unit = reps ? "reps" : "sec";
  const pad3 = (n: number) => String(Math.floor(n)).padStart(3, "0");
  switch (snapshot.phase) {
    case "CODING":
      return <span className="small">{debug ? "Idle · start a test when you’re ready" : `Next workout in ${mmss(snapshot.remainingSeconds)}`}</span>;
    case "WEIGHT_CONFIRMATION":
      return (
        <>
          <span className="small">Set done</span>
          <span className="big">{pad3(target)}</span>
          <span className="small">
            / {target} {unit}
          </span>
        </>
      );
    default:
      return (
        <>
          <span className="small">{label}</span>
          <span className="big">{pad3(snapshot.progress?.value ?? 0)}</span>
          <span className="small">
            / {target} {unit}
          </span>
        </>
      );
  }
}

export function Screen({ snapshot, variant, debug = false }: { snapshot: Snapshot; variant: Variant; debug?: boolean }) {
  const coding = snapshot.phase === "CODING";
  const mode = coding ? "code" : "workout";
  const [fallback, setFallback] = useState(false);
  useEffect(() => {
    if (coding) setFallback(false);
  }, [coding]);
  useEffect(() => {
    let active=true;let remove:(()=>void)|undefined;
    void listen<{reason:string}>("vision-fallback",()=>{if(active)setFallback(true);}).then(fn=>{if(active)remove=fn;else fn();});
    return()=>{active=false;remove?.();};
  },[]);

  // Set logged → whole-screen takeover for the beat before CODE.
  if (snapshot.phase === "UNLOCKED") {
    return (
      <div className="takeover" data-phase="UNLOCKED">
        <svg className="check" viewBox="0 0 100 100" role="img" aria-label="logged">
          <circle cx="50" cy="50" r="44" fill="var(--go)" stroke="var(--ink)" strokeWidth="6" />
          <path d="M28 52 L44 68 L74 36" fill="none" stroke="var(--ivory)" strokeWidth="12" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <h1 className="title">{debug ? "TEST COMPLETE" : "LOGGED"}</h1>
      </div>
    );
  }

  return (
    <div className={`stage ${mode}`} data-phase={snapshot.phase}>
      {/* Animated WebP, not <video>: WebKitGTK's looping media pipeline leaks
          ~6MB/s. Falls back to the still if the animation fails to load. */}
      <img
        key={mode}
        className="scene"
        src={`/art/scene-${mode}.webp`}
        alt=""
        aria-hidden
        onError={(e) => {
          e.currentTarget.src = `/art/scene-${mode}.png`;
        }}
      />
      <h1 className="title">{debug ? (coding ? "DEBUG" : "TEST WORKOUT") : (coding ? "CODE" : "WORKOUT")}</h1>
      <div className="character" />
      <div className="status">
        {variant === "gym" ? <GymStatus snapshot={snapshot} debug={debug} /> : <PrimaryStatus snapshot={snapshot} fallback={fallback} debug={debug} />}
      </div>
    </div>
  );
}
