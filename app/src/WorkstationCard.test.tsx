import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WorkstationCard } from "./WorkstationCard";
import type { Snapshot } from "./snapshot";

const base: Snapshot = {
  phase: "CODING",
  remainingSeconds: 1122,
  prescription: null,
  progress: null,
  capacityUsed: 3,
  capacityLimit: 20,
  rotation: ["bench", "row"],
  pointer: 1,
};

describe("WorkstationCard", () => {
  it("shows the countdown and next exercise while coding", () => {
    render(<WorkstationCard snapshot={base} onAction={vi.fn()} />);
    expect(screen.getByText("18:42")).toBeInTheDocument();
    expect(screen.getByText(/row/)).toBeInTheDocument();
    expect(screen.getByText(/Coding Session/)).toBeInTheDocument();
  });

  it("shows the prescription when exercise is required", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "EXERCISE_REQUIRED",
          prescription: {
            exercise: "bench",
            kind: "REP",
            targetReps: 10,
            targetSeconds: 0,
            defaultWeight: 95,
          },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/Movement Required/)).toBeInTheDocument();
    expect(screen.getByText(/bench/)).toBeInTheDocument();
    expect(screen.getByText(/0 \/ 10 reps/)).toBeInTheDocument();
  });

  it("shows live progress during the workout", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "WORKOUT_ACTIVE",
          prescription: {
            exercise: "bench",
            kind: "REP",
            targetReps: 10,
            targetSeconds: 0,
            defaultWeight: 95,
          },
          progress: { value: 4, unit: "reps", satisfied: false },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/4 \/ 10 reps/)).toBeInTheDocument();
  });

  it("shows targetSeconds instead of targetReps for continuous prescriptions when required", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "EXERCISE_REQUIRED",
          prescription: {
            exercise: "jumprope",
            kind: "CONTINUOUS",
            targetReps: 0,
            targetSeconds: 60,
            defaultWeight: 0,
          },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/0 \/ 60 seconds/)).toBeInTheDocument();
  });

  it("shows targetSeconds instead of targetReps for continuous prescriptions during the workout", () => {
    render(
      <WorkstationCard
        snapshot={{
          ...base,
          phase: "WORKOUT_ACTIVE",
          prescription: {
            exercise: "jumprope",
            kind: "CONTINUOUS",
            targetReps: 0,
            targetSeconds: 60,
            defaultWeight: 0,
          },
          progress: { value: 12, unit: "seconds", satisfied: false },
        }}
        onAction={vi.fn()}
      />,
    );
    expect(screen.getByText(/12 \/ 60 seconds/)).toBeInTheDocument();
  });

  it("shows unlocked state", () => {
    render(
      <WorkstationCard snapshot={{ ...base, phase: "UNLOCKED" }} onAction={vi.fn()} />,
    );
    expect(screen.getByText(/Unlocked/)).toBeInTheDocument();
  });
});
