// File: crates/index-engine/src/trigram.rs

//! Trigram inverted index (优化版).
//!
//! 关键改动：
//! - 提供 `query_bitmap()`，搜索阶段直接返回 `RoaringTreemap`，避免 `Vec<u64> -> RoaringTreemap` 二次构造
//! - `build()` 改为分片并行局部聚合再 merge，避免 `Vec<(u64, Vec<u32>)>` 这种超大中间结构
//! - 预分配与合并路径优化，降低 build 峰值内存
//! - 保留旧 `query()` / `query_into()` 兼容 API，但内部复用位图查询

use hyperfind_common::utils::trigram_codes;
use parking_lot::RwLock;
use rayon::prelude::*;
use roaring::RoaringTreemap;
use rustc_hash::FxHashMap;
use tracing::info;

pub struct TrigramIndex {
    postings: RwLock<FxHashMap<u32, RoaringTreemap>>,
}

impl TrigramIndex {
    pub fn new() -> Self {
        Self {
            postings: RwLock::new(FxHashMap::default()),
        }
    }

    /// 分片并行构建：
    /// - 每个分片在线程内局部聚合 trigram -> roaring
    /// - 最后 merge 到全局 map
    /// 这样比先构造 `Vec<(u64, Vec<u32>)>` 更省内存。
    pub fn build(&self, docs: &[(u64, &str)]) {
        if docs.is_empty() {
            self.postings.write().clear();
            return;
        }

        let threads = rayon::current_num_threads().max(1);
        let chunk_size = (docs.len() / threads).max(8_192);

        let partials: Vec<FxHashMap<u32, RoaringTreemap>> = docs
            .par_chunks(chunk_size)
            .map(|chunk| {
                let mut local: FxHashMap<u32, RoaringTreemap> = FxHashMap::default();

                for &(doc_id, name) in chunk {
                    let codes = trigram_codes(name);
                    for code in codes {
                        local
                            .entry(code)
                            .or_insert_with(RoaringTreemap::new)
                            .insert(doc_id);
                    }
                }

                local
            })
            .collect();

        let mut merged: FxHashMap<u32, RoaringTreemap> = FxHashMap::default();
        let estimated = partials
            .iter()
            .map(|m| m.len())
            .max()
            .unwrap_or(0)
            .saturating_mul(2);
        merged.reserve(estimated);

        for part in partials {
            for (code, rt) in part {
                match merged.get_mut(&code) {
                    Some(existing) => {
                        *existing |= rt;
                    }
                    None => {
                        merged.insert(code, rt);
                    }
                }
            }
        }

        let count = merged.len();
        *self.postings.write() = merged;
        info!("TrigramIndex built: {} unique trigrams (roaring)", count);
    }

    pub fn add_document(&self, doc_id: u64, name: &str) {
        let codes = trigram_codes(name);
        let mut postings = self.postings.write();
        for code in codes {
            postings
                .entry(code)
                .or_insert_with(RoaringTreemap::new)
                .insert(doc_id);
        }
    }

    pub fn remove_document(&self, doc_id: u64, name: &str) {
        let codes = trigram_codes(name);
        let mut postings = self.postings.write();
        for code in codes {
            if let Some(rt) = postings.get_mut(&code) {
                rt.remove(doc_id);
                if rt.is_empty() {
                    postings.remove(&code);
                }
            }
        }
    }

    /// 直接返回位图结果，供搜索链路直接做位图交集。
    pub fn query_bitmap(&self, keyword: &str) -> Option<RoaringTreemap> {
        let codes = trigram_codes(keyword);
        if codes.is_empty() {
            return None;
        }

        let postings = self.postings.read();

        let mut lists: Vec<&RoaringTreemap> = Vec::with_capacity(codes.len());
        for code in &codes {
            match postings.get(code) {
                Some(list) => lists.push(list),
                None => return Some(RoaringTreemap::new()),
            }
        }

        lists.sort_by_key(|l| l.len());

        let mut acc = lists[0].clone();
        for list in &lists[1..] {
            acc &= *list;
            if acc.is_empty() {
                break;
            }
        }

        Some(acc)
    }

    /// 兼容旧 API。
    pub fn query(&self, keyword: &str) -> Vec<u64> {
        match self.query_bitmap(keyword) {
            Some(rt) => rt.into_iter().collect(),
            None => Vec::new(),
        }
    }

    /// 兼容旧 API，避免调用方二次分配。
    pub fn query_into(&self, keyword: &str, out: &mut Vec<u64>) {
        out.clear();
        if let Some(rt) = self.query_bitmap(keyword) {
            out.reserve(rt.len() as usize);
            for v in rt {
                out.push(v);
            }
        }
    }

    pub fn trigram_count(&self) -> u64 {
        self.postings.read().len() as u64
    }

    pub fn clear(&self) {
        self.postings.write().clear();
    }

    /// 二进制序列化：
    /// [count: u32]
    /// 每条: [trigram_code: u32] [roaring_len: u32] [roaring bytes...]
    pub fn serialize(&self) -> Vec<u8> {
        let postings = self.postings.read();
        let mut buf = Vec::with_capacity(postings.len() * 32);
        buf.extend_from_slice(&(postings.len() as u32).to_le_bytes());

        let mut tmp = Vec::with_capacity(4096);
        for (code, rt) in postings.iter() {
            tmp.clear();
            rt.serialize_into(&mut tmp).expect("roaring serialize");
            buf.extend_from_slice(&code.to_le_bytes());
            buf.extend_from_slice(&(tmp.len() as u32).to_le_bytes());
            buf.extend_from_slice(&tmp);
        }

        buf
    }

    pub fn deserialize(&self, data: &[u8]) {
        if data.len() < 4 {
            self.postings.write().clear();
            return;
        }

        let mut pos = 0usize;
        let count = u32::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
        ]) as usize;
        pos += 4;

        let mut postings: FxHashMap<u32, RoaringTreemap> =
            FxHashMap::with_capacity_and_hasher(count, Default::default());

        for _ in 0..count {
            if pos + 8 > data.len() {
                break;
            }

            let code = u32::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]);
            pos += 4;

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
                postings.insert(code, rt);
            }
        }

        *self.postings.write() = postings;
    }
}

impl Default for TrigramIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_query() {
        let idx = TrigramIndex::new();
        let docs = vec![
            (1u64, "main.rs"),
            (2, "main.py"),
            (3, "lib.rs"),
            (4, "maintenance.log"),
        ];
        let refs: Vec<(u64, &str)> = docs.iter().map(|(id, n)| (*id, *n)).collect();
        idx.build(&refs);

        let r = idx.query("main");
        assert!(r.contains(&1));
        assert!(r.contains(&2));
        assert!(r.contains(&4));
        assert!(!r.contains(&3));
    }

    #[test]
    fn test_query_bitmap() {
        let idx = TrigramIndex::new();
        let refs: Vec<(u64, &str)> = vec![(1, "hello"), (2, "help"), (3, "world")];
        idx.build(&refs);

        let r = idx.query_bitmap("hel").unwrap();
        assert!(r.contains(1));
        assert!(r.contains(2));
        assert!(!r.contains(3));
    }

    #[test]
    fn test_serialize_roundtrip() {
        let idx = TrigramIndex::new();
        let refs: Vec<(u64, &str)> = vec![(1, "hello"), (2, "help"), (3, "world")];
        idx.build(&refs);

        let data = idx.serialize();

        let idx2 = TrigramIndex::new();
        idx2.deserialize(&data);

        let r = idx2.query("hel");
        assert!(r.contains(&1));
        assert!(r.contains(&2));
    }
}