// File: crates/collector/src/scanner.rs

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_index_engine::document;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info, warn};
use walkdir::{DirEntry, WalkDir};

const WALKDIR_BUILD_CHUNK: usize = 16_384;

pub fn scan_directory(
    root: &str,
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    #[cfg(target_os = "windows")]
    {
        if let Some(letter) = try_extract_drive_letter(root) {
            info!("Attempting MFT scan for drive {}: ...", letter);
            match hyperfind_platform_windows::mft::scan_volume_mft(letter, excluded_patterns) {
                Ok(docs) => {
                    info!("MFT scan succeeded: {} entries", docs.len());
                    return Ok(docs);
                }
                Err(e) => {
                    warn!(
                        "MFT scan failed for {}: — falling back to walkdir: {}",
                        letter, e
                    );
                }
            }
        }
    }

    scan_directory_walkdir(root, excluded_patterns)
}

pub fn scan_directories(
    roots: &[String],
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    let results: Vec<Vec<FileDocument>> = roots
        .par_iter()
        .filter_map(|r| match scan_directory(r, excluded_patterns) {
            Ok(d) => Some(d),
            Err(e) => {
                warn!("Failed to scan {}: {}", r, e);
                None
            }
        })
        .collect();

    let total: usize = results.iter().map(|v| v.len()).sum();
    let mut all = Vec::with_capacity(total);
    for v in results {
        all.extend(v);
    }
    Ok(all)
}

pub fn scan_all_drives(excluded_patterns: &[String]) -> Result<Vec<FileDocument>, HyperFindError> {
    #[cfg(target_os = "windows")]
    {
        info!("Using MFT for all-drive scan on Windows");
        match hyperfind_platform_windows::mft::scan_all_volumes_mft(excluded_patterns) {
            Ok(docs) => return Ok(docs),
            Err(e) => warn!("MFT all-volume scan failed: {} — falling back to walkdir", e),
        }
    }

    let volumes = crate::volumes::discover_volumes();
    let roots: Vec<String> = volumes.iter().map(|v| v.mount_point.clone()).collect();
    scan_directories(&roots, excluded_patterns)
}

fn scan_directory_walkdir(
    root: &str,
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    let root_path = Path::new(root);
    if !root_path.exists() {
        return Err(HyperFindError::ScanError(format!(
            "Directory does not exist: {}",
            root
        )));
    }
    if !root_path.is_dir() {
        return Err(HyperFindError::ScanError(format!(
            "Path is not a directory: {}",
            root
        )));
    }

    info!("Walkdir scan: {}", root);

    let patterns_lower: Arc<Vec<String>> = Arc::new(
        excluded_patterns
            .iter()
            .map(|p| p.to_ascii_lowercase())
            .collect(),
    );
    let errors = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);

    let mut documents: Vec<FileDocument> = Vec::new();
    let mut batch: Vec<DirEntry> = Vec::with_capacity(WALKDIR_BUILD_CHUNK);

    let mut flush_batch = |batch: &mut Vec<DirEntry>, documents: &mut Vec<FileDocument>| {
        if batch.is_empty() {
            return;
        }

        let built: Vec<FileDocument> = batch
            .par_iter()
            .filter_map(|entry| match build_document(entry.path()) {
                Ok(d) => Some(d),
                Err(e) => {
                    debug!("build_document error {:?}: {}", entry.path(), e);
                    errors.fetch_add(1, Ordering::Relaxed);
                    None
                }
            })
            .collect();

        documents.extend(built);
        batch.clear();
    };

    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        match entry {
            Ok(entry) => {
                if should_exclude(entry.path(), &patterns_lower) {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                batch.push(entry);
                if batch.len() >= WALKDIR_BUILD_CHUNK {
                    flush_batch(&mut batch, &mut documents);
                }
            }
            Err(e) => {
                debug!("Walk error: {}", e);
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    flush_batch(&mut batch, &mut documents);

    info!(
        "Walkdir scan complete: {} docs, {} skipped, {} errors",
        documents.len(),
        skipped.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed)
    );

    Ok(documents)
}

fn build_document(path: &Path) -> Result<FileDocument, HyperFindError> {
    let metadata = path
        .metadata()
        .map_err(|e| HyperFindError::ScanError(format!("metadata {:?}: {}", path, e)))?;

    let full_path = hyperfind_common::utils::normalize_path(path);
    let name = hyperfind_common::utils::extract_file_name(path);
    let extension = hyperfind_common::utils::extract_extension(path);
    let is_dir = metadata.is_dir();
    let size = if is_dir { 0 } else { metadata.len() };
    let modified = metadata
        .modified()
        .map(hyperfind_common::utils::system_time_to_utc)
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(FileDocument {
        id: document::next_id(),
        name: Arc::from(name.as_str()),
        path: Arc::from(full_path.as_str()),
        extension: Arc::from(extension.as_str()),
        size,
        modified,
        is_dir,
        content_hash: None,
    })
}

fn should_exclude(path: &Path, patterns_lower: &[String]) -> bool {
    if patterns_lower.is_empty() {
        return false;
    }

    for component in path.components() {
        let comp = component.as_os_str().to_string_lossy();
        if comp.is_ascii() {
            for p in patterns_lower {
                if p.len() == comp.len() && comp.eq_ignore_ascii_case(p) {
                    return true;
                }
            }
        } else {
            let lc = comp.to_lowercase();
            if patterns_lower.iter().any(|p| *p == lc) {
                return true;
            }
        }
    }

    false
}

#[cfg(target_os = "windows")]
fn try_extract_drive_letter(path: &str) -> Option<char> {
    let trimmed = path.trim_end_matches(|c| c == '\\' || c == '/');
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        Some(bytes[0].to_ascii_uppercase() as char)
    } else {
        None
    }
}