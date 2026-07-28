export type Phase =
  | "CODING"
  | "EXERCISE_REQUIRED"
  | "WORKOUT_ACTIVE"
  | "WEIGHT_CONFIRMATION"
  | "UNLOCKED";

export interface Prescription {
  exercise: string;
  kind: "REP" | "CONTINUOUS";
  targetReps: number;
  targetSeconds: number;
  defaultWeight: number;
}

export interface Progress {
  value: number;
  unit: string;
  satisfied: boolean;
}

// One line of the day's routine (a lift, jump-rope, or a stretch) with how many
// of its sets/rounds are done. "One set per lock" decrements `done` each time.
export interface DayItem {
  name: string;
  label: string;
  kind: "lift" | "jumprope" | "stretch";
  unit: "reps" | "seconds";
  target: number; // reps per set, or seconds per round/hold
  done: number; // sets/rounds/holds completed
  total: number; // sets/rounds/holds required today
}

export interface DayPlan {
  items: DayItem[];
  setsDone: number; // completed across all items
  setsTotal: number; // required across all items
  complete: boolean; // every item done → day cleared
}

export interface Snapshot {
  phase: Phase;
  remainingSeconds: number;
  prescription: Prescription | null;
  progress: Progress | null;
  capacityUsed: number;
  capacityLimit: number;
  rotation: string[];
  pointer: number;
  // Present once the routine.json plan is wired; optional so the UI degrades
  // gracefully before then.
  day?: DayPlan | null;
}
