//! Scripted in-memory hub for app-level tests: records calls, replays events.

use std::sync::mpsc;

use crate::{EnableMetric, HubError, HubHealth, VisionEvent, VisionHub};

pub struct FakeHub {
    pub calls: Vec<String>,
    scripted: Vec<VisionEvent>,
    tx: mpsc::Sender<VisionEvent>,
    rx: Option<mpsc::Receiver<VisionEvent>>,
    pub fail_next: bool,
}

impl FakeHub {
    /// Events in `scripted` are emitted when a metric is enabled.
    pub fn new(scripted: Vec<VisionEvent>) -> Self {
        let (tx, rx) = mpsc::channel();
        FakeHub {
            calls: vec![],
            scripted,
            tx,
            rx: Some(rx),
            fail_next: false,
        }
    }

    /// Push an extra event mid-test.
    pub fn push(&self, event: VisionEvent) {
        let _ = self.tx.send(event);
    }

    fn check_fail(&mut self) -> Result<(), HubError> {
        if self.fail_next {
            self.fail_next = false;
            return Err(HubError::Down);
        }
        Ok(())
    }
}

impl VisionHub for FakeHub {
    fn enable_metric(&mut self, req: &EnableMetric) -> Result<(), HubError> {
        self.check_fail()?;
        self.calls
            .push(format!("enable:{}:{}", req.metric_id, req.plugin_id));
        for event in self.scripted.drain(..) {
            let _ = self.tx.send(event);
        }
        Ok(())
    }

    fn disable_metric(&mut self, metric_id: &str) -> Result<(), HubError> {
        self.check_fail()?;
        self.calls.push(format!("disable:{metric_id}"));
        Ok(())
    }

    fn update_metric_config(
        &mut self,
        metric_id: &str,
        _config: &serde_json::Value,
    ) -> Result<(), HubError> {
        self.calls.push(format!("update:{metric_id}"));
        Ok(())
    }

    fn simulate(&mut self, metric_id: &str, event: &serde_json::Value) -> Result<(), HubError> {
        self.calls.push(format!("simulate:{metric_id}"));
        if let Some(converted) = crate::event_from_frame(&serde_json::json!({
            "stream": "event",
            "data": event,
        })) {
            let _ = self.tx.send(converted);
        }
        Ok(())
    }

    fn health(&mut self) -> Result<HubHealth, HubError> {
        Ok(HubHealth {
            vision_host: "up".into(),
            camera: "closed".into(),
            enabled_metrics: vec![],
        })
    }

    fn take_receiver(&mut self) -> Option<mpsc::Receiver<VisionEvent>> {
        self.rx.take()
    }
}
