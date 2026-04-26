// File: crates/index-engine/src/segment.rs

//! Segment 文件格式 v2（高性能版）。
//!
//! Layout:
//!   [magic: 4]
//!   [version: 4]
//!   [doc_count: 8]
//!   [doc_data_len: 8] [doc_data: bincode]
//!   [trigram_len: 8]  [trigram_data]
//!   [bitmap_len: 8]   [bitmap_data]
//!   [crc32: 4]
//!
//! 改进：
//! - 文档使用 `bincode` 而非 `serde_json`：体积↓，编/解码速度↑
//! - 用 `mmap2` 读 segment，避免一次性 `fs::read`
//! - v2 增加更严格的边界检查，避免切片 panic

use crate::bitmap::BitmapIndex;
use crate::trigram::TrigramIndex;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_common::paths;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use tracing::info;

const SEGMENT_MAGIC: &[u8; 4] = b"HFSG";
const SEGMENT_VERSION: u32 = 2;

pub fn write_segment(
    segment_id: &str,
    documents: &[FileDocument],
    trigram_index: &TrigramIndex,
    bitmap_index: &BitmapIndex,
) -> Result<std::path::PathBuf, HyperFindError> {
    let segments_dir = paths::segments_dir()?;
    fs::create_dir_all(&segments_dir)?;
    let path = segments_dir.join(format!("{}.seg", segment_id));

    let doc_data = bincode::serialize(documents)
        .map_err(|e| HyperFindError::IndexError(format!("bincode docs: {}", e)))?;
    let tri_data = trigram_index.serialize();
    let bmp_data = bitmap_index.serialize();

    let mut crc = crc32fast::Hasher::new();
    crc.update(&doc_data);
    crc.update(&tri_data);
    crc.update(&bmp_data);
    let checksum = crc.finalize();

    let mut f = fs::File::create(&path)?;
    f.write_all(SEGMENT_MAGIC)?;
    f.write_all(&SEGMENT_VERSION.to_le_bytes())?;
    f.write_all(&(documents.len() as u64).to_le_bytes())?;
    f.write_all(&(doc_data.len() as u64).to_le_bytes())?;
    f.write_all(&doc_data)?;
    f.write_all(&(tri_data.len() as u64).to_le_bytes())?;
    f.write_all(&tri_data)?;
    f.write_all(&(bmp_data.len() as u64).to_le_bytes())?;
    f.write_all(&bmp_data)?;
    f.write_all(&checksum.to_le_bytes())?;
    f.flush()?;

    info!(
        "Segment v2 written: {} ({} docs, doc={}KB tri={}KB bmp={}KB)",
        segment_id,
        documents.len(),
        doc_data.len() / 1024,
        tri_data.len() / 1024,
        bmp_data.len() / 1024
    );

    Ok(path)
}

pub fn read_segment(
    path: &std::path::Path,
) -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let file = fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data: &[u8] = &mmap;

    if data.len() < 16 {
        return Err(HyperFindError::IndexError("segment too small".into()));
    }
    if &data[0..4] != SEGMENT_MAGIC {
        return Err(HyperFindError::IndexError("invalid magic".into()));
    }

    let version = read_u32_at(data, 4)?;

    if version == 2 {
        return read_segment_v2(data);
    }
    if version == 1 {
        return read_segment_v1_compat(data);
    }

    Err(HyperFindError::IndexError(format!(
        "unknown segment version {}",
        version
    )))
}

fn read_segment_v2(data: &[u8]) -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let mut pos = 8usize;

    let _doc_count = read_u64(data, &mut pos)?;
    let doc_len = read_u64(data, &mut pos)? as usize;
    let doc_data = read_slice(data, &mut pos, doc_len)?;

    let tri_len = read_u64(data, &mut pos)? as usize;
    let tri_data = read_slice(data, &mut pos, tri_len)?;

    let bmp_len = read_u64(data, &mut pos)? as usize;
    let bmp_data = read_slice(data, &mut pos, bmp_len)?;

    let stored_crc = read_u32(data, &mut pos)?;

    if pos != data.len() {
        return Err(HyperFindError::IndexError(format!(
            "segment trailing bytes detected: total={} parsed={}",
            data.len(),
            pos
        )));
    }

    let mut crc = crc32fast::Hasher::new();
    crc.update(doc_data);
    crc.update(tri_data);
    crc.update(bmp_data);
    let computed = crc.finalize();

    if computed != stored_crc {
        return Err(HyperFindError::IndexError(format!(
            "checksum mismatch: stored={} computed={}",
            stored_crc, computed
        )));
    }

    let docs: Vec<FileDocument> = bincode::deserialize(doc_data)
        .map_err(|e| HyperFindError::IndexError(format!("bincode docs decode: {}", e)))?;

    Ok((docs, tri_data.to_vec(), bmp_data.to_vec()))
}

fn read_segment_v1_compat(
    data: &[u8],
) -> Result<(Vec<FileDocument>, Vec<u8>, Vec<u8>), HyperFindError> {
    let file_len = data.len();
    let scan_start = if file_len > 8192 { file_len - 8192 } else { 8 };

    for cand in scan_start..file_len.saturating_sub(4) {
        let footer_len = u32::from_le_bytes([
            data[cand],
            data[cand + 1],
            data[cand + 2],
            data[cand + 3],
        ]) as usize;

        if footer_len > 0 && footer_len < 4096 && cand + 4 + footer_len == file_len {
            let footer_bytes = &data[cand + 4..];

            #[derive(Deserialize)]
            struct OldFooter {
                doc_store_offset: u64,
                doc_store_len: u64,
                trigram_offset: u64,
                trigram_len: u64,
                bitmap_offset: u64,
                bitmap_len: u64,
                doc_count: u64,
                checksum: u32,
            }

            if let Ok(footer) = serde_json::from_slice::<OldFooter>(footer_bytes) {
                let _ = footer.doc_store_offset;
                let _ = footer.trigram_offset;
                let _ = footer.bitmap_offset;
                let _ = footer.doc_count;
                let _ = footer.checksum;

                let base = 8usize;
                let doc_end = base + footer.doc_store_len as usize;
                let tri_end = doc_end + footer.trigram_len as usize;
                let bmp_end = tri_end + footer.bitmap_len as usize;

                if bmp_end > data.len() || base > doc_end || doc_end > tri_end || tri_end > bmp_end
                {
                    return Err(HyperFindError::IndexError(
                        "v1 segment ranges out of bounds".into(),
                    ));
                }

                let doc_slice = &data[base..doc_end];
                let tri_slice = &data[doc_end..tri_end];
                let bmp_slice = &data[tri_end..bmp_end];

                let docs: Vec<FileDocument> = serde_json::from_slice(doc_slice)
                    .map_err(|e| HyperFindError::IndexError(format!("v1 docs: {}", e)))?;

                return Ok((docs, tri_slice.to_vec(), bmp_slice.to_vec()));
            }
        }
    }

    Err(HyperFindError::IndexError(
        "v1 segment footer not found".into(),
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPoint {
    pub generation: u64,
    pub segments: Vec<String>,
    pub timestamp: String,
}

pub fn write_commit(commit: &CommitPoint) -> Result<(), HyperFindError> {
    let index_dir = paths::index_dir()?;
    let path = index_dir.join("commit.json");
    let data = serde_json::to_string_pretty(commit)
        .map_err(|e| HyperFindError::IndexError(format!("commit serialize: {}", e)))?;
    fs::write(&path, data)?;
    Ok(())
}

pub fn read_commit() -> Result<Option<CommitPoint>, HyperFindError> {
    let index_dir = paths::index_dir()?;
    let path = index_dir.join("commit.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)?;
    let commit: CommitPoint = serde_json::from_str(&data)
        .map_err(|e| HyperFindError::IndexError(format!("commit parse: {}", e)))?;
    Ok(Some(commit))
}

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

fn read_u32_at(data: &[u8], offset: usize) -> Result<u32, HyperFindError> {
    if offset + 4 > data.len() {
        return Err(HyperFindError::IndexError(format!(
            "unexpected eof while reading u32 at {}",
            offset
        )));
    }

    Ok(u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn read_u32(data: &[u8], pos: &mut usize) -> Result<u32, HyperFindError> {
    let v = read_u32_at(data, *pos)?;
    *pos += 4;
    Ok(v)
}

fn read_u64(data: &[u8], pos: &mut usize) -> Result<u64, HyperFindError> {
    if *pos + 8 > data.len() {
        return Err(HyperFindError::IndexError(format!(
            "unexpected eof while reading u64 at {}",
            *pos
        )));
    }

    let v = u64::from_le_bytes([
        data[*pos],
        data[*pos + 1],
        data[*pos + 2],
        data[*pos + 3],
        data[*pos + 4],
        data[*pos + 5],
        data[*pos + 6],
        data[*pos + 7],
    ]);
    *pos += 8;
    Ok(v)
}

fn read_slice<'a>(
    data: &'a [u8],
    pos: &mut usize,
    len: usize,
) -> Result<&'a [u8], HyperFindError> {
    if *pos + len > data.len() {
        return Err(HyperFindError::IndexError(format!(
            "unexpected eof while reading slice at {} len {}",
            *pos, len
        )));
    }

    let s = &data[*pos..*pos + len];
    *pos += len;
    Ok(s)
}