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
            env: vec![(
                "HUB_PLUGIN_ARGS".into(),
                format!(
                    "--plugin-path {} --plugin reps_vision.hub_plugin.plugin:RepsVisionPlugin",
                    plugin_path.display()
                ),
            )],
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
}
