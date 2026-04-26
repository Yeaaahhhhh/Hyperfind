// File: apps/desktop/src-tauri/src/ipc/dto.rs

use hyperfind_common::models::{
    AppConfig, FileDocument, IndexStats, IndexedDirectory, SearchResult,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResultDto {
    pub document: FileDocumentDto,
    pub score: f64,
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileDocumentDto {
    pub id: u64,
    pub name: String,
    pub name_lower: String,
    pub path: String,
    pub parent: String,
    pub extension: String,
    pub size: u64,
    pub modified: String,
    pub is_dir: bool,
}

impl From<SearchResult> for SearchResultDto {
    fn from(r: SearchResult) -> Self {
        Self {
            document: FileDocumentDto::from(r.document),
            score: r.score,
            snippet: r.snippet,
        }
    }
}

impl From<FileDocument> for FileDocumentDto {
    fn from(d: FileDocument) -> Self {
        Self {
            id: d.id,
            name: d.name,
            name_lower: d.name_lower,
            path: d.path,
            parent: d.parent,
            extension: d.extension,
            size: d.size,
            modified: d.modified.to_rfc3339(),
            is_dir: d.is_dir,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexStatsDto {
    pub total_documents: u64,
    pub total_files: u64,
    pub total_directories: u64,
    pub total_size_bytes: u64,
    pub indexed_roots: Vec<String>,
    pub last_scan: Option<String>,
    pub last_update: Option<String>,
    pub trigram_count: u64,
    pub segment_count: u32,
    pub index_size_bytes: u64,
}

impl From<IndexStats> for IndexStatsDto {
    fn from(s: IndexStats) -> Self {
        Self {
            total_documents: s.total_documents,
            total_files: s.total_files,
            total_directories: s.total_directories,
            total_size_bytes: s.total_size_bytes,
            indexed_roots: s.indexed_roots,
            last_scan: s.last_scan.map(|d| d.to_rfc3339()),
            last_update: s.last_update.map(|d| d.to_rfc3339()),
            trigram_count: s.trigram_count,
            segment_count: s.segment_count,
            index_size_bytes: s.index_size_bytes,
        }
    }
}

/// Progress event emitted during indexing.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexProgressEvent {
    pub phase: String,
    pub message: String,
    pub progress_pct: Option<f64>,
    pub done: bool,
    pub error: Option<String>,
    pub stats: Option<IndexStatsDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfigDto {
    pub directories: Vec<IndexedDirectoryDto>,
    pub excluded_patterns: Vec<String>,
    pub default_result_limit: usize,
    pub auto_watch: bool,
    pub auto_rebuild: bool,
    pub index_content: bool,
    pub content_max_size: u64,
    pub content_extensions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexedDirectoryDto {
    pub path: String,
    pub enabled: bool,
}

impl From<AppConfig> for AppConfigDto {
    fn from(c: AppConfig) -> Self {
        Self {
            directories: c.directories.into_iter().map(|d| d.into()).collect(),
            excluded_patterns: c.excluded_patterns,
            default_result_limit: c.default_result_limit,
            auto_watch: c.auto_watch,
            auto_rebuild: c.auto_rebuild,
            index_content: c.index_content,
            content_max_size: c.content_max_size,
            content_extensions: c.content_extensions,
        }
    }
}

impl From<AppConfigDto> for AppConfig {
    fn from(c: AppConfigDto) -> Self {
        Self {
            directories: c.directories.into_iter().map(|d| d.into()).collect(),
            excluded_patterns: c.excluded_patterns,
            default_result_limit: c.default_result_limit,
            auto_watch: c.auto_watch,
            auto_rebuild: c.auto_rebuild,
            index_content: c.index_content,
            content_max_size: c.content_max_size,
            content_extensions: c.content_extensions,
        }
    }
}

impl From<IndexedDirectory> for IndexedDirectoryDto {
    fn from(d: IndexedDirectory) -> Self {
        Self { path: d.path, enabled: d.enabled }
    }
}

impl From<IndexedDirectoryDto> for IndexedDirectory {
    fn from(d: IndexedDirectoryDto) -> Self {
        Self { path: d.path, enabled: d.enabled }
    }
}