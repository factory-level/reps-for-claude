//! Time behind a trait so every engine test can use a fake clock.

use std::cell::Cell;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock {
    /// Monotonic-enough seconds. Only differences matter.
    fn now(&self) -> f64;
    /// Local date as ISO "YYYY-MM-DD" (drives daily capacity reset).
    fn today(&self) -> String;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before 1970")
            .as_secs_f64()
    }

    fn today(&self) -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }
}

pub struct FakeClock {
    now: Cell<f64>,
    date: Mutex<String>,
}

impl FakeClock {
    pub fn new(start: f64, date: &str) -> Self {
        Self { now: Cell::new(start), date: Mutex::new(date.to_string()) }
    }

    pub fn advance(&self, secs: f64) {
        self.now.set(self.now.get() + secs);
    }

    pub fn set_date(&self, date: &str) {
        *self.date.lock().unwrap() = date.to_string();
    }
}

impl Clock for FakeClock {
    fn now(&self) -> f64 {
        self.now.get()
    }

    fn today(&self) -> String {
        self.date.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances() {
        let c = FakeClock::new(100.0, "2026-07-19");
        assert_eq!(c.now(), 100.0);
        c.advance(5.0);
        assert_eq!(c.now(), 105.0);
        assert_eq!(c.today(), "2026-07-19");
        c.set_date("2026-07-20");
        assert_eq!(c.today(), "2026-07-20");
    }
}
