// File: crates/index-engine/src/trigram.rs

//! Trigram-based inverted index for sub-linear file name search.
//!
//! Instead of scanning all N documents for every query, we:
//! 1. Generate trigrams from file names at index time.
//! 2. Build posting lists: trigram -> [doc_id, doc_id, ...]
//! 3. At query time, generate trigrams from the query and intersect posting lists.
//! 4. Only score the candidate documents, which is typically << N.

use hyperfind_common::utils::generate_trigrams;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use tracing::info;

/// A posting list: sorted list of document IDs that contain a particular trigram.
pub type PostingList = Vec<u64>;

/// The trigram inverted index.
pub struct TrigramIndex {
    /// Maps trigram string to a sorted list of document IDs.
    postings: RwLock<HashMap<String, PostingList>>,
}

impl TrigramIndex {
    pub fn new() -> Self {
        Self {
            postings: RwLock::new(HashMap::new()),
        }
    }

    /// Builds the trigram index from a list of (doc_id, name) pairs.
    /// Uses rayon for parallel trigram generation.
    pub fn build(&self, docs: &[(u64, &str)]) {
        // Parallel: generate trigrams for each doc
        let all_trigrams: Vec<(u64, Vec<String>)> = docs
            .par_iter()
            .map(|(id, name)| (*id, generate_trigrams(name)))
            .collect();

        // Single-threaded merge into the posting lists
        let mut postings = HashMap::new();
        for (doc_id, trigrams) in &all_trigrams {
            for tri in trigrams {
                postings
                    .entry(tri.clone())
                    .or_insert_with(Vec::new)
                    .push(*doc_id);
            }
        }

        // Sort each posting list for efficient intersection
        for list in postings.values_mut() {
            list.sort_unstable();
            list.dedup();
        }

        let count = postings.len();
        *self.postings.write() = postings;
        info!("TrigramIndex built: {} unique trigrams", count);
    }

    /// Adds a single document to the index.
    pub fn add_document(&self, doc_id: u64, name: &str) {
        let trigrams = generate_trigrams(name);
        let mut postings = self.postings.write();
        for tri in trigrams {
            let list = postings.entry(tri).or_insert_with(Vec::new);
            // Insert sorted
            if let Err(pos) = list.binary_search(&doc_id) {
                list.insert(pos, doc_id);
            }
        }
    }

    /// Removes a document from the index.
    pub fn remove_document(&self, doc_id: u64, name: &str) {
        let trigrams = generate_trigrams(name);
        let mut postings = self.postings.write();
        for tri in trigrams {
            if let Some(list) = postings.get_mut(&tri) {
                if let Ok(pos) = list.binary_search(&doc_id) {
                    list.remove(pos);
                }
            }
        }
    }

    /// Queries the trigram index for candidate document IDs.
    /// Returns the intersection of posting lists for all query trigrams.
    pub fn query(&self, keyword: &str) -> Vec<u64> {
        let trigrams = generate_trigrams(keyword);
        if trigrams.is_empty() {
            return Vec::new();
        }

        let postings = self.postings.read();

        let mut lists: Vec<&PostingList> = Vec::new();
        for tri in &trigrams {
            if let Some(list) = postings.get(tri) {
                lists.push(list);
            } else {
                // If any trigram has no posting list, intersection is empty
                return Vec::new();
            }
        }

        if lists.is_empty() {
            return Vec::new();
        }

        // Sort by smallest list first for efficient intersection
        lists.sort_by_key(|l| l.len());

        let mut result = lists[0].clone();
        for list in &lists[1..] {
            result = intersect_sorted(&result, list);
            if result.is_empty() {
                break;
            }
        }

        result
    }

    /// Returns the total number of unique trigrams.
    pub fn trigram_count(&self) -> u64 {
        self.postings.read().len() as u64
    }

    /// Clears the entire trigram index.
    pub fn clear(&self) {
        self.postings.write().clear();
    }

    /// Serializes the trigram index to bytes for segment storage.
    pub fn serialize(&self) -> Vec<u8> {
        let postings = self.postings.read();
        // Format: [count: u32] [for each: [trigram_len: u16] [trigram_bytes] [posting_count: u32] [doc_ids: u64...]]
        let mut buf = Vec::new();
        let count = postings.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());

        for (trigram, list) in postings.iter() {
            let tri_bytes = trigram.as_bytes();
            buf.extend_from_slice(&(tri_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(tri_bytes);
            buf.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for &doc_id in list {
                buf.extend_from_slice(&doc_id.to_le_bytes());
            }
        }

        buf
    }

    /// Deserializes the trigram index from bytes.
    pub fn deserialize(&self, data: &[u8]) {
        let mut pos = 0;
        if data.len() < 4 {
            return;
        }

        let count = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let mut postings = HashMap::with_capacity(count);

        for _ in 0..count {
            if pos + 2 > data.len() {
                break;
            }
            let tri_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;

            if pos + tri_len > data.len() {
                break;
            }
            let trigram = String::from_utf8_lossy(&data[pos..pos + tri_len]).to_string();
            pos += tri_len;

            if pos + 4 > data.len() {
                break;
            }
            let list_count = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            pos += 4;

            let mut list = Vec::with_capacity(list_count);
            for _ in 0..list_count {
                if pos + 8 > data.len() {
                    break;
                }
                let doc_id = u64::from_le_bytes([
                    data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
                    data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7],
                ]);
                pos += 8;
                list.push(doc_id);
            }
            postings.insert(trigram, list);
        }

        *self.postings.write() = postings;
    }
}

impl Default for TrigramIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Intersects two sorted slices efficiently.
fn intersect_sorted(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut result = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                result.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_query() {
        let idx = TrigramIndex::new();
        let docs = vec![
            (1, "main.rs"),
            (2, "main.py"),
            (3, "lib.rs"),
            (4, "maintenance.log"),
        ];
        let refs: Vec<(u64, &str)> = docs.iter().map(|(id, name)| (*id, *name)).collect();
        idx.build(&refs);

        // "main" should match docs containing "mai", "ain"
        let candidates = idx.query("main");
        assert!(candidates.contains(&1));
        assert!(candidates.contains(&2));
        assert!(candidates.contains(&4));
        assert!(!candidates.contains(&3));
    }

    #[test]
    fn test_serialize_deserialize() {
        let idx = TrigramIndex::new();
        let docs = vec![(1, "hello"), (2, "help"), (3, "world")];
        let refs: Vec<(u64, &str)> = docs.iter().map(|(id, name)| (*id, *name)).collect();
        idx.build(&refs);

        let data = idx.serialize();

        let idx2 = TrigramIndex::new();
        idx2.deserialize(&data);

        let candidates = idx2.query("hel");
        assert!(candidates.contains(&1));
        assert!(candidates.contains(&2));
    }

    #[test]
    fn test_intersect_sorted() {
        assert_eq!(intersect_sorted(&[1, 3, 5, 7], &[2, 3, 5, 8]), vec![3, 5]);
        assert_eq!(intersect_sorted(&[1, 2, 3], &[4, 5, 6]), Vec::<u64>::new());
    }
}