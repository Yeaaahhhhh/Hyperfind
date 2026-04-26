// File: apps/desktop/src-tauri/src/commands/index.rs

use crate::ipc::dto::{IndexProgressEvent, IndexStatsDto};
use crate::state::app_state::AppState;
use std::sync::Arc;
use tauri::{Manager, State, Window};

/// Non-blocking index rebuild with progress events.
#[tauri::command]
pub async fn rebuild_index(
    window: Window,
    state: State<'_, AppState>,
) -> Result<IndexStatsDto, String> {
    if state.is_indexing() {
        return Err("Indexing is already in progress".to_string());
    }

    let service = state.search_service.clone();
    let indexing_flag = Arc::new(parking_lot::Mutex::new(()));

    state.set_indexing(true);
    let state_ref = state.inner().clone_for_async();

    let win = window.clone();

    let result = tokio::task::spawn_blocking(move || {
        emit_progress(&win, "scan", "Scanning directories...", Some(0.0), false, None);

        let stats = service.rebuild_index().map_err(|e| e.to_string())?;
        let dto = IndexStatsDto::from(stats);

        emit_progress(&win, "done", "Index rebuild complete", Some(100.0), true, Some(dto.clone()));

        Ok(dto)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    state.set_indexing(false);
    result
}

/// Non-blocking scan of a single directory.
#[tauri::command]
pub async fn scan_directory(
    path: String,
    window: Window,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let service = state.search_service.clone();
    let win = window.clone();

    tokio::task::spawn_blocking(move || {
        emit_progress(&win, "scan", &format!("Scanning {}...", path), Some(0.0), false, None);

        let count = service.scan_directory(&path).map_err(|e| e.to_string())?;

        emit_progress(&win, "done", &format!("Scanned {} files", count), Some(100.0), true, None);

        Ok(count)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Non-blocking index load.
#[tauri::command]
pub async fn load_index(
    window: Window,
    state: State<'_, AppState>,
) -> Result<IndexStatsDto, String> {
    let service = state.search_service.clone();
    let win = window.clone();

    tokio::task::spawn_blocking(move || {
        emit_progress(&win, "load", "Loading index...", Some(0.0), false, None);

        let stats = service.load_index().map_err(|e| e.to_string())?;
        let dto = IndexStatsDto::from(stats);

        emit_progress(&win, "done", "Index loaded", Some(100.0), true, Some(dto.clone()));

        Ok(dto)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Non-blocking full system index with progress events.
#[tauri::command]
pub async fn index_all_volumes(
    window: Window,
    state: State<'_, AppState>,
) -> Result<IndexStatsDto, String> {
    if state.is_indexing() {
        return Err("Indexing is already in progress".to_string());
    }

    state.set_indexing(true);
    let service = state.search_service.clone();
    let win = window.clone();

    let result = tokio::task::spawn_blocking(move || {
        emit_progress(&win, "discover", "Discovering drives...", Some(5.0), false, None);

        let stats = service.index_all_volumes().map_err(|e| e.to_string())?;
        let dto = IndexStatsDto::from(stats);

        emit_progress(&win, "done",
            &format!("Indexing complete: {} items", dto.total_documents),
            Some(100.0), true, Some(dto.clone()));

        Ok(dto)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    state.set_indexing(false);
    result
}

fn emit_progress(
    window: &Window,
    phase: &str,
    message: &str,
    progress_pct: Option<f64>,
    done: bool,
    stats: Option<IndexStatsDto>,
) {
    let event = IndexProgressEvent {
        phase: phase.to_string(),
        message: message.to_string(),
        progress_pct,
        done,
        error: None,
        stats,
    };
    let _ = window.emit("index-progress", &event);
}

// Helper trait so we can clone what we need from AppState
trait CloneForAsync {
    fn clone_for_async(&self) -> AsyncStateHandle;
}

struct AsyncStateHandle;

impl CloneForAsync for AppState {
    fn clone_for_async(&self) -> AsyncStateHandle {
        AsyncStateHandle
    }
}