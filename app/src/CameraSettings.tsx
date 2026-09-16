import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
interface Settings { consensus: boolean; usbDevice: string; usbRotation: number; phoneUrl: string; phoneRotation: number }
export function CameraSettings({ idle }: { idle: boolean }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    let active = true;
    invoke<Settings>("get_camera_settings").then(value => { if (active) setSettings(value); })
      .catch(error => { if (active) setMessage(String(error)); });
    return () => { active = false; };
  }, []);
  async function save() {
    setSaving(true);
    try { await invoke("save_camera_settings", { settings }); setMessage("Saved for the next workout."); }
    catch (error) { setMessage(String(error)); }
    finally { setSaving(false); }
  }
  return <details><summary>Cameras</summary>
    {settings && <fieldset disabled={!idle || saving}>
      <label>Counting policy <select value={settings.consensus ? "consensus" : "single"}
        onChange={e => setSettings({ ...settings, consensus: e.target.value === "consensus" })}>
        <option value="single">Single camera</option><option value="consensus">Both cameras must confirm</option>
      </select></label>
      <label>USB device <input value={settings.usbDevice} onChange={e => setSettings({ ...settings, usbDevice: e.target.value })} /></label>
      <label>USB rotation <select value={settings.usbRotation} onChange={e => setSettings({ ...settings, usbRotation: Number(e.target.value) })}>
        {[0, 90, 180, 270].map(value => <option key={value} value={value}>{value}°</option>)}
      </select></label>
      <label>Phone stream URL <input type="url" placeholder="http://192.168.1.10:8081/" value={settings.phoneUrl}
        onChange={e => setSettings({ ...settings, phoneUrl: e.target.value })} /></label>
      <label>Phone rotation <select value={settings.phoneRotation} onChange={e => setSettings({ ...settings, phoneRotation: Number(e.target.value) })}>
        {[0, 90, 180, 270].map(value => <option key={value} value={value}>{value}°</option>)}
      </select></label>
      <button onClick={() => void save()}>Save cameras</button>
      {settings.consensus && <p>Both views need timing calibration before reps can be credited. Recalibrate after reconnecting the phone.</p>}
    </fieldset>}
    <p role="status">{message}</p>
  </details>;
}
export function ConsensusStatus({ idle }: { idle: boolean }) {
  const [status, setStatus] = useState("");
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const labels: Record<string, string> = { confirming: "Waiting for the other camera", confirmed: "Both cameras confirmed",
      insufficient_evidence: "Waiting for both views", ineligible_evidence: "Camera evidence needs calibration or better visibility",
      disconnected: "Camera disconnected", disagreement: "Camera views disagree", ambiguous_cycle: "Movement could not be matched",
      stale_evidence: "Camera frames arrived too late", evidence_limit: "Restart this workout" };
    listen<{ state: string }>("vision-consensus", event => { if (!cancelled) setStatus(labels[event.payload.state] ?? "Waiting for camera evidence"); })
      .then(fn => { if (cancelled) fn(); else unlisten = fn; }).catch(() => { if (!cancelled) setStatus("Camera status unavailable"); });
    return () => { cancelled = true; unlisten?.(); };
  }, []);
  useEffect(() => { if (idle) setStatus(""); }, [idle]);
  return !idle && status ? <p role="status">Consensus · {status}</p> : null;
}

export function CameraPreviews({ idle }: { idle: boolean }) {
  const [frames, setFrames] = useState<Record<string, { jpeg: string; at: number }>>({});
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (idle) { setFrames({}); return; }
    let active = true;
    let unlisten: (() => void) | undefined;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    listen<{ cameraId: string; data: { jpegB64?: string } }>("vision-camera", ({ payload }) => {
      if (active && payload.data.jpegB64) setFrames(current => ({ ...current, [payload.cameraId]: { jpeg: payload.data.jpegB64!, at: Date.now() } }));
    }).then(fn => { if (active) unlisten = fn; else fn(); }).catch(() => {});
    return () => { active = false; clearInterval(timer); unlisten?.(); };
  }, [idle]);
  return <div className="camera-previews">{Object.entries(frames).map(([camera, frame]) => <figure key={camera}>
    <img src={`data:image/jpeg;base64,${frame.jpeg}`} alt={`${camera} camera preview`} />
    <figcaption>{camera} · {now - frame.at > 2000 ? "Stale — check connection" : "Live"}</figcaption>
  </figure>)}</div>;
}
