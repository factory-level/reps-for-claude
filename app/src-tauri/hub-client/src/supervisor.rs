//! Owns the bundled hubd process: spawn, READY handshake, restart-once,
//! drop-safe kill. The lock screen must never depend on an unsupervised
//! external service.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{
    EnableMetric, HubClient, HubError, HubHealth, VisionEvent, VisionHub,
};

const READY_TIMEOUT: Duration = Duration::from_secs(60);
const RESTART_BACKOFF: Duration = Duration::from_millis(500);
/// hubd's default primary listen port. The app never overrides it, so a stale
/// process bound here at launch is always a leaked hub from a prior run
/// (e.g. a `tauri dev` hot-reload that hard-killed the app before Drop ran).
const HUB_PORT: u16 = 8443;

/// True if something is currently listening on `127.0.0.1:port`.
fn port_in_use(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// Before spawning our own hubd, free the hub port if a leaked hub still holds
/// it — otherwise the new hubd dies on EADDRINUSE, never signals READY, and the
/// app silently falls back to honor mode after the ready timeout. Only the
/// process on the app's own hub port is signaled. No-op when the port is free.
fn free_stale_hub_port(port: u16) {
    if !port_in_use(port) {
        return;
    }
    eprintln!("hub: port {port} already in use — clearing a leaked hub before starting");
    #[cfg(unix)]
    {
        // Signal the whole process GROUP of whatever holds the port, not just
        // the listener: hubd is spawned as its own group (see pre_exec below),
        // so its vision-host — the process actually holding the camera — dies
        // with it. Killing hubd alone orphaned the host and the next lock
        // failed with "cannot open camera".
        let groups = stale_port_groups(port);
        for pgid in &groups {
            unsafe { libc::kill(-pgid, libc::SIGTERM) };
        }
        for _ in 0..30 {
            if !port_in_use(port) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        for pgid in &groups {
            unsafe { libc::kill(-pgid, libc::SIGKILL) };
        }
    }
    for _ in 0..30 {
        if !port_in_use(port) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("hub: warning — port {port} still in use after cleanup; hubd may fail to start");
}

/// Process groups (other than our own) of the processes bound to `port`.
#[cfg(unix)]
fn stale_port_groups(port: u16) -> Vec<i32> {
    let out = Command::new("fuser")
        .arg(format!("{port}/tcp"))
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mine = unsafe { libc::getpgrp() };
    let mut groups: Vec<i32> = out
        .split_whitespace()
        .filter_map(|s| s.parse::<i32>().ok())
        .map(|pid| unsafe { libc::getpgid(pid) })
        .filter(|pgid| *pgid > 0 && *pgid != mine)
        .collect();
    groups.sort_unstable();
    groups.dedup();
    groups
}

pub struct HubSupervisorConfig {
    /// Directory of the usb-mcp-hub checkout or bundle.
    pub hub_dir: std::path::PathBuf,
    /// Command + args to launch hubd (e.g. ["pnpm", "--filter", "@hub/hubd", "start"]).
    pub command: Vec<String>,
    /// Extra environment (e.g. HUB_PLUGIN_ARGS for the reps plugin).
    pub env: Vec<(String, String)>,
}

impl HubSupervisorConfig {
    /// Production: the staged bundle under the app's resources dir
    /// (hub-bundle/hubd.mjs + public/ + vision/), reps plugin loaded from
    /// `plugin_src` (the shipped reps_vision sources).
    pub fn bundled(resources_dir: &std::path::Path, plugin_src: &std::path::Path) -> Self {
        let bundle = resources_dir.join("hub-bundle");
        HubSupervisorConfig {
            hub_dir: bundle.clone(),
            command: vec![
                "node".into(),
                bundle.join("hubd.mjs").display().to_string(),
            ],
            env: vec![
                (
                    "HUB_VISION_DIR".into(),
                    bundle.join("vision").display().to_string(),
                ),
                (
                    "HUB_PUBLIC_DIR".into(),
                    bundle.join("public").display().to_string(),
                ),
                (
                    "HUB_PLUGIN_ARGS".into(),
                    format!(
                        "--plugin-path {} --plugin reps_vision.hub_plugin.plugin:RepsVisionPlugin",
                        plugin_src.display()
                    ),
                ),
                // Companion screens staged beside the bundle (hubd ignores the
                // env when the directory is absent).
                (
                    "HUB_APP_UI_DIR".into(),
                    resources_dir.join("companion").display().to_string(),
                ),
            ],
        }
    }

    /// Dev default: sibling checkout via $HUB_DIR, reps plugin loaded from
    /// this repo's vision/src.
    pub fn dev(repo_root: &std::path::Path) -> Self {
        let hub_dir = std::env::var("HUB_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| repo_root.join("..").join("usb-mcp-hub"));
        let plugin_path = repo_root.join("vision").join("src");
        HubSupervisorConfig {
            hub_dir,
            command: vec![
                "pnpm".into(),
                "--filter".into(),
                "@hub/hubd".into(),
                "start".into(),
            ],
            env: vec![
                (
                    "HUB_PLUGIN_ARGS".into(),
                    format!(
                        "--plugin-path {} --plugin reps_vision.hub_plugin.plugin:RepsVisionPlugin",
                        plugin_path.display()
                    ),
                ),
                // Companion screens (Workout / Calibrate / History) ship with
                // reps; hubd hosts them under /app/.
                (
                    "HUB_APP_UI_DIR".into(),
                    repo_root.join("companion").display().to_string(),
                ),
            ],
        }
    }
}

struct Active {
    child: Child,
    client: HubClient,
}

pub struct HubSupervisor {
    config: HubSupervisorConfig,
    active: Arc<Mutex<Option<Active>>>,
    stopping: Arc<AtomicBool>,
    dead: Arc<AtomicBool>,
    event_tx: mpsc::Sender<VisionEvent>,
    events_rx: Option<mpsc::Receiver<VisionEvent>>,
    restarts: Arc<Mutex<u32>>,
}

impl HubSupervisor {
    pub fn start(config: HubSupervisorConfig) -> Result<Self, HubError> {
        let (event_tx, events_rx) = mpsc::channel();
        let mut supervisor = HubSupervisor {
            config,
            active: Arc::new(Mutex::new(None)),
            stopping: Arc::new(AtomicBool::new(false)),
            dead: Arc::new(AtomicBool::new(false)),
            event_tx,
            events_rx: Some(events_rx),
            restarts: Arc::new(Mutex::new(0)),
        };
        supervisor.spawn_and_connect()?;
        Ok(supervisor)
    }

    fn spawn_and_connect(&mut self) -> Result<(), HubError> {
        let (program, args) = self
            .config
            .command
            .split_first()
            .ok_or_else(|| HubError::Api("empty hub command".into()))?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&self.config.hub_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (key, value) in &self.config.env {
            command.env(key, value);
        }
        // Clear a leaked hub from a previous run so we don't die on EADDRINUSE.
        free_stale_hub_port(HUB_PORT);
        // New process group so drop can signal hubd (and, via its own
        // shutdown handler, vision-host) without touching our group.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
        }
        let mut child = command.spawn().map_err(|e| HubError::Io(e.to_string()))?;

        let stdout = child.stdout.take().ok_or_else(|| HubError::Io("no stdout".into()))?;
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("hubd-stdout".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(json) = line.strip_prefix("HUBD READY ") {
                        let _ = ready_tx.send(json.to_string());
                    }
                }
            })
            .map_err(|e| HubError::Io(e.to_string()))?;

        let ready = ready_rx
            .recv_timeout(READY_TIMEOUT)
            .map_err(|_| HubError::Timeout("HUBD READY handshake".into()))?;
        let ready: serde_json::Value =
            serde_json::from_str(&ready).map_err(|e| HubError::Api(e.to_string()))?;
        let port = ready
            .get("debugPort")
            .and_then(|p| p.as_u64())
            .ok_or_else(|| HubError::Api("no debugPort in HUBD READY".into()))?;

        let mut client = HubClient::connect(&format!("ws://127.0.0.1:{port}/v1/ws"))?;

        // Forward client events into the supervisor's single stream.
        let client_rx = client
            .take_receiver()
            .ok_or_else(|| HubError::Api("client receiver already taken".into()))?;
        let forward_tx = self.event_tx.clone();
        std::thread::Builder::new()
            .name("hub-event-forward".into())
            .spawn(move || {
                for event in client_rx {
                    if forward_tx.send(event).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| HubError::Io(e.to_string()))?;

        *self.active.lock().unwrap() = Some(Active { child, client });
        self.watch();
        Ok(())
    }

    /// Monitor thread: on unexpected exit restart once, then give up.
    fn watch(&self) {
        let active = Arc::clone(&self.active);
        let stopping = Arc::clone(&self.stopping);
        let dead = Arc::clone(&self.dead);
        let restarts = Arc::clone(&self.restarts);
        let event_tx = self.event_tx.clone();
        let config_snapshot = (
            self.config.hub_dir.clone(),
            self.config.command.clone(),
            self.config.env.clone(),
        );
        let self_active = Arc::clone(&self.active);
        std::thread::Builder::new()
            .name("hubd-watch".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_millis(250));
                if stopping.load(Ordering::SeqCst) || dead.load(Ordering::SeqCst) {
                    return;
                }
                let exited = {
                    let mut guard = active.lock().unwrap();
                    match guard.as_mut() {
                        None => return,
                        Some(active_ref) => active_ref.child.try_wait().ok().flatten().is_some(),
                    }
                };
                if !exited {
                    continue;
                }
                let _ = event_tx.send(VisionEvent::ConnectionLost);
                let mut count = restarts.lock().unwrap();
                if *count >= 1 {
                    dead.store(true, Ordering::SeqCst);
                    let _ = event_tx.send(VisionEvent::Health(HubHealth::failed()));
                    return;
                }
                *count += 1;
                drop(count);
                std::thread::sleep(RESTART_BACKOFF);
                let mut respawner = HubSupervisor {
                    config: HubSupervisorConfig {
                        hub_dir: config_snapshot.0.clone(),
                        command: config_snapshot.1.clone(),
                        env: config_snapshot.2.clone(),
                    },
                    active: Arc::clone(&self_active),
                    stopping: Arc::clone(&stopping),
                    dead: Arc::clone(&dead),
                    event_tx: event_tx.clone(),
                    events_rx: None,
                    restarts: Arc::clone(&restarts),
                };
                if respawner.spawn_and_connect().is_err() {
                    dead.store(true, Ordering::SeqCst);
                    let _ = event_tx.send(VisionEvent::Health(HubHealth::failed()));
                }
                // respawner shares Arcs; prevent its Drop from killing the child
                std::mem::forget(respawner);
                return;
            })
            .ok();
    }

    fn with_client<T>(
        &mut self,
        f: impl FnOnce(&mut HubClient) -> Result<T, HubError>,
    ) -> Result<T, HubError> {
        if self.dead.load(Ordering::SeqCst) {
            return Err(HubError::Down);
        }
        let mut guard = self.active.lock().unwrap();
        match guard.as_mut() {
            Some(active) => f(&mut active.client),
            None => Err(HubError::Down),
        }
    }

    fn kill_child(&self) {
        if let Some(active) = self.active.lock().unwrap().as_mut() {
            #[cfg(unix)]
            {
                let pid = active.child.id() as i32;
                unsafe {
                    libc::kill(-pid, libc::SIGTERM);
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if active.child.try_wait().ok().flatten().is_some() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            let _ = active.child.kill();
            let _ = active.child.wait();
        }
    }

    pub fn stop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.kill_child();
    }
}

impl Drop for HubSupervisor {
    /// The camera can never stay hot past the app: kill hubd on drop.
    fn drop(&mut self) {
        self.stop();
    }
}

impl VisionHub for HubSupervisor {
    fn enable_metric(&mut self, req: &EnableMetric) -> Result<(), HubError> {
        let req = req.clone();
        self.with_client(move |client| client.enable_metric(&req))
    }

    fn disable_metric(&mut self, metric_id: &str) -> Result<(), HubError> {
        let id = metric_id.to_string();
        self.with_client(move |client| client.disable_metric(&id))
    }

    fn add_camera(&mut self, camera: &serde_json::Value) -> Result<(), HubError> {
        let camera = camera.clone();
        self.with_client(move |client| client.add_camera(&camera))
    }

    fn update_metric_config_for_camera(
        &mut self,
        metric_id: &str,
        camera_id: &str,
        config: &serde_json::Value,
    ) -> Result<(), HubError> {
        let (id, camera, config) = (metric_id.to_string(), camera_id.to_string(), config.clone());
        self.with_client(move |client| {
            client.update_metric_config_for_camera(&id, &camera, &config)
        })
    }

    fn update_metric_config(
        &mut self,
        metric_id: &str,
        config: &serde_json::Value,
    ) -> Result<(), HubError> {
        let id = metric_id.to_string();
        let config = config.clone();
        self.with_client(move |client| client.update_metric_config(&id, &config))
    }

    fn simulate(&mut self, metric_id: &str, event: &serde_json::Value) -> Result<(), HubError> {
        let id = metric_id.to_string();
        let event = event.clone();
        self.with_client(move |client| client.simulate(&id, &event))
    }

    fn health(&mut self) -> Result<HubHealth, HubError> {
        self.with_client(|client| client.health())
    }

    fn take_receiver(&mut self) -> Option<mpsc::Receiver<VisionEvent>> {
        self.events_rx.take()
    }

    fn register_application(
        &mut self,
        app_id: &str,
        version: &str,
        manifest: &serde_json::Value,
    ) -> Result<(), HubError> {
        let (app_id, version, manifest) =
            (app_id.to_string(), version.to_string(), manifest.clone());
        self.with_client(move |client| client.register_application(&app_id, &version, &manifest))
    }

    fn publish_event(&mut self, params: &serde_json::Value) -> Result<(), HubError> {
        let params = params.clone();
        self.with_client(move |client| client.publish_event(&params))
    }

    fn publish_command(&mut self, params: &serde_json::Value) -> Result<(), HubError> {
        let params = params.clone();
        self.with_client(move |client| client.publish_command(&params))
    }

    fn report_action_result(&mut self, params: &serde_json::Value) -> Result<(), HubError> {
        let params = params.clone();
        self.with_client(move |client| client.report_action_result(&params))
    }
}

#[cfg(test)]
mod port_tests {
    use super::{free_stale_hub_port, port_in_use};
    use std::net::TcpListener;

    #[test]
    fn port_in_use_tracks_a_live_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port_in_use(port), "a bound listener should read as in-use");
        drop(listener);
        // A sibling test spawning a child briefly duplicates this listener's fd
        // until its exec, so give the kernel a moment to actually close it.
        let freed = (0..50).any(|_| {
            !port_in_use(port) || {
                std::thread::sleep(std::time::Duration::from_millis(20));
                false
            }
        });
        assert!(freed, "a released port should read as free");
    }

    /// The leak that bit the rig: a hard-killed app leaves hubd AND its
    /// vision-host behind; clearing the port must take the whole group.
    #[cfg(unix)]
    #[test]
    fn free_stale_hub_port_kills_the_listeners_whole_group() {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        let port = {
            let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        // "hubd" (python listener) with a "vision-host" sibling (sleep) in one group.
        let script = format!(
            "sleep 300 & python3 -c \"import socket,time; s=socket.socket(); s.bind(('127.0.0.1',{port})); s.listen(); time.sleep(300)\""
        );
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&script)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = child.id() as i32;
        for _ in 0..50 {
            if port_in_use(port) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(port_in_use(port), "fixture listener never came up");

        free_stale_hub_port(port);

        assert!(!port_in_use(port));
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_millis(200));
        // Nothing left in the group — the "vision-host" sleep died too.
        let survivors = Command::new("pgrep").arg("-g").arg(pgid.to_string()).output().unwrap();
        assert!(
            survivors.stdout.is_empty(),
            "group {pgid} still has members: {}",
            String::from_utf8_lossy(&survivors.stdout)
        );
    }

    #[test]
    fn free_stale_hub_port_is_a_noop_when_free() {
        // Grab a port then release it so nothing is listening; the preflight
        // must return immediately without trying to kill anything.
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        free_stale_hub_port(port); // no panic, no hang
        assert!(!port_in_use(port));
    }
}
