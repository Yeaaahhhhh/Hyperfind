// File: crates/index-engine/src/loader.rs

use crate::document::{self, IndexMeta};
use crate::segment;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_common::paths;
use std::fs;
use tracing::info;

pub fn load_index() -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let index_dir = paths::index_dir()?;
    let meta_path = index_dir.join("meta.json");
    if !meta_path.exists() {
        return Err(HyperFindError::IndexError(
            "Index metadata not found.".into(),
        ));
    }

    let meta: IndexMeta = serde_json::from_str(&fs::read_to_string(&meta_path)?)
        .map_err(|e| HyperFindError::SerializationError(format!("meta parse: {}", e)))?;

    info!(
        "Loading index: version={}, expected_docs={}",
        meta.version, meta.doc_count
    );

    document::reset_id_counter(meta.next_id);

    let commit = segment::read_commit()?
        .ok_or_else(|| HyperFindError::IndexError("commit not found".into()))?;

    let segments_dir = paths::segments_dir()?;
    let mut all_docs: Vec<FileDocument> = Vec::with_capacity(meta.doc_count as usize);

    let mut all_tri = Vec::new();
    let mut all_bmp = Vec::new();

    for (i, seg_id) in commit.segments.iter().enumerate() {
        let seg_path = segments_dir.join(format!("{}.seg", seg_id));
        if !seg_path.exists() {
            tracing::warn!("Segment file missing: {}", seg_id);
            continue;
        }

        let (docs, tri, bmp) = segment::read_segment(&seg_path)?;
        all_docs.extend(docs);

        if i == 0 {
            all_tri = tri;
            all_bmp = bmp;
        } else {
            tracing::warn!(
                "Multiple segments found ({} total); current loader only uses trigram/bitmap from first segment",
                commit.segments.len()
            );
        }
    }

    info!(
        "Index loaded: {} docs from {} segments",
        all_docs.len(),
        commit.segments.len()
    );

    Ok((all_docs, all_tri, all_bmp))
}