// File: crates/index-engine/src/document.rs

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn reset_id_counter(next: u64) { NEXT_ID.store(next, Ordering::SeqCst); }
pub fn next_id() -> u64 { NEXT_ID.fetch_add(1, Ordering::Relaxed) } // Relaxed 足够，性能更好
pub fn current_id_counter() -> u64 { NEXT_ID.load(Ordering::SeqCst) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub version: u32,
    pub doc_count: u64,
    pub next_id: u64,
    pub created_at: String,
    pub segment_count: u32,
    pub trigram_count: u64,
}

impl IndexMeta {
    pub fn new(doc_count: u64, next_id: u64, segment_count: u32, trigram_count: u64) -> Self {
        Self {
            version: 3,
            doc_count, next_id,
            created_at: chrono::Utc::now().to_rfc3339(),
            segment_count, trigram_count,
        }
    }
}