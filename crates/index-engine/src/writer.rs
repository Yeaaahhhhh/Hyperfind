// File: crates/index-engine/src/writer.rs

use crate::bitmap::BitmapIndex;
use crate::document::{self, IndexMeta};
use crate::segment::{self, CommitPoint};
use crate::trigram::TrigramIndex;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_common::paths;
use tracing::info;
use uuid::Uuid;

pub fn write_index(
    documents: &[FileDocument],
    trigram_index: &TrigramIndex,
    bitmap_index: &BitmapIndex,
) -> Result<(), HyperFindError> {
    let index_dir = paths::index_dir()?;
    std::fs::create_dir_all(&index_dir)?;

    segment::delete_all_segments()?;
    std::fs::create_dir_all(paths::segments_dir()?)?;

    let segment_id = Uuid::new_v4().to_string();
    segment::write_segment(&segment_id, documents, trigram_index, bitmap_index)?;

    let commit = CommitPoint {
        generation: 1,
        segments: vec![segment_id],
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    segment::write_commit(&commit)?;

    let meta = IndexMeta::new(
        documents.len() as u64,
        document::current_id_counter(),
        1,
        trigram_index.trigram_count(),
    );
    let meta_json = serde_json::to_string_pretty(&meta)
        .map_err(|e| HyperFindError::SerializationError(format!("meta serialize: {}", e)))?;
    std::fs::write(index_dir.join("meta.json"), meta_json)?;

    info!("Index written: {} docs (segment v2 / bincode / mmap-ready)", documents.len());
    Ok(())
}

pub fn index_exists() -> Result<bool, HyperFindError> {
    let index_dir = paths::index_dir()?;
    Ok(index_dir.join("meta.json").exists() && index_dir.join("commit.json").exists())
}

pub fn delete_index() -> Result<(), HyperFindError> {
    let index_dir = paths::index_dir()?;
    if index_dir.exists() { std::fs::remove_dir_all(&index_dir)?; }
    Ok(())
}

pub fn index_size_bytes() -> Result<u64, HyperFindError> {
    let index_dir = paths::index_dir()?;
    if !index_dir.exists() { return Ok(0); }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(&index_dir) {
        if let Ok(e) = entry {
            if e.file_type().is_file() {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    Ok(total)
}