//! Headless core: state machine, timers, workout engine, storage.
//! No tauri dependency — everything here tests with `cargo test -p engine`.

pub mod clock;
pub mod session;
pub mod store;
pub mod timer;
pub mod types;
pub mod workout;
