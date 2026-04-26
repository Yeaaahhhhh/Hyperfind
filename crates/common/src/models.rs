// File: crates/common/src/models.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Represents a single indexed file or directory entry.
///
/// 优化点：
/// - 删除冗余 `name_lower`（按需小写化，节省每条 ~40B + 堆分配）
/// - 删除冗余 `parent`（可由 `path` 派生）
/// - `name` / `path` / `extension` 改为 `Arc<str>`，跨多处共享避免 clone 时复制堆数据
/// - `content_hash` 改为 `Option<Arc<str>>`，命中率低时几乎零成本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDocument {
    pub id: u64,
    pub name: Arc<str>,
    pub path: Arc<str>,
    pub extension: Arc<str>,
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub is_dir: bool,
    pub content_hash: Option<Arc<str>>,
}

impl FileDocument {
    /// 兼容旧 API：按需返回 lower-case name。
    /// 大多数路径下，name 本身就是 ASCII，可走 fast-path。
    #[inline]
    pub fn name_lower(&self) -> String {
        self.name.to_lowercase()
    }

    /// 从 path 派生 parent，避免占用额外内存。
    #[inline]
    pub fn parent(&self) -> &str {
        let p: &str = &self.path;
        let sep_pos = p.rfind(|c| c == '\\' || c == '/');
        match sep_pos {
            Some(i) => &p[..i],
            None => "",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub raw: String,
    pub keywords: Vec<String>,
    pub filters: SearchFilters,
    pub limit: Option<usize>,
    pub sort_by: SortField,
    pub sort_order: SortOrder,
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
    fn default() -> Self { SortField::Relevance }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self { SortOrder::Descending }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub document: FileDocument,
    pub score: f64,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub index_content: bool,
    pub content_max_size: u64,
    pub content_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            directories: Vec::new(),
            excluded_patterns: vec![
                ".git".into(),
                "node_modules".into(),
                "__pycache__".into(),
                ".DS_Store".into(),
                "$Recycle.Bin".into(),
                "System Volume Information".into(),
            ],
            default_result_limit: 500,
            auto_watch: true,
            auto_rebuild: true,
            index_content: true,
            content_max_size: 10 * 1024 * 1024,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub label: Option<String>,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub fs_type: Option<String>,
}