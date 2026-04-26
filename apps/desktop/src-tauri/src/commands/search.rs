// File: apps/desktop/src-tauri/src/commands/search.rs

use crate::ipc::dto::{IndexStatsDto, SearchResultDto};
use crate::state::app_state::AppState;
use tauri::State;

#[tauri::command]
pub async fn search_files(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResultDto>, String> {
    let service = state.search_service.clone();
    let limit = limit.unwrap_or(500);

    tokio::task::spawn_blocking(move || {
        let results = service.search(&query).map_err(|e| e.to_string())?;
        let dto_results: Vec<SearchResultDto> = results
            .into_iter()
            .take(limit)
            .map(SearchResultDto::from)
            .collect();
        Ok(dto_results)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<IndexStatsDto, String> {
    let service = state.search_service.clone();

    tokio::task::spawn_blocking(move || {
        let stats = service.get_stats();
        Ok(IndexStatsDto::from(stats))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}