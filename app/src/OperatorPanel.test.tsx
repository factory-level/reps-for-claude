import { act, render, screen, fireEvent } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OperatorPanel } from "./OperatorPanel";

const invokeMock = vi.fn();
const listeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, cb: (event: { payload: unknown }) => void) => {
    listeners.set(name, cb);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

function emit(name: string, payload: unknown) {
  act(() => {
    listeners.get(name)?.({ payload });
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
  listeners.clear();
});

describe("OperatorPanel", () => {
  it("shows pose status and angle from landmark frames", async () => {
    render(<OperatorPanel />);
    await screen.findByTestId("operator-status");
    emit("vision-landmarks", {
      poseDetected: true,
      landmarks: { left_knee: [0.5, 0.5, 0.9] },
      angle: 97.4,
      measuredJoints: ["left_knee"],
      tsUs: 1,
    });
    expect(screen.getByTestId("operator-status").textContent).toContain("angle 97°");
    emit("vision-landmarks", {
      poseDetected: false,
      landmarks: {},
      angle: null,
      measuredJoints: [],
      tsUs: 2,
    });
    expect(screen.getByTestId("operator-status").textContent).toContain("no pose detected");
  });

  it("surfaces semantic events", async () => {
    render(<OperatorPanel />);
    await screen.findByTestId("operator-status");
    emit("vision-event", { kind: "rep_completed", payload: { count: 3 } });
    expect(screen.getByTestId("operator-status").textContent).toContain("rep_completed ✓");
  });

  it("offers honor-mode completion when the camera path fails", async () => {
    render(<OperatorPanel />);
    await screen.findByTestId("operator-status");
    emit("vision-fallback", { reason: "hub down after restart" });
    const honor = await screen.findByTestId("honor-mode");
    expect(honor.textContent).toContain("hub down after restart");
    fireEvent.click(screen.getByText("Done (honor)"));
    expect(invokeMock).toHaveBeenCalledWith("honor_complete");
  });
});
