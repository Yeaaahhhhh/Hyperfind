// File: crates/core-engine/src/ranking.rs

use hyperfind_common::models::{SearchResult, SortField, SortOrder};
use chrono::Utc;

pub fn rank_results(
    results: &mut Vec<SearchResult>,
    sort_by: &SortField,
    sort_order: &SortOrder,
) {
    if *sort_by == SortField::Relevance {
        let now = Utc::now();
        for result in results.iter_mut() {
            let age_days = (now - result.document.modified).num_days();
            if age_days <= 7 {
                result.score *= 1.10;
            } else if age_days <= 30 {
                result.score *= 1.05;
            }
        }
    }

    match sort_by {
        SortField::Relevance => {
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        }
        SortField::Name => {
            results.sort_by(|a, b| a.document.name_lower.cmp(&b.document.name_lower));
        }
        SortField::Path => {
            results.sort_by(|a, b| a.document.path.cmp(&b.document.path));
        }
        SortField::Size => {
            results.sort_by(|a, b| a.document.size.cmp(&b.document.size));
        }
        SortField::Modified => {
            results.sort_by(|a, b| a.document.modified.cmp(&b.document.modified));
        }
    }

    if *sort_order == SortOrder::Descending && *sort_by != SortField::Relevance {
        results.reverse();
    }
}