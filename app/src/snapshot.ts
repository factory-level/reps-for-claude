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

export interface Snapshot {
  phase: Phase;
  remainingSeconds: number;
  prescription: Prescription | null;
  progress: Progress | null;
  capacityUsed: number;
  capacityLimit: number;
  rotation: string[];
  pointer: number;
}
