// File: crates/core-engine/src/search/query.rs

use hyperfind_common::models::SearchQuery;

pub fn normalize_query(mut query: SearchQuery) -> SearchQuery {
    query.keywords = query.keywords
        .into_iter()
        .map(|k| k.trim().to_lowercase())
        .filter(|k| !k.is_empty())
        .collect();
    if query.limit.is_none() {
        query.limit = Some(500);
    }
    query
}