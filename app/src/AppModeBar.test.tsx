import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AppModeBar } from "./AppModeBar";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockReset(); });
it("keeps the chosen mode pending while the backend restarts", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  render(<AppModeBar mode="debug" primary />);
  expect(screen.getByRole("button", { name: "Debug" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Workout" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("set_app_mode", { mode: "workout" }));
  expect(screen.getByRole("status")).toHaveTextContent("Restarting");
  expect(screen.getByRole("button", { name: "Debug" })).toBeDisabled();
});
it("allows retry when the preference cannot be saved", async () => {
  vi.mocked(invoke).mockRejectedValue("Disk full");
  render(<AppModeBar mode="workout" primary />);
  fireEvent.click(screen.getByRole("button", { name: "Debug" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Disk full");
  expect(screen.getByRole("button", { name: "Debug" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Workout" })).toHaveAttribute("aria-pressed", "true");
});
it("opens the optional gym window from Debug", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  render(<AppModeBar mode="debug" primary />);
  fireEvent.click(screen.getByRole("button", { name: "Open gym window" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("show_gym"));
});
