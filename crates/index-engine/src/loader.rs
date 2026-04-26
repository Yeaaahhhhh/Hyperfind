// File: crates/index-engine/src/loader.rs

use crate::document::{self, IndexMeta};
use crate::segment;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_common::paths;
use std::fs;
use tracing::info;

/// Loads the index from segment files.
/// Returns (documents, trigram_data, bitmap_data).
pub fn load_index() -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let index_dir = paths::index_dir()?;
    let meta_path = index_dir.join("meta.json");

    if !meta_path.exists() {
        return Err(HyperFindError::IndexError(
            "Index metadata not found. Please build the index first.".to_string(),
        ));
    }

    // Load metadata
    let meta_content = fs::read_to_string(&meta_path)?;
    let meta: IndexMeta = serde_json::from_str(&meta_content).map_err(|e| {
        HyperFindError::SerializationError(format!("Failed to parse meta: {}", e))
    })?;

    info!("Loading index: version={}, expected_docs={}", meta.version, meta.doc_count);
    document::reset_id_counter(meta.next_id);

    // Load commit point
    let commit = segment::read_commit()?.ok_or_else(|| {
        HyperFindError::IndexError("Commit point not found".to_string())
    })?;

    let segments_dir = paths::segments_dir()?;
    let mut all_docs = Vec::new();
    let mut all_trigram_data = Vec::new();
    let mut all_bitmap_data = Vec::new();

    for seg_id in &commit.segments {
        let seg_path = segments_dir.join(format!("{}.seg", seg_id));
        if !seg_path.exists() {
            tracing::warn!("Segment file missing: {}", seg_id);
            continue;
        }

        let (docs, tri_data, bmp_data) = segment::read_segment(&seg_path)?;
        all_docs.extend(docs);
        // For single-segment MVP, we just use the last segment's indexes
        all_trigram_data = tri_data;
        all_bitmap_data = bmp_data;
    }

    info!("Index loaded: {} documents from {} segments", all_docs.len(), commit.segments.len());
    Ok((all_docs, all_trigram_data, all_bitmap_data))
}