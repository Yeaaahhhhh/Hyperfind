// File: apps/desktop/src-tauri/src/state/app_state.rs

use hyperfind_core_engine::service::SearchService;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AppState {
    pub search_service: Arc<SearchService>,
    pub indexing_in_progress: Mutex<bool>,
}

impl AppState {
    pub fn new(search_service: Arc<SearchService>) -> Self {
        Self {
            search_service,
            indexing_in_progress: Mutex::new(false),
        }
    }

    pub fn is_indexing(&self) -> bool {
        *self.indexing_in_progress.lock()
    }

    pub fn set_indexing(&self, val: bool) {
        *self.indexing_in_progress.lock() = val;
    }
}