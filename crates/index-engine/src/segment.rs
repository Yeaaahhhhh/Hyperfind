// File: crates/index-engine/src/segment.rs

//! Segment-based binary index format.
//!
//! Each segment file contains:
//! - Header (magic, version, doc_count)
//! - Document store: serialized FileDocuments
//! - Trigram index data
//! - Bitmap index data
//! - Footer with offsets and checksum
//!
//! Segments are immutable once written. Updates create new segments.
//! A commit file references all active segments.

use crate::bitmap::BitmapIndex;
use crate::trigram::TrigramIndex;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_common::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use tracing::info;

const SEGMENT_MAGIC: &[u8; 4] = b"HFSG";
const SEGMENT_VERSION: u32 = 1;

/// Segment file layout offsets stored in the footer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentFooter {
    pub doc_store_offset: u64,
    pub doc_store_len: u64,
    pub trigram_offset: u64,
    pub trigram_len: u64,
    pub bitmap_offset: u64,
    pub bitmap_len: u64,
    pub doc_count: u64,
    pub checksum: u32,
}

/// Writes a segment file to disk.
pub fn write_segment(
    segment_id: &str,
    documents: &[FileDocument],
    trigram_index: &TrigramIndex,
    bitmap_index: &BitmapIndex,
) -> Result<std::path::PathBuf, HyperFindError> {
    let segments_dir = paths::segments_dir()?;
    fs::create_dir_all(&segments_dir)?;

    let path = segments_dir.join(format!("{}.seg", segment_id));

    // Serialize documents
    let doc_data = serde_json::to_vec(documents).map_err(|e| {
        HyperFindError::IndexError(format!("Failed to serialize documents: {}", e))
    })?;

    // Serialize trigram index
    let trigram_data = trigram_index.serialize();

    // Serialize bitmap index
    let bitmap_data = bitmap_index.serialize();

    // Build file
    let mut file = fs::File::create(&path)?;

    // Header
    file.write_all(SEGMENT_MAGIC)?;
    file.write_all(&SEGMENT_VERSION.to_le_bytes())?;

    let doc_store_offset = 8u64; // after magic + version
    file.write_all(&doc_data)?;

    let trigram_offset = doc_store_offset + doc_data.len() as u64;
    file.write_all(&trigram_data)?;

    let bitmap_offset = trigram_offset + trigram_data.len() as u64;
    file.write_all(&bitmap_data)?;

    // Compute checksum over all data
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&doc_data);
    hasher.update(&trigram_data);
    hasher.update(&bitmap_data);
    let checksum = hasher.finalize();

    let footer = SegmentFooter {
        doc_store_offset,
        doc_store_len: doc_data.len() as u64,
        trigram_offset,
        trigram_len: trigram_data.len() as u64,
        bitmap_offset,
        bitmap_len: bitmap_data.len() as u64,
        doc_count: documents.len() as u64,
        checksum,
    };

    let footer_data = serde_json::to_vec(&footer).map_err(|e| {
        HyperFindError::IndexError(format!("Failed to serialize footer: {}", e))
    })?;

    // Write footer length then footer
    file.write_all(&(footer_data.len() as u32).to_le_bytes())?;
    file.write_all(&footer_data)?;

    file.flush()?;

    info!(
        "Segment written: {} ({} docs, {} bytes)",
        segment_id,
        documents.len(),
        doc_data.len() + trigram_data.len() + bitmap_data.len()
    );

    Ok(path)
}

/// Reads a segment file and returns its components.
pub fn read_segment(
    path: &std::path::Path,
) -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let data = fs::read(path)?;

    if data.len() < 8 {
        return Err(HyperFindError::IndexError("Segment too small".into()));
    }

    // Verify magic
    if &data[0..4] != SEGMENT_MAGIC {
        return Err(HyperFindError::IndexError("Invalid segment magic".into()));
    }

    // Read footer length from end
    if data.len() < 12 {
        return Err(HyperFindError::IndexError("Segment too small for footer".into()));
    }


    let file_len = data.len();
    // Try reading last 8KB as potential footer area
    let scan_start = if file_len > 8192 { file_len - 8192 } else { 8 };

    // Find the footer_len: iterate from scan_start
    for candidate_pos in scan_start..file_len.saturating_sub(4) {
        let footer_len = u32::from_le_bytes([
            data[candidate_pos],
            data[candidate_pos + 1],
            data[candidate_pos + 2],
            data[candidate_pos + 3],
        ]) as usize;

        if footer_len > 0
            && footer_len < 4096
            && candidate_pos + 4 + footer_len == file_len
        {
            let footer_bytes = &data[candidate_pos + 4..];
            if let Ok(footer) = serde_json::from_slice::<SegmentFooter>(footer_bytes) {
                // Verify checksum
                let content_end = candidate_pos;
                let all_content = &data[8..content_end];

                let mut hasher = crc32fast::Hasher::new();
                hasher.update(all_content);
                let computed = hasher.finalize();

                if computed != footer.checksum {
                    return Err(HyperFindError::IndexError(format!(
                        "Segment checksum mismatch: expected {}, got {}",
                        footer.checksum, computed
                    )));
                }

                // Extract sections
                let base = 8usize;
                let doc_start = 0usize;
                let doc_end = footer.doc_store_len as usize;
                let tri_start = doc_end;
                let tri_end = tri_start + footer.trigram_len as usize;
                let bmp_start = tri_end;
                let bmp_end = bmp_start + footer.bitmap_len as usize;

                let doc_slice = &data[base + doc_start..base + doc_end];
                let tri_slice = &data[base + tri_start..base + tri_end];
                let bmp_slice = &data[base + bmp_start..base + bmp_end];

                let documents: Vec<FileDocument> = serde_json::from_slice(doc_slice)
                    .map_err(|e| {
                        HyperFindError::IndexError(format!("Failed to parse documents: {}", e))
                    })?;

                return Ok((documents, tri_slice.to_vec(), bmp_slice.to_vec()));
            }
        }
    }

    Err(HyperFindError::IndexError("Could not locate segment footer".into()))
}

/// The commit file: lists active segment IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPoint {
    pub generation: u64,
    pub segments: Vec<String>,
    pub timestamp: String,
}

/// Writes the commit point.
pub fn write_commit(commit: &CommitPoint) -> Result<(), HyperFindError> {
    let index_dir = paths::index_dir()?;
    let path = index_dir.join("commit.json");
    let data = serde_json::to_string_pretty(commit).map_err(|e| {
        HyperFindError::IndexError(format!("Failed to serialize commit: {}", e))
    })?;
    fs::write(&path, data)?;
    Ok(())
}

/// Reads the commit point.
pub fn read_commit() -> Result<Option<CommitPoint>, HyperFindError> {
    let index_dir = paths::index_dir()?;
    let path = index_dir.join("commit.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)?;
    let commit: CommitPoint = serde_json::from_str(&data).map_err(|e| {
        HyperFindError::IndexError(format!("Failed to parse commit: {}", e))
    })?;
    Ok(Some(commit))
}

/// Deletes all segment files and the commit point.
pub fn delete_all_segments() -> Result<(), HyperFindError> {
    let segments_dir = paths::segments_dir()?;
    if segments_dir.exists() {
        fs::remove_dir_all(&segments_dir)?;
    }
    let commit_path = paths::index_dir()?.join("commit.json");
    if commit_path.exists() {
        fs::remove_file(&commit_path)?;
    }
    Ok(())
}