#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use log;
use env_logger;

fn main() {
    // Default to info-level logging, but respect RUST_LOG if the developer
    // set it (e.g. `RUST_LOG=app_lib::audio=debug`) so debug logs are reachable
    // without editing source. Previously this overwrote the env unconditionally.
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    // Async logger will be initialized lazily when first needed (after Tauri runtime starts)
    log::info!("Starting application...");
    app_lib::run();
}
