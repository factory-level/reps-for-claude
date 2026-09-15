import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DebugModeToggle } from "./DebugModeToggle";
import type { Snapshot } from "./snapshot";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const snapshot: Snapshot = { phase: "CODING", remainingSeconds: 0, prescription: null, progress: null, capacityUsed: 0, capacityLimit: 20, rotation: [], pointer: 0 };
it("starts the chosen exercise and switches an active detector", async () => {
  vi.mocked(invoke).mockImplementation(async command => command === "debug_exercises" ? [
    { exercise: "bench", kind: "REP", targetReps: 10, targetSeconds: 0, defaultWeight: 0 },
    { exercise: "jumprope", kind: "CONTINUOUS", targetReps: 0, targetSeconds: 60, defaultWeight: 0 },
  ] : undefined);
  const { rerender } = render(<DebugModeToggle snapshot={snapshot} onDetectionDebug={() => {}} />);
  await screen.findByRole("option", { name: "jumprope · 60 sec" });
  fireEvent.change(screen.getByLabelText("Exercise"), { target: { value: "jumprope" } });
  expect(invoke).not.toHaveBeenCalledWith("debug_mode", expect.anything());
  fireEvent.click(screen.getByText("F2 Start camera"));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("debug_mode", { mode: "workout", exercise: "jumprope" }));
  rerender(<DebugModeToggle snapshot={{ ...snapshot, phase: "WORKOUT_ACTIVE" }} onDetectionDebug={() => {}} />);
  fireEvent.change(screen.getByLabelText("Exercise"), { target: { value: "bench" } });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("debug_mode", { mode: "workout", exercise: "bench" }));
});
