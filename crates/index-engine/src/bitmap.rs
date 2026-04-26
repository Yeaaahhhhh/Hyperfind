// File: crates/index-engine/src/bitmap.rs

//! Bitmap-based filter indexes (Roaring-backed).
//!
//! 保持现有接口，但修正一些细节：
//! - `remove_document()` 需要同时支持 file / dir 两侧删除
//! - build / serialize / deserialize 路径保持紧凑
//! - 继续保留 Arc 读优化

use parking_lot::RwLock;
use roaring::RoaringTreemap;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tracing::info;

pub struct BitmapIndex {
    extension_map: RwLock<FxHashMap<String, Arc<RoaringTreemap>>>,
    file_set: RwLock<Arc<RoaringTreemap>>,
    dir_set: RwLock<Arc<RoaringTreemap>>,
}

impl BitmapIndex {
    pub fn new() -> Self {
        Self {
            extension_map: RwLock::new(FxHashMap::default()),
            file_set: RwLock::new(Arc::new(RoaringTreemap::new())),
            dir_set: RwLock::new(Arc::new(RoaringTreemap::new())),
        }
    }

    pub fn build(&self, docs: &[(u64, &str, bool)]) {
        let mut ext_map: FxHashMap<String, RoaringTreemap> = FxHashMap::default();
        let mut files = RoaringTreemap::new();
        let mut dirs = RoaringTreemap::new();

        for &(id, ext, is_dir) in docs {
            if is_dir {
                dirs.insert(id);
            } else {
                files.insert(id);
            }

            if !ext.is_empty() {
                ext_map
                    .entry(ext.to_ascii_lowercase())
                    .or_insert_with(RoaringTreemap::new)
                    .insert(id);
            }
        }

        info!(
            "BitmapIndex built: {} extensions, {} files, {} dirs",
            ext_map.len(),
            files.len(),
            dirs.len()
        );

        let arc_map: FxHashMap<String, Arc<RoaringTreemap>> =
            ext_map.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();

        *self.extension_map.write() = arc_map;
        *self.file_set.write() = Arc::new(files);
        *self.dir_set.write() = Arc::new(dirs);
    }

    pub fn add_document(&self, doc_id: u64, ext: &str, is_dir: bool) {
        if is_dir {
            let mut g = self.dir_set.write();
            let mut new = (**g).clone();
            new.insert(doc_id);
            *g = Arc::new(new);
        } else {
            let mut g = self.file_set.write();
            let mut new = (**g).clone();
            new.insert(doc_id);
            *g = Arc::new(new);
        }

        if !ext.is_empty() {
            let key = ext.to_ascii_lowercase();
            let mut map = self.extension_map.write();

            let new_rt = match map.get(&key) {
                Some(rt) => {
                    let mut n = (**rt).clone();
                    n.insert(doc_id);
                    n
                }
                None => {
                    let mut n = RoaringTreemap::new();
                    n.insert(doc_id);
                    n
                }
            };

            map.insert(key, Arc::new(new_rt));
        }
    }

    pub fn remove_document(&self, doc_id: u64, ext: &str) {
        {
            let mut g = self.file_set.write();
            if g.contains(doc_id) {
                let mut n = (**g).clone();
                n.remove(doc_id);
                *g = Arc::new(n);
            }
        }

        {
            let mut g = self.dir_set.write();
            if g.contains(doc_id) {
                let mut n = (**g).clone();
                n.remove(doc_id);
                *g = Arc::new(n);
            }
        }

        if !ext.is_empty() {
            let key = ext.to_ascii_lowercase();
            let mut map = self.extension_map.write();

            if let Some(rt) = map.get(&key) {
                let mut n = (**rt).clone();
                n.remove(doc_id);
                if n.is_empty() {
                    map.remove(&key);
                } else {
                    map.insert(key, Arc::new(n));
                }
            }
        }
    }

    pub fn get_by_extension_arc(&self, ext: &str) -> Option<Arc<RoaringTreemap>> {
        self.extension_map
            .read()
            .get(&ext.to_ascii_lowercase())
            .cloned()
    }

    pub fn get_files_arc(&self) -> Arc<RoaringTreemap> {
        self.file_set.read().clone()
    }

    pub fn get_dirs_arc(&self) -> Arc<RoaringTreemap> {
        self.dir_set.read().clone()
    }

    pub fn get_by_extension(&self, ext: &str) -> Option<std::collections::HashSet<u64>> {
        self.get_by_extension_arc(ext).map(|rt| rt.iter().collect())
    }

    pub fn get_files(&self) -> std::collections::HashSet<u64> {
        self.get_files_arc().iter().collect()
    }

    pub fn get_dirs(&self) -> std::collections::HashSet<u64> {
        self.get_dirs_arc().iter().collect()
    }

    pub fn clear(&self) {
        self.extension_map.write().clear();
        *self.file_set.write() = Arc::new(RoaringTreemap::new());
        *self.dir_set.write() = Arc::new(RoaringTreemap::new());
    }

    pub fn serialize(&self) -> Vec<u8> {
        let ext_map = self.extension_map.read();
        let files = self.file_set.read();
        let dirs = self.dir_set.read();

        let mut buf = Vec::new();
        buf.extend_from_slice(&(ext_map.len() as u32).to_le_bytes());

        let mut tmp = Vec::with_capacity(4096);

        for (ext, rt) in ext_map.iter() {
            let eb = ext.as_bytes();
            buf.extend_from_slice(&(eb.len() as u16).to_le_bytes());
            buf.extend_from_slice(eb);

            tmp.clear();
            rt.serialize_into(&mut tmp).expect("roaring serialize");
            buf.extend_from_slice(&(tmp.len() as u32).to_le_bytes());
            buf.extend_from_slice(&tmp);
        }

        tmp.clear();
        files.serialize_into(&mut tmp).expect("roaring serialize");
        buf.extend_from_slice(&(tmp.len() as u32).to_le_bytes());
        buf.extend_from_slice(&tmp);

        tmp.clear();
        dirs.serialize_into(&mut tmp).expect("roaring serialize");
        buf.extend_from_slice(&(tmp.len() as u32).to_le_bytes());
        buf.extend_from_slice(&tmp);

        buf
    }

    pub fn deserialize(&self, data: &[u8]) {
        let mut pos = 0usize;
        if pos + 4 > data.len() {
            self.clear();
            return;
        }

        let ext_count = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;

        let mut ext_map: FxHashMap<String, Arc<RoaringTreemap>> =
            FxHashMap::with_capacity_and_hasher(ext_count, Default::default());

        for _ in 0..ext_count {
            if pos + 2 > data.len() {
                break;
            }

            let elen = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
            pos += 2;

            if pos + elen > data.len() {
                break;
            }

            let ext = String::from_utf8_lossy(&data[pos..pos + elen]).to_string();
            pos += elen;

            if pos + 4 > data.len() {
                break;
            }

            let rlen = u32::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]) as usize;
            pos += 4;

            if pos + rlen > data.len() {
                break;
            }

            let mut slice = &data[pos..pos + rlen];
            pos += rlen;

            if let Ok(rt) = RoaringTreemap::deserialize_from(&mut slice) {
                ext_map.insert(ext, Arc::new(rt));
            }
        }

        if pos + 4 > data.len() {
            *self.extension_map.write() = ext_map;
            *self.file_set.write() = Arc::new(RoaringTreemap::new());
            *self.dir_set.write() = Arc::new(RoaringTreemap::new());
            return;
        }

        let flen = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;

        if pos + flen > data.len() {
            *self.extension_map.write() = ext_map;
            *self.file_set.write() = Arc::new(RoaringTreemap::new());
            *self.dir_set.write() = Arc::new(RoaringTreemap::new());
            return;
        }

        let mut fs = &data[pos..pos + flen];
        pos += flen;
        let files = RoaringTreemap::deserialize_from(&mut fs).unwrap_or_default();

        if pos + 4 > data.len() {
            *self.extension_map.write() = ext_map;
            *self.file_set.write() = Arc::new(files);
            *self.dir_set.write() = Arc::new(RoaringTreemap::new());
            return;
        }

        let dlen = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;

        if pos + dlen > data.len() {
            *self.extension_map.write() = ext_map;
            *self.file_set.write() = Arc::new(files);
            *self.dir_set.write() = Arc::new(RoaringTreemap::new());
            return;
        }

        let mut ds = &data[pos..pos + dlen];
        let dirs = RoaringTreemap::deserialize_from(&mut ds).unwrap_or_default();

        *self.extension_map.write() = ext_map;
        *self.file_set.write() = Arc::new(files);
        *self.dir_set.write() = Arc::new(dirs);
    }
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
    fn test_build_and_query() {
        let bm = BitmapIndex::new();
        let docs = vec![(1u64, "rs", false), (2, "rs", false), (3, "txt", false), (4, "", true)];
        bm.build(&docs);

        let rs = bm.get_by_extension_arc("rs").unwrap();
        assert!(rs.contains(1));
        assert!(rs.contains(2));
        assert!(!rs.contains(3));

        assert_eq!(bm.get_files_arc().len(), 3);
        assert!(bm.get_dirs_arc().contains(4));
    }

    #[test]
    fn test_serialize_roundtrip() {
        let bm = BitmapIndex::new();
        bm.build(&[(1u64, "rs", false), (2, "py", false), (3, "", true)]);

        let data = bm.serialize();

        let bm2 = BitmapIndex::new();
        bm2.deserialize(&data);

        assert!(bm2.get_by_extension_arc("rs").unwrap().contains(1));
        assert!(bm2.get_dirs_arc().contains(3));
    }
}