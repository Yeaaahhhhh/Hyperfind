// File: crates/core-engine/src/cache.rs

//! 查询缓存（优化版）。
//!
//! 改动：
//! - 单 Mutex，减少双锁开销
//! - 严格 LRU 风格更新顺序
//! - 避免 `order` 中出现重复 query

use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;

struct CacheState {
    entries: FxHashMap<String, Vec<u64>>,
    order: VecDeque<String>,
}

pub struct QueryCache {
    state: Mutex<CacheState>,
    max_entries: usize,
}

impl QueryCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            state: Mutex::new(CacheState {
                entries: FxHashMap::default(),
                order: VecDeque::new(),
            }),
            max_entries,
        }
    }

    pub fn get(&self, query: &str) -> Option<Vec<u64>> {
        let mut state = self.state.lock();
        let ids = state.entries.get(query).cloned()?;

        if let Some(pos) = state.order.iter().position(|q| q == query) {
            state.order.remove(pos);
        }
        state.order.push_back(query.to_string());

        Some(ids)
    }

    pub fn put(&self, query: String, ids: Vec<u64>) {
        let mut state = self.state.lock();

        if state.entries.contains_key(&query) {
            state.entries.insert(query.clone(), ids);
            if let Some(pos) = state.order.iter().position(|q| q == &query) {
                state.order.remove(pos);
            }
            state.order.push_back(query);
            return;
        }

        while state.entries.len() >= self.max_entries {
            if let Some(oldest) = state.order.pop_front() {
                state.entries.remove(&oldest);
            } else {
                break;
            }
        }

        state.entries.insert(query.clone(), ids);
        state.order.push_back(query);
    }

    pub fn clear(&self) {
        let mut state = self.state.lock();
        state.entries.clear();
        state.order.clear();
    }

    pub fn len(&self) -> usize {
        self.state.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.lock().entries.is_empty()
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new(128)
    }
}