// File: crates/core-engine/src/cache.rs

use hyperfind_common::models::SearchResult;
use parking_lot::Mutex;
use std::collections::HashMap;

pub struct QueryCache {
    entries: Mutex<HashMap<String, Vec<SearchResult>>>,
    max_entries: usize,
    order: Mutex<Vec<String>>,
}

impl QueryCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            max_entries,
            order: Mutex::new(Vec::new()),
        }
    }

    pub fn get(&self, query: &str) -> Option<Vec<SearchResult>> {
        self.entries.lock().get(query).cloned()
    }

    pub fn put(&self, query: String, results: Vec<SearchResult>) {
        let mut entries = self.entries.lock();
        let mut order = self.order.lock();
        while entries.len() >= self.max_entries && !order.is_empty() {
            let oldest = order.remove(0);
            entries.remove(&oldest);
        }
        entries.insert(query.clone(), results);
        order.push(query);
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
        self.order.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }
}

impl Default for QueryCache {
    fn default() -> Self { Self::new(128) }
}