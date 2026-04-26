// File: crates/core-engine/src/service.rs

use crate::cache::QueryCache;
use crate::content::extractor;
use crate::parser::dsl;
use crate::parser::filters;
use crate::ranking;
use crate::search::matcher;
use crate::search::query;
use hyperfind_common::config;
use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::{
    AppConfig, IndexStats, IndexedDirectory, SearchResult, VolumeInfo,
};
use hyperfind_collector::scanner;
use hyperfind_collector::volumes;
use hyperfind_index_engine::index_store::IndexStore;
use hyperfind_index_engine::{loader, writer};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

pub struct SearchService {
    index_store: Arc<IndexStore>,
    cache: Arc<QueryCache>,
    config: RwLock<AppConfig>,
}

impl SearchService {
    pub fn new(config: AppConfig) -> Self {
        Self {
            index_store: Arc::new(IndexStore::new()),
            cache: Arc::new(QueryCache::new(128)),
            config: RwLock::new(config),
        }
    }

    pub fn search(&self, raw_query: &str) -> Result<Vec<SearchResult>, HyperFindError> {
        if let Some(cached) = self.cache.get(raw_query) {
            return Ok(cached);
        }

        let parsed = dsl::parse_query(raw_query)?;
        let normalized = query::normalize_query(parsed);
        let limit = normalized.limit.unwrap_or(500);
        let keywords = &normalized.keywords;
        let config = self.config.read().clone();

        // Step 1: Bitmap filter (ext/type)
        let bitmap_candidates = filters::compile_bitmap_filter(
            &normalized.filters,
            &self.index_store.bitmap_index,
        );

        // Step 2: Trigram candidates (keywords)
        let trigram_candidates: Option<HashSet<u64>> = if !keywords.is_empty() {
            let mut combined: Option<HashSet<u64>> = None;
            for keyword in keywords {
                let ids: HashSet<u64> = self.index_store.trigram_index
                    .query(keyword).into_iter().collect();
                combined = Some(match combined {
                    Some(existing) => existing.intersection(&ids).copied().collect(),
                    None => ids,
                });
            }
            combined
        } else {
            None
        };

        // Step 3: Intersect
        let candidate_ids: Option<HashSet<u64>> = match (bitmap_candidates, trigram_candidates) {
            (Some(bm), Some(tri)) => Some(bm.intersection(&tri).copied().collect()),
            (Some(bm), None) => Some(bm),
            (None, Some(tri)) => Some(tri),
            (None, None) => None,
        };

        // Step 4: Post-filter
        let post_filter = filters::compile_post_filter(&normalized.filters);

        // Step 5: Get candidates and score
        let candidates = if let Some(ref ids) = candidate_ids {
            let id_vec: Vec<u64> = ids.iter().copied().collect();
            self.index_store.get_by_ids(&id_vec)
        } else {
            self.index_store.all_documents()
        };

        let mut results: Vec<SearchResult> = candidates
            .into_iter()
            .filter(|doc| post_filter(doc))
            .filter_map(|doc| {
                let match_result = matcher::match_document(&doc, keywords);
                if match_result.matched || keywords.is_empty() {
                    let snippet = if normalized.search_content && !keywords.is_empty() {
                        search_content(&doc.path, keywords, &config)
                    } else {
                        None
                    };
                    Some(SearchResult {
                        score: match_result.score,
                        document: doc,
                        snippet,
                    })
                } else {
                    None
                }
            })
            .collect();

        ranking::rank_results(&mut results, &normalized.sort_by, &normalized.sort_order);
        results.truncate(limit);

        self.cache.put(raw_query.to_string(), results.clone());
        info!("Search '{}' returned {} results", raw_query, results.len());
        Ok(results)
    }

    /// One-click full system index: uses MFT on Windows, walkdir on other platforms.
    pub fn index_all_volumes(&self) -> Result<IndexStats, HyperFindError> {
        let config = self.config.read().clone();

        info!("Starting full system index...");
        let start = std::time::Instant::now();

        // Use the scanner's smart all-drive scan (MFT on Windows, walkdir fallback)
        let documents = scanner::scan_all_drives(&config.excluded_patterns)?;

        info!("Scan phase: {} documents in {:.2}s",
            documents.len(), start.elapsed().as_secs_f64());

        // Load into store (builds trigram + bitmap indexes)
        let idx_start = std::time::Instant::now();
        self.index_store.load(documents.clone());
        info!("Index build phase: {:.2}s", idx_start.elapsed().as_secs_f64());

        // Persist to disk
        let write_start = std::time::Instant::now();
        writer::write_index(
            &documents,
            &self.index_store.trigram_index,
            &self.index_store.bitmap_index,
        )?;
        info!("Disk write phase: {:.2}s", write_start.elapsed().as_secs_f64());

        self.cache.clear();

        // Update config with discovered volumes
        let vols = volumes::discover_volumes();
        let mut cfg = self.config.write();
        cfg.directories = vols.iter()
            .map(|v| IndexedDirectory { path: v.mount_point.clone(), enabled: true })
            .collect();
        let cfg_clone = cfg.clone();
        drop(cfg);
        let _ = config::save_config(&cfg_clone);

        let total_time = start.elapsed().as_secs_f64();
        info!("Full system index complete: {} documents in {:.2}s", documents.len(), total_time);

        Ok(self.compute_stats())
    }

    pub fn rebuild_index(&self) -> Result<IndexStats, HyperFindError> {
        let config = self.config.read().clone();
        let enabled_dirs: Vec<String> = config.directories.iter()
            .filter(|d| d.enabled)
            .map(|d| d.path.clone())
            .collect();

        if enabled_dirs.is_empty() {
            warn!("No directories configured for indexing");
            return Ok(self.compute_stats());
        }

        let documents = scanner::scan_directories(&enabled_dirs, &config.excluded_patterns)?;
        self.index_store.load(documents.clone());
        writer::write_index(
            &documents,
            &self.index_store.trigram_index,
            &self.index_store.bitmap_index,
        )?;
        self.cache.clear();
        Ok(self.compute_stats())
    }

    pub fn scan_directory(&self, path: &str) -> Result<usize, HyperFindError> {
        let config = self.config.read().clone();
        let documents = scanner::scan_directory(path, &config.excluded_patterns)?;
        let count = documents.len();
        for doc in documents {
            self.index_store.upsert(doc);
        }
        let all = self.index_store.all_documents();
        writer::write_index(
            &all,
            &self.index_store.trigram_index,
            &self.index_store.bitmap_index,
        )?;
        self.cache.clear();
        Ok(count)
    }

    pub fn load_index(&self) -> Result<IndexStats, HyperFindError> {
        if !writer::index_exists()? {
            return Ok(self.compute_stats());
        }
        let (documents, trigram_data, bitmap_data) = loader::load_index()?;
        self.index_store.load(documents);
        if !trigram_data.is_empty() {
            self.index_store.trigram_index.deserialize(&trigram_data);
        }
        if !bitmap_data.is_empty() {
            self.index_store.bitmap_index.deserialize(&bitmap_data);
        }
        self.cache.clear();
        Ok(self.compute_stats())
    }

    pub fn save_index(&self) -> Result<(), HyperFindError> {
        let documents = self.index_store.all_documents();
        writer::write_index(
            &documents,
            &self.index_store.trigram_index,
            &self.index_store.bitmap_index,
        )?;
        Ok(())
    }

    pub fn get_stats(&self) -> IndexStats { self.compute_stats() }

    pub fn get_config(&self) -> AppConfig { self.config.read().clone() }

    pub fn update_config(&self, new_config: AppConfig) -> Result<(), HyperFindError> {
        config::save_config(&new_config)?;
        *self.config.write() = new_config;
        Ok(())
    }

    pub fn add_directory(&self, path: &str) -> Result<(), HyperFindError> {
        config::validate_directory(path)?;
        let mut cfg = self.config.write();
        if cfg.directories.iter().any(|d| d.path == path) {
            return Err(HyperFindError::ConfigError(format!("Already configured: {}", path)));
        }
        cfg.directories.push(IndexedDirectory { path: path.to_string(), enabled: true });
        let cfg_clone = cfg.clone();
        drop(cfg);
        config::save_config(&cfg_clone)?;
        Ok(())
    }

    pub fn remove_directory(&self, path: &str) -> Result<(), HyperFindError> {
        let mut cfg = self.config.write();
        cfg.directories.retain(|d| d.path != path);
        let cfg_clone = cfg.clone();
        drop(cfg);
        config::save_config(&cfg_clone)?;
        Ok(())
    }

    pub fn discover_volumes(&self) -> Vec<VolumeInfo> { volumes::discover_volumes() }

    pub fn index_store(&self) -> &Arc<IndexStore> { &self.index_store }

    fn compute_stats(&self) -> IndexStats {
        let docs = self.index_store.all_documents();
        let config = self.config.read();
        let total_files = docs.iter().filter(|d| !d.is_dir).count() as u64;
        let total_dirs = docs.iter().filter(|d| d.is_dir).count() as u64;
        let total_size: u64 = docs.iter().map(|d| d.size).sum();
        let index_size = writer::index_size_bytes().unwrap_or(0);

        IndexStats {
            total_documents: docs.len() as u64,
            total_files,
            total_directories: total_dirs,
            total_size_bytes: total_size,
            indexed_roots: config.directories.iter().map(|d| d.path.clone()).collect(),
            last_scan: None,
            last_update: None,
            trigram_count: self.index_store.trigram_index.trigram_count(),
            segment_count: 1,
            index_size_bytes: index_size,
        }
    }
}

fn search_content(path: &str, keywords: &[String], config: &AppConfig) -> Option<String> {
    if !config.index_content { return None; }
    let p = Path::new(path);
    let content = extractor::extract_content(p, config.content_max_size, &config.content_extensions)?;
    for kw in keywords {
        if let Some(snippet) = extractor::generate_snippet(&content, kw, 60) {
            return Some(snippet);
        }
    }
    None
}