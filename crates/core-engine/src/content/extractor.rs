// File: crates/core-engine/src/content/extractor.rs

//! Content extraction for full-text search.
//! Reads text content from files for indexing and search-time snippet generation.

use hyperfind_common::errors::HyperFindError;
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::debug;

const MAX_READ_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Extracts text content from a file.
/// Returns None if the file is binary or unreadable.
pub fn extract_content(
    path: &Path,
    max_size: u64,
    allowed_extensions: &[String],
) -> Option<String> {
    // Check extension
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !allowed_extensions.iter().any(|a| a == &ext) {
        return None;
    }

    // Check size
    let metadata = path.metadata().ok()?;
    if metadata.len() > max_size || metadata.len() > MAX_READ_BYTES as u64 {
        return None;
    }

    // Read file
    let mut file = fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut buf).ok()?;

    // Check if content is valid UTF-8 (text file)
    match String::from_utf8(buf) {
        Ok(content) => {
            debug!("Extracted {} bytes of content from {:?}", content.len(), path);
            Some(content)
        }
        Err(_) => None,
    }
}

/// Generates a snippet of content around a keyword match.
pub fn generate_snippet(content: &str, keyword: &str, context_chars: usize) -> Option<String> {
    let lower_content = content.to_lowercase();
    let lower_keyword = keyword.to_lowercase();

    if let Some(pos) = lower_content.find(&lower_keyword) {
        let start = pos.saturating_sub(context_chars);
        let end = (pos + keyword.len() + context_chars).min(content.len());

        // Find safe UTF-8 boundaries
        let start = content[..start].rfind(char::is_whitespace).map(|p| p + 1).unwrap_or(start);
        let end = content[end..].find(char::is_whitespace).map(|p| end + p).unwrap_or(end);

        let mut snippet = String::new();
        if start > 0 { snippet.push_str("..."); }
        snippet.push_str(content[start..end].trim());
        if end < content.len() { snippet.push_str("..."); }

        Some(snippet)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_snippet() {
        let content = "This is a test file with some content about Rust programming.";
        let snippet = generate_snippet(content, "Rust", 20).unwrap();
        assert!(snippet.contains("Rust"));
    }
}