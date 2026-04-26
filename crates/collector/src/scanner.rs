// File: crates/collector/src/scanner.rs

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;
use hyperfind_index_engine::document;
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// Scans a single directory using the best available method.
/// On Windows + NTFS, uses MFT if the path is a drive root.
/// Otherwise falls back to walkdir.
pub fn scan_directory(
    root: &str,
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    // On Windows, check if this is a drive root like "C:\" and try MFT
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
                    warn!("MFT scan failed for {}: — falling back to walkdir: {}", letter, e);
                }
            }
        }
    }

    // Fallback: walkdir-based scan (cross-platform)
    scan_directory_walkdir(root, excluded_patterns)
}

/// Scans multiple directories.
pub fn scan_directories(
    roots: &[String],
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    let mut all_docs = Vec::new();
    for root in roots {
        match scan_directory(root, excluded_patterns) {
            Ok(docs) => all_docs.extend(docs),
            Err(e) => warn!("Failed to scan {}: {}", root, e),
        }
    }
    Ok(all_docs)
}

/// Scans all drives on the system using the fastest available method.
pub fn scan_all_drives(
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    #[cfg(target_os = "windows")]
    {
        info!("Using MFT for all-drive scan on Windows");
        match hyperfind_platform_windows::mft::scan_all_volumes_mft(excluded_patterns) {
            Ok(docs) => return Ok(docs),
            Err(e) => {
                warn!("MFT all-volume scan failed: {} — falling back to walkdir", e);
            }
        }
    }

    // Fallback: discover volumes and scan with walkdir
    let volumes = crate::volumes::discover_volumes();
    let roots: Vec<String> = volumes.iter().map(|v| v.mount_point.clone()).collect();
    scan_directories(&roots, excluded_patterns)
}

/// Walkdir-based scanning (cross-platform fallback).
fn scan_directory_walkdir(
    root: &str,
    excluded_patterns: &[String],
) -> Result<Vec<FileDocument>, HyperFindError> {
    let root_path = Path::new(root);
    if !root_path.exists() {
        return Err(HyperFindError::ScanError(format!(
            "Directory does not exist: {}", root
        )));
    }
    if !root_path.is_dir() {
        return Err(HyperFindError::ScanError(format!(
            "Path is not a directory: {}", root
        )));
    }

    info!("Walkdir scan: {}", root);
    let errors = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);

    let entries: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => {
                if should_exclude(entry.path(), excluded_patterns) {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    None
                } else {
                    Some(entry)
                }
            }
            Err(e) => {
                debug!("Walk error: {}", e);
                errors.fetch_add(1, Ordering::Relaxed);
                None
            }
        })
        .collect();

    let documents: Vec<FileDocument> = entries
        .par_iter()
        .filter_map(|entry| {
            match build_document(entry.path()) {
                Ok(doc) => Some(doc),
                Err(e) => {
                    debug!("Failed to build document for {:?}: {}", entry.path(), e);
                    errors.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
        })
        .collect();

    info!(
        "Walkdir scan complete: {} docs, {} skipped, {} errors",
        documents.len(),
        skipped.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed),
    );
    Ok(documents)
}

fn build_document(path: &Path) -> Result<FileDocument, HyperFindError> {
    let metadata = path.metadata().map_err(|e| {
        HyperFindError::ScanError(format!("Metadata error {:?}: {}", path, e))
    })?;

    let full_path = hyperfind_common::utils::normalize_path(path);
    let name = hyperfind_common::utils::extract_file_name(path);
    let parent = hyperfind_common::utils::extract_parent(path);
    let extension = hyperfind_common::utils::extract_extension(path);
    let is_dir = metadata.is_dir();
    let size = if is_dir { 0 } else { metadata.len() };
    let modified = metadata
        .modified()
        .map(hyperfind_common::utils::system_time_to_utc)
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(FileDocument {
        id: document::next_id(),
        name_lower: hyperfind_common::utils::normalize_string(&name),
        name,
        path: full_path,
        parent,
        extension,
        size,
        modified,
        is_dir,
        content_hash: None,
    })
}

fn should_exclude(path: &Path, patterns: &[String]) -> bool {
    for component in path.components() {
        let comp_str = component.as_os_str().to_string_lossy();
        for pattern in patterns {
            if comp_str.eq_ignore_ascii_case(pattern) {
                return true;
            }
        }
    }
    false
}

/// Tries to extract a drive letter from a Windows path like "C:\", "D:\\"
#[cfg(target_os = "windows")]
fn try_extract_drive_letter(path: &str) -> Option<char> {
    let trimmed = path.trim_end_matches(|c| c == '\\' || c == '/');
    let bytes = trimmed.as_bytes();
    // Match patterns: "C:", "C:\", "C:\\"
    if bytes.len() >= 2
        && bytes[1] == b':'
        && bytes[0].is_ascii_alphabetic()
    {
        Some(bytes[0].to_ascii_uppercase() as char)
    } else {
        None
    }
}