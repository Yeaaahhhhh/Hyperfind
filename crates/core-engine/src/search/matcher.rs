// File: crates/core-engine/src/search/matcher.rs

use hyperfind_common::models::FileDocument;
use hyperfind_common::utils::ascii_contains_ignore_case;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matched: bool,
    pub score: f64,
}

pub fn match_document(doc: &FileDocument, keywords: &[String]) -> MatchResult {
    if keywords.is_empty() {
        return MatchResult {
            matched: true,
            score: 1.0,
        };
    }

    let mut total_score = 0.0;

    for keyword in keywords {
        let score = score_keyword(doc, keyword);
        if score <= 0.0 {
            return MatchResult {
                matched: false,
                score: 0.0,
            };
        }
        total_score += score;
    }

    MatchResult {
        matched: true,
        score: total_score / keywords.len() as f64,
    }
}

fn score_keyword(doc: &FileDocument, keyword: &str) -> f64 {
    let name = doc.name.as_ref();
    let path = doc.path.as_ref();

    if name.is_ascii() && keyword.is_ascii() {
        return score_keyword_ascii(name, path, keyword);
    }

    let name_lower = name.to_lowercase();
    let path_lower = path.to_lowercase();

    if name_lower == keyword {
        return 100.0;
    }

    let stem = name_lower
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(name_lower.as_str());

    if stem == keyword {
        return 90.0;
    }

    if name_lower.starts_with(keyword) {
        return 75.0;
    }
    if name_lower.contains(keyword) {
        return 50.0;
    }
    if path_lower.contains(keyword) {
        return 25.0;
    }

    0.0
}

#[inline]
fn score_keyword_ascii(name: &str, path: &str, keyword_lower: &str) -> f64 {
    if ascii_eq_ignore_case(name, keyword_lower) {
        return 100.0;
    }

    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    if ascii_eq_ignore_case(stem, keyword_lower) {
        return 90.0;
    }

    if ascii_starts_with_ignore_case(name, keyword_lower) {
        return 75.0;
    }
    if ascii_contains_ignore_case(name, keyword_lower) {
        return 50.0;
    }
    if ascii_contains_ignore_case(path, keyword_lower) {
        return 25.0;
    }

    0.0
}

#[inline]
fn ascii_eq_ignore_case(a: &str, b_lower: &str) -> bool {
    if a.len() != b_lower.len() {
        return false;
    }

    for (x, y) in a.bytes().zip(b_lower.bytes()) {
        let xl = if x.is_ascii_uppercase() { x + 32 } else { x };
        if xl != y {
            return false;
        }
    }

    true
}

#[inline]
fn ascii_starts_with_ignore_case(haystack: &str, prefix_lower: &str) -> bool {
    if prefix_lower.len() > haystack.len() {
        return false;
    }

    for (x, y) in haystack.bytes().take(prefix_lower.len()).zip(prefix_lower.bytes()) {
        let xl = if x.is_ascii_uppercase() { x + 32 } else { x };
        if xl != y {
            return false;
        }
    }

    true
}