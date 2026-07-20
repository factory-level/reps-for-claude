import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DebugPanel } from "./DebugPanel";

const invokeMock = vi.fn();
let listenCallback: ((event: { payload: unknown }) => void) | undefined;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (_name: string, cb: (event: { payload: unknown }) => void) => {
    listenCallback = cb;
    return Promise.resolve(() => {
      listenCallback = undefined;
    });
  },
}));

const VIDEOS = [
  { exercise: "squat", path: "/vision/tests/fixtures/videos/squat_demo.webm" },
  { exercise: "bench", path: "/vision/tests/fixtures/videos/youtube/bench.mp4" },
];

function emit(payload: unknown) {
  act(() => {
    listenCallback?.({ payload });
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  listenCallback = undefined;
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "debug_videos") return Promise.resolve(VIDEOS);
    return Promise.resolve(undefined);
  });
});

describe("DebugPanel", () => {
  it("lists videos from debug_videos and starts the selected one", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");

    fireEvent.click(screen.getByText("Start"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("debug_stream_start", {
        video: "/vision/tests/fixtures/videos/squat_demo.webm",
        exercise: "squat",
      }),
    );
  });

  it("renders frame events as an image and a progress readout", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");
    await waitFor(() => expect(listenCallback).toBeDefined());

    emit({ event: "open", exercise: "squat", fps: 30, frameCount: 100 });
    emit({ event: "progress", frame: 5, value: 3, unit: "reps", satisfied: false });
    emit({ event: "frame", frame: 5, jpegB64: "AAAA" });

    expect(screen.getByAltText("debug stream frame")).toHaveAttribute(
      "src",
      "data:image/jpeg;base64,AAAA",
    );
    expect(screen.getByText(/3 reps/)).toBeInTheDocument();
    expect(screen.getByText(/✗/)).toBeInTheDocument();
  });

  it("shows a satisfied checkmark once satisfied", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");
    await waitFor(() => expect(listenCallback).toBeDefined());

    emit({ event: "progress", frame: 20, value: 12, unit: "reps", satisfied: true });

    expect(screen.getByText(/12 reps/)).toBeInTheDocument();
    expect(screen.getByText(/✓/)).toBeInTheDocument();
  });

  it("shows the done readout", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");
    await waitFor(() => expect(listenCallback).toBeDefined());

    emit({ event: "done", total: 12, satisfied: true });

    expect(screen.getByText(/Done: 12/)).toBeInTheDocument();
  });

  it("shows the exited readout when the sidecar exits", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");
    await waitFor(() => expect(listenCallback).toBeDefined());

    emit({ event: "exited", code: 0 });

    expect(screen.getByText(/Exited/)).toBeInTheDocument();
  });

  it("stops the stream via debug_stream_stop", async () => {
    render(<DebugPanel />);
    await screen.findByText("squat");

    fireEvent.click(screen.getByText("Start"));
    fireEvent.click(screen.getByText("Stop"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("debug_stream_stop"));
  });
});
