// File: crates/common/src/models.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a single indexed file or directory entry.
/// This is stored in binary segment format with trigram postings and bitmap indexes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDocument {
    pub id: u64,
    pub name: String,
    pub name_lower: String,
    pub path: String,
    pub parent: String,
    pub extension: String,
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub is_dir: bool,
    /// Optional content hash for deduplication.
    pub content_hash: Option<String>,
}

/// Fixed-size binary representation for mmap storage.
/// Used in segment files for zero-copy access.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct FileDocumentCompact {
    pub id: u64,
    pub size: u64,
    pub modified_ts: i64,
    pub is_dir: u8,
    pub name_offset: u64,
    pub name_len: u32,
    pub path_offset: u64,
    pub path_len: u32,
    pub ext_id: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub raw: String,
    pub keywords: Vec<String>,
    pub filters: SearchFilters,
    pub limit: Option<usize>,
    pub sort_by: SortField,
    pub sort_order: SortOrder,
    /// Whether to search file contents as well.
    pub search_content: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub extension: Option<String>,
    pub path_contains: Option<String>,
    pub size_min: Option<u64>,
    pub size_max: Option<u64>,
    pub modified_after: Option<DateTime<Utc>>,
    pub modified_before: Option<DateTime<Utc>>,
    pub entry_type: Option<EntryType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortField {
    Relevance,
    Name,
    Path,
    Size,
    Modified,
}

impl Default for SortField {
    fn default() -> Self {
        SortField::Relevance
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Descending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub document: FileDocument,
    pub score: f64,
    /// Content snippet if content search matched.
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_documents: u64,
    pub total_files: u64,
    pub total_directories: u64,
    pub total_size_bytes: u64,
    pub indexed_roots: Vec<String>,
    pub last_scan: Option<DateTime<Utc>>,
    pub last_update: Option<DateTime<Utc>>,
    pub trigram_count: u64,
    pub segment_count: u32,
    pub index_size_bytes: u64,
}

impl Default for IndexStats {
    fn default() -> Self {
        Self {
            total_documents: 0,
            total_files: 0,
            total_directories: 0,
            total_size_bytes: 0,
            indexed_roots: Vec::new(),
            last_scan: None,
            last_update: None,
            trigram_count: 0,
            segment_count: 0,
            index_size_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDirectory {
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub directories: Vec<IndexedDirectory>,
    pub excluded_patterns: Vec<String>,
    pub default_result_limit: usize,
    pub auto_watch: bool,
    pub auto_rebuild: bool,
    /// Whether to index file contents for text files.
    pub index_content: bool,
    /// Max file size (bytes) for content indexing.
    pub content_max_size: u64,
    /// File extensions eligible for content indexing.
    pub content_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            directories: Vec::new(),
            excluded_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "__pycache__".to_string(),
                ".DS_Store".to_string(),
                "$Recycle.Bin".to_string(),
                "System Volume Information".to_string(),
            ],
            default_result_limit: 500,
            auto_watch: true,
            auto_rebuild: true,
            index_content: true,
            content_max_size: 10 * 1024 * 1024, // 10 MB
            content_extensions: vec![
                "txt".into(), "md".into(), "rs".into(), "py".into(),
                "js".into(), "ts".into(), "jsx".into(), "tsx".into(),
                "java".into(), "c".into(), "cpp".into(), "h".into(),
                "hpp".into(), "go".into(), "rb".into(), "php".into(),
                "html".into(), "css".into(), "xml".into(), "json".into(),
                "toml".into(), "yaml".into(), "yml".into(), "ini".into(),
                "cfg".into(), "conf".into(), "sh".into(), "bat".into(),
                "ps1".into(), "sql".into(), "log".into(), "csv".into(),
            ],
        }
    }
}

/// Represents a volume/drive discovered on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub label: Option<String>,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub fs_type: Option<String>,
}