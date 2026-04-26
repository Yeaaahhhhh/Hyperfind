// File: crates/index-engine/src/mmap_store.rs

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
    /// 打开并 mmap 一个 segment 文件。
    /// 优化：用 `MmapOptions::populate()` 提前把页表读入，避免后续随机访问触发缺页中断。
    pub fn open(path: &Path) -> Result<Self, HyperFindError> {
        let file = File::open(path)?;
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .populate()       // 预读：内核会预先把整个文件页缓存好
                .map(&file)
                .map_err(|e| {
                    HyperFindError::IndexError(format!("mmap segment {:?}: {}", path, e))
                })?
        };

        // 顺序访问 hint：让 OS 优化预读策略
        #[cfg(unix)]
        {
            let _ = mmap.advise(memmap2::Advice::Sequential);
        }

        info!("Mmap segment opened: {:?} ({} MB)", path, mmap.len() / (1024 * 1024));

        Ok(Self {
            _file: file,
            mmap,
            path: path.to_string_lossy().to_string(),
        })
    }

    pub fn data(&self) -> &[u8] { &self.mmap }
    pub fn len(&self) -> usize { self.mmap.len() }
    pub fn is_empty(&self) -> bool { self.mmap.is_empty() }
}