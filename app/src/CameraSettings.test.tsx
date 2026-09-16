import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { CameraSettings } from "./CameraSettings";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
beforeEach(() => vi.mocked(invoke).mockReset());
describe("camera configuration", () => {
  it("saves explicit consensus without changing the current workout", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ consensus: false, usbDevice: "/dev/video0", usbRotation: 180, phoneUrl: "", phoneRotation: 0 }).mockResolvedValue(undefined);
    render(<CameraSettings idle />);
    fireEvent.change(await screen.findByLabelText("Counting policy"), { target: { value: "consensus" } });
    fireEvent.change(screen.getByLabelText("Phone stream URL"), { target: { value: "http://192.168.1.253:8081/" } });
    fireEvent.click(screen.getByText("Save cameras"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_camera_settings", { settings: { consensus: true, usbDevice: "/dev/video0", usbRotation: 180, phoneUrl: "http://192.168.1.253:8081/", phoneRotation: 0 } }));
    expect(await screen.findByText("Saved for the next workout.")).toBeInTheDocument();
  });
  it("disables editing during a workout", async () => {
    vi.mocked(invoke).mockResolvedValue({ consensus: true, usbDevice: "/dev/video0", usbRotation: 180, phoneUrl: "rtsp://phone/live", phoneRotation: 0 });
    render(<CameraSettings idle={false} />);
    expect(await screen.findByLabelText("Counting policy")).toBeDisabled();
  });
});
