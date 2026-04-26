// File: crates/index-engine/src/bitmap.rs

//! Bitmap-based filter indexes for O(1) categorical filtering.
//!
//! Instead of checking each document's extension/type at search time,
//! we pre-build bitmap sets:
//!   - Extension bitmap: ext_name -> set of doc_ids
//!   - Type bitmap: "file" -> set of doc_ids, "dir" -> set of doc_ids
//!
//! At query time, filter evaluation becomes a set lookup + intersection,
//! which is O(1) per filter regardless of index size.

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use tracing::info;

/// Bitmap index for fast categorical filtering.
pub struct BitmapIndex {
    /// Extension -> set of doc IDs.
    extension_map: RwLock<HashMap<String, HashSet<u64>>>,
    /// Set of doc IDs that are files.
    file_set: RwLock<HashSet<u64>>,
    /// Set of doc IDs that are directories.
    dir_set: RwLock<HashSet<u64>>,
}

impl BitmapIndex {
    pub fn new() -> Self {
        Self {
            extension_map: RwLock::new(HashMap::new()),
            file_set: RwLock::new(HashSet::new()),
            dir_set: RwLock::new(HashSet::new()),
        }
    }

    /// Builds the bitmap index from document metadata.
    pub fn build(&self, docs: &[(u64, &str, bool)]) {
        let mut ext_map: HashMap<String, HashSet<u64>> = HashMap::new();
        let mut files = HashSet::new();
        let mut dirs = HashSet::new();

        for &(doc_id, ext, is_dir) in docs {
            if is_dir {
                dirs.insert(doc_id);
            } else {
                files.insert(doc_id);
            }
            if !ext.is_empty() {
                ext_map
                    .entry(ext.to_lowercase())
                    .or_insert_with(HashSet::new)
                    .insert(doc_id);
            }
        }

        info!(
            "BitmapIndex built: {} extensions, {} files, {} dirs",
            ext_map.len(),
            files.len(),
            dirs.len()
        );

        *self.extension_map.write() = ext_map;
        *self.file_set.write() = files;
        *self.dir_set.write() = dirs;
    }

    /// Adds a single document.
    pub fn add_document(&self, doc_id: u64, ext: &str, is_dir: bool) {
        if is_dir {
            self.dir_set.write().insert(doc_id);
        } else {
            self.file_set.write().insert(doc_id);
        }
        if !ext.is_empty() {
            self.extension_map
                .write()
                .entry(ext.to_lowercase())
                .or_insert_with(HashSet::new)
                .insert(doc_id);
        }
    }

    /// Removes a document.
    pub fn remove_document(&self, doc_id: u64, ext: &str) {
        self.file_set.write().remove(&doc_id);
        self.dir_set.write().remove(&doc_id);
        if !ext.is_empty() {
            if let Some(set) = self.extension_map.write().get_mut(&ext.to_lowercase()) {
                set.remove(&doc_id);
            }
        }
    }

    /// Returns doc IDs matching a specific extension.
    pub fn get_by_extension(&self, ext: &str) -> Option<HashSet<u64>> {
        self.extension_map.read().get(&ext.to_lowercase()).cloned()
    }

    /// Returns all file doc IDs.
    pub fn get_files(&self) -> HashSet<u64> {
        self.file_set.read().clone()
    }

    /// Returns all directory doc IDs.
    pub fn get_dirs(&self) -> HashSet<u64> {
        self.dir_set.read().clone()
    }

    /// Clears all bitmaps.
    pub fn clear(&self) {
        self.extension_map.write().clear();
        self.file_set.write().clear();
        self.dir_set.write().clear();
    }

    /// Serializes the bitmap index to bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let ext_map = self.extension_map.read();
        let files = self.file_set.read();
        let dirs = self.dir_set.read();

        let mut buf = Vec::new();

        // Extension map
        buf.extend_from_slice(&(ext_map.len() as u32).to_le_bytes());
        for (ext, ids) in ext_map.iter() {
            let ext_bytes = ext.as_bytes();
            buf.extend_from_slice(&(ext_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(ext_bytes);
            buf.extend_from_slice(&(ids.len() as u32).to_le_bytes());
            for &id in ids {
                buf.extend_from_slice(&id.to_le_bytes());
            }
        }

        // File set
        buf.extend_from_slice(&(files.len() as u32).to_le_bytes());
        for &id in files.iter() {
            buf.extend_from_slice(&id.to_le_bytes());
        }

        // Dir set
        buf.extend_from_slice(&(dirs.len() as u32).to_le_bytes());
        for &id in dirs.iter() {
            buf.extend_from_slice(&id.to_le_bytes());
        }

        buf
    }

    /// Deserializes the bitmap index from bytes.
    pub fn deserialize(&self, data: &[u8]) {
        let mut pos = 0;

        // Extension map
        if pos + 4 > data.len() { return; }
        let ext_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;

        let mut ext_map = HashMap::with_capacity(ext_count);
        for _ in 0..ext_count {
            if pos + 2 > data.len() { break; }
            let ext_len = u16::from_le_bytes([data[pos], data[pos+1]]) as usize;
            pos += 2;
            if pos + ext_len > data.len() { break; }
            let ext = String::from_utf8_lossy(&data[pos..pos+ext_len]).to_string();
            pos += ext_len;

            if pos + 4 > data.len() { break; }
            let id_count = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
            pos += 4;

            let mut ids = HashSet::with_capacity(id_count);
            for _ in 0..id_count {
                if pos + 8 > data.len() { break; }
                let id = u64::from_le_bytes([
                    data[pos], data[pos+1], data[pos+2], data[pos+3],
                    data[pos+4], data[pos+5], data[pos+6], data[pos+7],
                ]);
                pos += 8;
                ids.insert(id);
            }
            ext_map.insert(ext, ids);
        }

        // File set
        let files = deserialize_id_set(data, &mut pos);
        // Dir set
        let dirs = deserialize_id_set(data, &mut pos);

        *self.extension_map.write() = ext_map;
        *self.file_set.write() = files;
        *self.dir_set.write() = dirs;
    }
}

fn deserialize_id_set(data: &[u8], pos: &mut usize) -> HashSet<u64> {
    if *pos + 4 > data.len() { return HashSet::new(); }
    let count = u32::from_le_bytes([data[*pos], data[*pos+1], data[*pos+2], data[*pos+3]]) as usize;
    *pos += 4;
    let mut set = HashSet::with_capacity(count);
    for _ in 0..count {
        if *pos + 8 > data.len() { break; }
        let id = u64::from_le_bytes([
            data[*pos], data[*pos+1], data[*pos+2], data[*pos+3],
            data[*pos+4], data[*pos+5], data[*pos+6], data[*pos+7],
        ]);
        *pos += 8;
        set.insert(id);
    }
    set
}

impl Default for BitmapIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_build_and_query() {
        let bm = BitmapIndex::new();
        let docs = vec![
            (1, "rs", false),
            (2, "rs", false),
            (3, "txt", false),
            (4, "", true),
        ];
        bm.build(&docs);

        let rs_set = bm.get_by_extension("rs").unwrap();
        assert!(rs_set.contains(&1));
        assert!(rs_set.contains(&2));
        assert!(!rs_set.contains(&3));

        let files = bm.get_files();
        assert_eq!(files.len(), 3);

        let dirs = bm.get_dirs();
        assert_eq!(dirs.len(), 1);
        assert!(dirs.contains(&4));
    }

    #[test]
    fn test_bitmap_serialize_roundtrip() {
        let bm = BitmapIndex::new();
        let docs = vec![(1, "rs", false), (2, "py", false), (3, "", true)];
        bm.build(&docs);

        let data = bm.serialize();

        let bm2 = BitmapIndex::new();
        bm2.deserialize(&data);

        assert!(bm2.get_by_extension("rs").unwrap().contains(&1));
        assert!(bm2.get_dirs().contains(&3));
    }
}