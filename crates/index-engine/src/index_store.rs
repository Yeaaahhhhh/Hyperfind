// File: crates/index-engine/src/index_store.rs

//! In-memory index store backed by trigram and bitmap indexes.
//!
//! This is the primary data structure used at search time.
//! Documents are stored in a Vec for random access by index.
//! A HashMap provides O(1) path-based dedup.
//! Trigram index enables sub-linear keyword search.
//! Bitmap index enables O(1) categorical filtering.

use crate::bitmap::BitmapIndex;
use crate::trigram::TrigramIndex;
use hyperfind_common::models::FileDocument;
use parking_lot::RwLock;
use std::collections::HashMap;
use tracing::info;

pub struct IndexStore {
    documents: RwLock<Vec<FileDocument>>,
    path_index: RwLock<HashMap<String, usize>>,
    id_index: RwLock<HashMap<u64, usize>>,
    pub trigram_index: TrigramIndex,
    pub bitmap_index: BitmapIndex,
}

impl IndexStore {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(Vec::new()),
            path_index: RwLock::new(HashMap::new()),
            id_index: RwLock::new(HashMap::new()),
            trigram_index: TrigramIndex::new(),
            bitmap_index: BitmapIndex::new(),
        }
    }

    /// Replaces the entire index with the given documents and rebuilds all indexes.
    pub fn load(&self, docs: Vec<FileDocument>) {
        // Build auxiliary indexes
        let tri_input: Vec<(u64, &str)> = docs.iter().map(|d| (d.id, d.name_lower.as_str())).collect();
        self.trigram_index.build(&tri_input);

        let bmp_input: Vec<(u64, &str, bool)> = docs
            .iter()
            .map(|d| (d.id, d.extension.as_str(), d.is_dir))
            .collect();
        self.bitmap_index.build(&bmp_input);

        // Build lookup maps
        let mut path_idx = HashMap::with_capacity(docs.len());
        let mut id_idx = HashMap::with_capacity(docs.len());
        for (i, doc) in docs.iter().enumerate() {
            path_idx.insert(doc.path.clone(), i);
            id_idx.insert(doc.id, i);
        }

        let count = docs.len();
        *self.documents.write() = docs;
        *self.path_index.write() = path_idx;
        *self.id_index.write() = id_idx;

        info!("IndexStore loaded {} documents with trigram + bitmap indexes", count);
    }

    pub fn all_documents(&self) -> Vec<FileDocument> {
        self.documents.read().clone()
    }

    pub fn get_by_id(&self, id: u64) -> Option<FileDocument> {
        let id_idx = self.id_index.read();
        let docs = self.documents.read();
        id_idx.get(&id).and_then(|&idx| docs.get(idx).cloned())
    }

    pub fn get_by_ids(&self, ids: &[u64]) -> Vec<FileDocument> {
        let id_idx = self.id_index.read();
        let docs = self.documents.read();
        ids.iter()
            .filter_map(|id| id_idx.get(id).and_then(|&idx| docs.get(idx).cloned()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.documents.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.read().is_empty()
    }

    pub fn upsert(&self, doc: FileDocument) {
        let mut docs = self.documents.write();
        let mut paths = self.path_index.write();
        let mut ids = self.id_index.write();

        if let Some(&idx) = paths.get(&doc.path) {
            // Remove old from trigram + bitmap
            let old = &docs[idx];
            self.trigram_index.remove_document(old.id, &old.name_lower);
            self.bitmap_index.remove_document(old.id, &old.extension);

            // Update
            ids.remove(&docs[idx].id);
            ids.insert(doc.id, idx);
            docs[idx] = doc.clone();
        } else {
            let idx = docs.len();
            paths.insert(doc.path.clone(), idx);
            ids.insert(doc.id, idx);
            docs.push(doc.clone());
        }

        // Add to trigram + bitmap
        self.trigram_index.add_document(doc.id, &doc.name_lower);
        self.bitmap_index.add_document(doc.id, &doc.extension, doc.is_dir);
    }

    pub fn remove(&self, path: &str) -> bool {
        let mut docs = self.documents.write();
        let mut paths = self.path_index.write();
        let mut ids = self.id_index.write();

        if let Some(idx) = paths.remove(path) {
            let removed = &docs[idx];
            self.trigram_index.remove_document(removed.id, &removed.name_lower);
            self.bitmap_index.remove_document(removed.id, &removed.extension);
            ids.remove(&removed.id);

            let last_idx = docs.len() - 1;
            if idx != last_idx {
                let moved_path = docs[last_idx].path.clone();
                let moved_id = docs[last_idx].id;
                docs.swap(idx, last_idx);
                paths.insert(moved_path, idx);
                ids.insert(moved_id, idx);
            }
            docs.pop();
            true
        } else {
            false
        }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.path_index.read().contains_key(path)
    }

    pub fn clear(&self) {
        self.documents.write().clear();
        self.path_index.write().clear();
        self.id_index.write().clear();
        self.trigram_index.clear();
        self.bitmap_index.clear();
    }

    /// Linear scan search (fallback for very short queries).
    pub fn search_with<F>(&self, f: F) -> Vec<FileDocument>
    where
        F: Fn(&FileDocument) -> bool,
    {
        self.documents.read().iter().filter(|d| f(d)).cloned().collect()
    }
}

impl Default for IndexStore {
    fn default() -> Self {
        Self::new()
    }
}