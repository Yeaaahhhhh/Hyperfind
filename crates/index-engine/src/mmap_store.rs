// File: crates/index-engine/src/mmap_store.rs

//! Memory-mapped segment reader for zero-copy index loading.
//!
//! Instead of reading the entire index into heap memory, we mmap the segment files.
//! The OS page cache handles caching; only accessed pages are loaded into RAM.
//! This dramatically reduces startup time and memory usage for large indexes.

use hyperfind_common::errors::HyperFindError;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use tracing::info;

/// A memory-mapped view of a segment file.
pub struct MmapSegment {
    _file: File,
    mmap: Mmap,
    pub path: String,
}

impl MmapSegment {
    /// Opens and memory-maps a segment file.
    pub fn open(path: &Path) -> Result<Self, HyperFindError> {
        let file = File::open(path)?;
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| {
                HyperFindError::IndexError(format!("Failed to mmap segment {:?}: {}", path, e))
            })?
        };

        info!("Mmap segment opened: {:?} ({} bytes)", path, mmap.len());

        Ok(Self {
            _file: file,
            mmap,
            path: path.to_string_lossy().to_string(),
        })
    }

    /// Returns a reference to the memory-mapped data.
    pub fn data(&self) -> &[u8] {
        &self.mmap
    }

    /// Returns the size of the mapped region.
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Returns true if the mapped region is empty.
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }
}