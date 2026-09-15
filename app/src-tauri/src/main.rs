// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--check-runtime") {
        match app_lib::check_runtime() {
            Ok(report) => println!("{report}"),
            Err(error) => { eprintln!("Runtime check failed: {error}"); std::process::exit(1); }
        }
        return;
    }
    app_lib::run()
}
