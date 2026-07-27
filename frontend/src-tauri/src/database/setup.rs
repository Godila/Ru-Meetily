use log::info;
use tauri::{AppHandle, Emitter, Manager};

use super::manager::DatabaseManager;
use crate::state::AppState;

/// Initialize database on app startup.
///
/// The database is ALWAYS created synchronously here, including on first launch.
/// This fixes a race condition where the frontend would render and invoke
/// `api_get_*` commands (which require `AppState`) before the database had been
/// initialized, producing "state not managed for field `state`" errors and a
/// blank main panel.
///
/// Previously, on first launch the database was NOT initialized here; instead a
/// `first-launch-detected` event was emitted and the frontend was expected to
/// call `initialize_fresh_database` later. But nothing on the frontend actually
/// listens for that event, so `AppState` was never registered and every command
/// that depends on it failed.
///
/// We still detect first launch for logging/analytics, but no longer gate
/// database creation on it.
pub async fn initialize_database_on_startup(app: &AppHandle) -> Result<(), String> {
    let is_first_launch = DatabaseManager::is_first_launch(app)
        .await
        .map_err(|e| format!("Failed to check first launch status: {}", e))?;

    if is_first_launch {
        info!("First launch detected - initializing fresh database synchronously");
    } else {
        info!("Subsequent launch detected - initializing database");
    }

    // Always create/open the database and register AppState. On first launch this
    // creates a fresh database with migrations; on subsequent launches it opens
    // the existing one. Either way AppState becomes available before the window
    // finishes loading, so frontend commands that depend on it succeed.
    let db_manager = DatabaseManager::new_from_app_handle(app)
        .await
        .map_err(|e| format!("Failed to initialize database manager: {}", e))?;

    app.manage(AppState { db_manager });
    info!("Database initialized successfully");

    // Notify frontend (kept for backward compat / any listeners that may exist).
    if is_first_launch {
        let _ = app.emit("first-launch-detected", ());
    }

    Ok(())
}
