// File: crates/index-engine/src/index_store.rs

//! In-memory index store (优化版).

use crate::bitmap::BitmapIndex;
use crate::trigram::TrigramIndex;
use hyperfind_common::models::FileDocument;
use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHasher};
use std::hash::Hasher;
use std::sync::Arc;
use tracing::info;

#[inline]
fn path_hash(p: &str) -> u64 {
    let mut h = FxHasher::default();
    h.write(p.as_bytes());
    h.finish()
}

pub struct IndexStore {
    documents: RwLock<Vec<Arc<FileDocument>>>,
    path_index: RwLock<FxHashMap<u64, u32>>,
    id_index: RwLock<FxHashMap<u64, u32>>,
    pub trigram_index: TrigramIndex,
    pub bitmap_index: BitmapIndex,
}

impl IndexStore {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(Vec::new()),
            path_index: RwLock::new(FxHashMap::default()),
            id_index: RwLock::new(FxHashMap::default()),
            trigram_index: TrigramIndex::new(),
            bitmap_index: BitmapIndex::new(),
        }
    }

    /// 全量重建：装载文档并并行重建 trigram + bitmap。
    pub fn load(&self, docs: Vec<FileDocument>) {
        let n = docs.len();
        let t0 = std::time::Instant::now();

        rayon::scope(|s| {
            s.spawn(|_| {
                let tri_input: Vec<(u64, &str)> =
                    docs.iter().map(|d| (d.id, d.name.as_ref())).collect();
                self.trigram_index.build(&tri_input);
            });

            s.spawn(|_| {
                let bmp_input: Vec<(u64, &str, bool)> = docs
                    .iter()
                    .map(|d| (d.id, d.extension.as_ref(), d.is_dir))
                    .collect();
                self.bitmap_index.build(&bmp_input);
            });
        });

        let arc_docs: Vec<Arc<FileDocument>> = docs.into_iter().map(Arc::new).collect();

        let entries: Vec<(u64, u64, u32)> = arc_docs
            .par_iter()
            .enumerate()
            .map(|(i, d)| (path_hash(&d.path), d.id, i as u32))
            .collect();

        let mut path_idx = FxHashMap::with_capacity_and_hasher(n, Default::default());
        let mut id_idx = FxHashMap::with_capacity_and_hasher(n, Default::default());

        for (ph, id, idx) in entries {
            path_idx.insert(ph, idx);
            id_idx.insert(id, idx);
        }

        *self.documents.write() = arc_docs;
        *self.path_index.write() = path_idx;
        *self.id_index.write() = id_idx;

        info!(
            "IndexStore::load done: {} docs in {:.2}s (parallel build)",
            n,
            t0.elapsed().as_secs_f64()
        );
    }

    /// 从磁盘恢复时只装载文档和 path/id 索引，不重建 trigram / bitmap。
    pub fn load_without_rebuild(&self, docs: Vec<FileDocument>) {
        let n = docs.len();
        let t0 = std::time::Instant::now();

        let arc_docs: Vec<Arc<FileDocument>> = docs.into_iter().map(Arc::new).collect();

        let entries: Vec<(u64, u64, u32)> = arc_docs
            .par_iter()
            .enumerate()
            .map(|(i, d)| (path_hash(&d.path), d.id, i as u32))
            .collect();

        let mut path_idx = FxHashMap::with_capacity_and_hasher(n, Default::default());
        let mut id_idx = FxHashMap::with_capacity_and_hasher(n, Default::default());

        for (ph, id, idx) in entries {
            path_idx.insert(ph, idx);
            id_idx.insert(id, idx);
        }

        *self.documents.write() = arc_docs;
        *self.path_index.write() = path_idx;
        *self.id_index.write() = id_idx;

        info!(
            "IndexStore::load_without_rebuild done: {} docs in {:.2}s",
            n,
            t0.elapsed().as_secs_f64()
        );
    }

    pub fn all_documents_arc(&self) -> Vec<Arc<FileDocument>> {
        self.documents.read().clone()
    }

    pub fn all_documents(&self) -> Vec<FileDocument> {
        self.documents
            .read()
            .iter()
            .map(|d| (**d).clone())
            .collect()
    }

    pub fn get_by_id(&self, id: u64) -> Option<Arc<FileDocument>> {
        let id_idx = self.id_index.read();
        let docs = self.documents.read();
        id_idx
            .get(&id)
            .and_then(|&idx| docs.get(idx as usize).cloned())
    }

    pub fn get_by_ids(&self, ids: &[u64]) -> Vec<Arc<FileDocument>> {
        let id_idx = self.id_index.read();
        let docs = self.documents.read();
        let mut out = Vec::with_capacity(ids.len());

        for id in ids {
            if let Some(&idx) = id_idx.get(id) {
                if let Some(d) = docs.get(idx as usize) {
                    out.push(d.clone());
                }
            }
        }

        out
    }

    pub fn len(&self) -> usize {
        self.documents.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.read().is_empty()
    }

    pub fn upsert(&self, doc: FileDocument) {
        let doc_arc = Arc::new(doc);
        let mut docs = self.documents.write();
        let mut paths = self.path_index.write();
        let mut ids = self.id_index.write();

        let ph = path_hash(&doc_arc.path);
        if let Some(&idx) = paths.get(&ph) {
            let old = docs[idx as usize].clone();
            let old_lower = old.name.to_lowercase();
            self.trigram_index.remove_document(old.id, &old_lower);
            self.bitmap_index.remove_document(old.id, &old.extension);

            ids.remove(&old.id);
            ids.insert(doc_arc.id, idx);
            docs[idx as usize] = doc_arc.clone();
        } else {
            let idx = docs.len() as u32;
            paths.insert(ph, idx);
            ids.insert(doc_arc.id, idx);
            docs.push(doc_arc.clone());
        }

        let lower = doc_arc.name.to_lowercase();
        self.trigram_index.add_document(doc_arc.id, &lower);
        self.bitmap_index
            .add_document(doc_arc.id, &doc_arc.extension, doc_arc.is_dir);
    }

    pub fn remove(&self, path: &str) -> bool {
        let ph = path_hash(path);
        let mut docs = self.documents.write();
        let mut paths = self.path_index.write();
        let mut ids = self.id_index.write();

        if let Some(idx) = paths.remove(&ph) {
            let idx_usz = idx as usize;
            let removed = docs[idx_usz].clone();
            let removed_lower = removed.name.to_lowercase();

            self.trigram_index.remove_document(removed.id, &removed_lower);
            self.bitmap_index
                .remove_document(removed.id, &removed.extension);
            ids.remove(&removed.id);

            let last = docs.len() - 1;
            if idx_usz != last {
                let moved_path_hash = path_hash(&docs[last].path);
                let moved_id = docs[last].id;
                docs.swap(idx_usz, last);
                paths.insert(moved_path_hash, idx);
                ids.insert(moved_id, idx);
            }

            docs.pop();
            true
        } else {
            false
        }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.path_index.read().contains_key(&path_hash(path))
    }

    pub fn clear(&self) {
        self.documents.write().clear();
        self.path_index.write().clear();
        self.id_index.write().clear();
        self.trigram_index.clear();
        self.bitmap_index.clear();
    }

    pub fn search_with_arc<F>(&self, f: F) -> Vec<Arc<FileDocument>>
    where
        F: Fn(&FileDocument) -> bool,
    {
        self.documents
            .read()
            .iter()
            .filter(|d| f(d.as_ref()))
            .cloned()
            .collect()
    }

    pub fn search_with<F>(&self, f: F) -> Vec<FileDocument>
    where
        F: Fn(&FileDocument) -> bool,
    {
        self.documents
            .read()
            .iter()
            .filter(|d| f(d.as_ref()))
            .map(|d| (**d).clone())
            .collect()
    }
}

impl Default for IndexStore {
    fn default() -> Self {
        Self::new()
    }
}