// File: crates/core-engine/src/ranking.rs

use chrono::Utc;
use hyperfind_common::models::{SearchResult, SortField, SortOrder};
use hyperfind_common::utils::ascii_cmp_ignore_case;

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
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortField::Name => {
            results.sort_by(|a, b| {
                let an = a.document.name.as_ref();
                let bn = b.document.name.as_ref();

                if an.is_ascii() && bn.is_ascii() {
                    ascii_cmp_ignore_case(an, bn)
                } else {
                    an.to_lowercase().cmp(&bn.to_lowercase())
                }
            });
        }
        SortField::Path => {
            results.sort_by(|a, b| a.document.path.as_ref().cmp(b.document.path.as_ref()));
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