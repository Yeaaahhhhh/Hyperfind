// File: crates/core-engine/src/parser/filters.rs

use hyperfind_common::models::{EntryType, FileDocument, SearchFilters};
use hyperfind_index_engine::bitmap::BitmapIndex;
use std::collections::HashSet;

/// Compiles filters into a candidate set using bitmap indexes where possible.
/// Returns None if no bitmap filtering can be applied (fall back to linear).
pub fn compile_bitmap_filter(
    filters: &SearchFilters,
    bitmap: &BitmapIndex,
) -> Option<HashSet<u64>> {
    let mut sets: Vec<HashSet<u64>> = Vec::new();

    // Extension filter
    if let Some(ref ext) = filters.extension {
        if let Some(ext_set) = bitmap.get_by_extension(ext) {
            sets.push(ext_set);
        } else {
            // Extension not found = no results
            return Some(HashSet::new());
        }
    }

    // Entry type filter
    if let Some(ref et) = filters.entry_type {
        match et {
            EntryType::File => sets.push(bitmap.get_files()),
            EntryType::Directory => sets.push(bitmap.get_dirs()),
        }
    }

    if sets.is_empty() {
        return None; // No bitmap filters applicable
    }

    // Intersect all sets
    let mut result = sets.remove(0);
    for set in sets {
        result = result.intersection(&set).copied().collect();
    }

    Some(result)
}

/// Compiles remaining (non-bitmap) filters into a closure.
pub fn compile_post_filter(filters: &SearchFilters) -> Box<dyn Fn(&FileDocument) -> bool + Send + Sync> {
    let path_contains = filters.path_contains.clone();
    let size_min = filters.size_min;
    let size_max = filters.size_max;
    let modified_after = filters.modified_after;
    let modified_before = filters.modified_before;

    Box::new(move |doc: &FileDocument| -> bool {
        if let Some(ref p) = path_contains {
            if !doc.path.to_lowercase().contains(&p.to_lowercase()) {
                return false;
            }
        }
        if let Some(min) = size_min {
            if doc.size < min { return false; }
        }
        if let Some(max) = size_max {
            if doc.size > max { return false; }
        }
        if let Some(after) = modified_after {
            if doc.modified < after { return false; }
        }
        if let Some(before) = modified_before {
            if doc.modified > before { return false; }
        }
        true
    })
}