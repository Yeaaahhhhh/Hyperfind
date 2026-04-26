// File: crates/core-engine/src/search/matcher.rs

use hyperfind_common::models::FileDocument;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matched: bool,
    pub score: f64,
}

pub fn match_document(doc: &FileDocument, keywords: &[String]) -> MatchResult {
    if keywords.is_empty() {
        return MatchResult { matched: true, score: 1.0 };
    }

    let mut total_score = 0.0;
    let mut all_matched = true;

    for keyword in keywords {
        let score = score_keyword(doc, keyword);
        if score <= 0.0 {
            all_matched = false;
            break;
        }
        total_score += score;
    }

    if !all_matched {
        return MatchResult { matched: false, score: 0.0 };
    }

    MatchResult {
        matched: true,
        score: total_score / keywords.len() as f64,
    }
}

fn score_keyword(doc: &FileDocument, keyword: &str) -> f64 {
    let name = &doc.name_lower;
    let path_lower = doc.path.to_lowercase();

    if name == keyword { return 100.0; }

    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    if stem == keyword { return 90.0; }

    if name.starts_with(keyword) { return 75.0; }
    if name.contains(keyword) { return 50.0; }
    if path_lower.contains(keyword) { return 25.0; }

    0.0
}