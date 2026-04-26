// File: apps/desktop/src-tauri/src/commands/settings.rs

use crate::ipc::dto::AppConfigDto;
use crate::state::app_state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfigDto, String> {
    let service = state.search_service.clone();

    tokio::task::spawn_blocking(move || {
        let config = service.get_config();
        Ok(AppConfigDto::from(config))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn save_config(
    config: AppConfigDto,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let service = state.search_service.clone();

    tokio::task::spawn_blocking(move || {
        let app_config = hyperfind_common::models::AppConfig::from(config);
        service.update_config(app_config).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn add_directory(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let service = state.search_service.clone();

    tokio::task::spawn_blocking(move || {
        service.add_directory(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn remove_directory(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let service = state.search_service.clone();

    tokio::task::spawn_blocking(move || {
        service.remove_directory(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}