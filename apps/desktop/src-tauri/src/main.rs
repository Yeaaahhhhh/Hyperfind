// File: apps/desktop/src-tauri/src/main.rs

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod ipc;
mod state;

use hyperfind_common::config;
use hyperfind_common::paths;
use hyperfind_core_engine::service::SearchService;
use state::app_state::AppState;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(e) = paths::ensure_dirs() {
        eprintln!("Failed to create app directories: {}", e);
    }

    let app_config = config::load_config().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config: {}, using defaults", e);
        hyperfind_common::models::AppConfig::default()
    });

    let search_service = Arc::new(SearchService::new(app_config));

    // DO NOT load index here on the main thread — let the frontend trigger it async
    let app_state = AppState::new(search_service);

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::search::search_files,
            commands::search::get_stats,
            commands::index::rebuild_index,
            commands::index::scan_directory,
            commands::index::load_index,
            commands::index::index_all_volumes,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::add_directory,
            commands::settings::remove_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}