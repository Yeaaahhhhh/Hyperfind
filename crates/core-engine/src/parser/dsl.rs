// File: crates/core-engine/src/parser/dsl.rs

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::{EntryType, SearchFilters, SearchQuery, SortField, SortOrder};
use hyperfind_common::utils;
use tracing::debug;

pub fn parse_query(raw: &str) -> Result<SearchQuery, HyperFindError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(SearchQuery {
            raw: String::new(),
            keywords: Vec::new(),
            filters: SearchFilters::default(),
            limit: None,
            sort_by: SortField::default(),
            sort_order: SortOrder::default(),
            search_content: false,
        });
    }

    let tokens = tokenize(raw)?;
    let mut keywords = Vec::new();
    let mut filters = SearchFilters::default();
    let mut search_content = false;

    for token in &tokens {
        if let Some(value) = strip_prefix_ci(token, "ext:") {
            filters.extension = Some(value.to_lowercase().trim_start_matches('.').to_string());
        } else if let Some(value) = strip_prefix_ci(token, "path:") {
            filters.path_contains = Some(value.to_string());
        } else if let Some(value) = strip_prefix_ci(token, "size:") {
            parse_size_filter(value, &mut filters)?;
        } else if let Some(value) = strip_prefix_ci(token, "modified:") {
            parse_modified_filter(value, &mut filters)?;
        } else if let Some(value) = strip_prefix_ci(token, "type:") {
            match value.to_lowercase().as_str() {
                "file" => filters.entry_type = Some(EntryType::File),
                "dir" | "directory" | "folder" => filters.entry_type = Some(EntryType::Directory),
                other => {
                    return Err(HyperFindError::ParseError(format!(
                        "Unknown type filter: '{}'. Expected 'file' or 'dir'.", other
                    )));
                }
            }
        } else if strip_prefix_ci(token, "content:").is_some() {
            search_content = true;
        } else {
            keywords.push(token.to_lowercase());
        }
    }

    debug!("Parsed query: keywords={:?}, filters={:?}, content={}", keywords, filters, search_content);

    Ok(SearchQuery {
        raw: raw.to_string(),
        keywords,
        filters,
        limit: None,
        sort_by: SortField::Relevance,
        sort_order: SortOrder::Descending,
        search_content,
    })
}

fn tokenize(input: &str) -> Result<Vec<String>, HyperFindError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                if in_quotes {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    in_quotes = false;
                } else {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    in_quotes = true;
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => { current.push(ch); }
        }
    }

    if in_quotes {
        return Err(HyperFindError::ParseError("Unclosed quote in query string".to_string()));
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn parse_size_filter(value: &str, filters: &mut SearchFilters) -> Result<(), HyperFindError> {
    if let Some(num_str) = value.strip_prefix('>') {
        filters.size_min = Some(num_str.parse().map_err(|_| HyperFindError::ParseError(format!("Invalid size: '{}'", num_str)))?);
    } else if let Some(num_str) = value.strip_prefix('<') {
        filters.size_max = Some(num_str.parse().map_err(|_| HyperFindError::ParseError(format!("Invalid size: '{}'", num_str)))?);
    } else {
        let size: u64 = value.parse().map_err(|_| HyperFindError::ParseError(format!("Invalid size: '{}'", value)))?;
        filters.size_min = Some(size);
        filters.size_max = Some(size);
    }
    Ok(())
}

fn parse_modified_filter(value: &str, filters: &mut SearchFilters) -> Result<(), HyperFindError> {
    if let Some(date_str) = value.strip_prefix('>') {
        let dt = utils::parse_date(date_str).ok_or_else(|| HyperFindError::ParseError(format!("Invalid date: '{}'", date_str)))?;
        filters.modified_after = Some(dt);
    } else if let Some(date_str) = value.strip_prefix('<') {
        let dt = utils::parse_date(date_str).ok_or_else(|| HyperFindError::ParseError(format!("Invalid date: '{}'", date_str)))?;
        filters.modified_before = Some(dt);
    } else {
        let dt = utils::parse_date(value).ok_or_else(|| HyperFindError::ParseError(format!("Invalid date: '{}'", value)))?;
        filters.modified_after = Some(dt);
        filters.modified_before = Some(dt + chrono::Duration::days(1));
    }
    Ok(())
}