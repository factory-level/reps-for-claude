import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Screen } from "./Screen";
import type { Snapshot } from "./snapshot";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

const base: Snapshot = {
  phase: "CODING",
  remainingSeconds: 359,
  prescription: null,
  progress: null,
  capacityUsed: 0,
  capacityLimit: 20,
  rotation: [],
  pointer: 0,
  day: {
    items: [{ name: "jumprope", label: "Jump rope", kind: "jumprope", unit: "seconds", target: 60, done: 0, total: 2 }],
    setsDone: 7,
    setsTotal: 31,
    complete: false,
  },
};

describe("Screen", () => {
  it("CODING shows CODE and the countdown", () => {
    render(<Screen snapshot={base} variant="primary" />);
    expect(screen.getByText("CODE")).toBeInTheDocument();
    expect(screen.getByText(/05:59/)).toBeInTheDocument();
  });

  it("primary shows the padlock and remaining sets while locked", () => {
    render(<Screen snapshot={{ ...base, phase: "EXERCISE_REQUIRED" }} variant="primary" />);
    expect(screen.getByText("WORKOUT")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "locked" })).toBeInTheDocument();
    expect(screen.getByText("24")).toBeInTheDocument();
  });

  it("gym shows the live count in the prescription's unit", () => {
    const snap: Snapshot = {
      ...base,
      phase: "WORKOUT_ACTIVE",
      prescription: { exercise: "jumprope", kind: "CONTINUOUS", targetReps: 0, targetSeconds: 60, defaultWeight: 0 },
      progress: { value: 42.7, unit: "seconds", satisfied: false },
    };
    render(<Screen snapshot={snap} variant="gym" />);
    expect(screen.getByText("Jump rope")).toBeInTheDocument();
    expect(screen.getByText("042")).toBeInTheDocument();
    expect(screen.getByText("/ 60 sec")).toBeInTheDocument();
  });
});
