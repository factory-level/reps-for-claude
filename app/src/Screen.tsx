// The one screen, two flavours. Both windows render the same three bands —
// title / character / status — from the same snapshot; only the status band
// differs: the primary monitor shows the padlock + debt (and takes the weight
// and the escape keys), the gym display shows the live count.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import type { Snapshot } from "./snapshot";

export type Variant = "primary" | "gym";

const ESCAPE_HOLD_MS = 3000;

export const mmss = (s: number) =>
  `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export const debtOf = (snapshot: Snapshot): number | null =>
  snapshot.day ? snapshot.day.setsTotal - snapshot.day.setsDone : null;

function Padlock() {
  // 16×16 pixel padlock; the shackle hole is ink, no transparency needed.
  const ink = "var(--ink)";
  const body = "var(--clawd)";
  return (
    <svg className="padlock" viewBox="0 0 16 16" aria-label="locked" role="img">
      <rect x="3" y="0" width="10" height="7" fill={ink} />
      <rect x="4" y="1" width="8" height="5" fill={body} />
      <rect x="6" y="3" width="4" height="3" fill={ink} />
      <rect x="1" y="6" width="14" height="10" fill={ink} />
      <rect x="2" y="7" width="12" height="8" fill={body} />
      <rect x="7" y="9" width="2" height="3" fill={ink} />
    </svg>
  );
}

function WeightEntry({ initial }: { initial: number }) {
  const [weight, setWeight] = useState(initial);
  return (
    <>
      <span className="small">Set done · log weight (lbs)</span>
      <input
        className="weight"
        type="number"
        step={5}
        min={0}
        autoFocus
        value={weight}
        onChange={(e) => setWeight(Number(e.target.value))}
        onKeyDown={(e) => {
          if (e.key === "Enter") void invoke("confirm_weight", { weight });
        }}
        aria-label="weight in pounds"
      />
      <span className="small">Enter to log</span>
    </>
  );
}

function PrimaryStatus({ snapshot, fallback }: { snapshot: Snapshot; fallback: boolean }) {
  switch (snapshot.phase) {
    case "CODING":
      return <span className="small">Next workout in {mmss(snapshot.remainingSeconds)}</span>;
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
            <Padlock />
            <span className="big">{debt ?? "!"}</span>
          </div>
          <span className="small">{fallback ? "Camera down · press H for honor mode" : "Workout debt remaining"}</span>
        </>
      );
    }
  }
}

function GymStatus({ snapshot }: { snapshot: Snapshot }) {
  const rx = snapshot.prescription;
  const label = snapshot.day?.items.find((i) => i.name === rx?.exercise)?.label ?? rx?.exercise ?? "";
  const reps = rx?.kind === "REP";
  const target = reps ? rx?.targetReps ?? 0 : rx?.targetSeconds ?? 0;
  const unit = reps ? "reps" : "sec";
  const pad3 = (n: number) => String(Math.floor(n)).padStart(3, "0");
  switch (snapshot.phase) {
    case "CODING":
      return <span className="small">Next workout in {mmss(snapshot.remainingSeconds)}</span>;
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

export function Screen({ snapshot, variant }: { snapshot: Snapshot; variant: Variant }) {
  const coding = snapshot.phase === "CODING";
  const mode = coding ? "code" : "workout";
  const [fallback, setFallback] = useState(false);
  const fallbackRef = useRef(false);
  fallbackRef.current = fallback;
  const hold = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (coding) setFallback(false);
  }, [coding]);

  // Gym window: F11 toggles fullscreen (remembered across launches).
  useEffect(() => {
    if (variant !== "gym") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "F11") void invoke("toggle_gym_fullscreen");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [variant]);

  useEffect(() => {
    if (variant !== "primary") return;
    const unlisten = listen<{ reason: string }>("vision-fallback", () => setFallback(true));
    // Escape hatch (spec §14): hold Ctrl+Shift+Backspace for 3s. Honor mode: H.
    const down = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "Backspace" && !hold.current) {
        hold.current = setTimeout(() => void invoke("debug_mode", { mode: "coding" }), ESCAPE_HOLD_MS);
      } else if (e.key.toLowerCase() === "h" && fallbackRef.current) {
        void invoke("honor_complete");
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === "Backspace" || e.key === "Control" || e.key === "Shift") {
        if (hold.current) clearTimeout(hold.current);
        hold.current = null;
      }
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      void unlisten.then((u) => u());
    };
  }, [variant]);

  // Set logged → whole-screen takeover for the beat before CODE.
  if (snapshot.phase === "UNLOCKED") {
    return (
      <div className="takeover" data-phase="UNLOCKED">
        <svg className="check" viewBox="0 0 100 100" role="img" aria-label="logged">
          <circle cx="50" cy="50" r="44" fill="var(--go)" stroke="var(--ink)" strokeWidth="6" />
          <path d="M28 52 L44 68 L74 36" fill="none" stroke="var(--ivory)" strokeWidth="12" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <h1 className="title">LOGGED</h1>
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
      <h1 className="title">{coding ? "CODE" : "WORKOUT"}</h1>
      <div className="character" />
      <div className="status">
        {variant === "gym" ? <GymStatus snapshot={snapshot} /> : <PrimaryStatus snapshot={snapshot} fallback={fallback} />}
      </div>
    </div>
  );
}
