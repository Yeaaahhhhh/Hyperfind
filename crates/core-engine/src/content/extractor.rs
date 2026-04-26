// File: crates/core-engine/src/content/extractor.rs

//! Content extraction for full-text search.
//! Reads text content from files for indexing and search-time snippet generation.

use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::debug;

const MAX_READ_BYTES: usize = 10 * 1024 * 1024;

pub fn extract_content(
    path: &Path,
    max_size: u64,
    allowed_extensions: &[String],
) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if !extension_allowed(ext, allowed_extensions) {
        return None;
    }

    let metadata = path.metadata().ok()?;
    let size = metadata.len();
    if size > max_size || size > MAX_READ_BYTES as u64 {
        return None;
    }

    let mut file = fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(size as usize);
    file.read_to_end(&mut buf).ok()?;

    match String::from_utf8(buf) {
        Ok(content) => {
            debug!("Extracted {} bytes of content from {:?}", content.len(), path);
            Some(content)
        }
        Err(_) => None,
    }
}

pub fn generate_snippet(content: &str, keyword: &str, context_chars: usize) -> Option<String> {
    if content.is_empty() || keyword.is_empty() {
        return None;
    }

    let found = if content.is_ascii() && keyword.is_ascii() {
        find_ascii_ignore_case(content, keyword)
    } else {
        let lower_content = content.to_lowercase();
        let lower_keyword = keyword.to_lowercase();
        lower_content.find(&lower_keyword)
    }?;

    let start_hint = found.saturating_sub(context_chars);
    let end_hint = (found + keyword.len() + context_chars).min(content.len());

    let start = floor_char_boundary(content, start_hint);
    let end = ceil_char_boundary(content, end_hint);

    let start = content[..start]
        .rfind(char::is_whitespace)
        .map(|p| p + 1)
        .unwrap_or(start);

    let end = content[end..]
        .find(char::is_whitespace)
        .map(|p| end + p)
        .unwrap_or(end);

    let mut snippet = String::with_capacity((end - start).min(256) + 6);
    if start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(content[start..end].trim());
    if end < content.len() {
        snippet.push_str("...");
    }

    Some(snippet)
}

#[inline]
fn extension_allowed(ext: &str, allowed_extensions: &[String]) -> bool {
    if ext.is_empty() {
        return false;
    }

    if ext.is_ascii() {
        'outer: for allowed in allowed_extensions {
            if allowed.len() != ext.len() {
                continue;
            }

            for (a, b) in ext.bytes().zip(allowed.bytes()) {
                let al = if a.is_ascii_uppercase() { a + 32 } else { a };
                if al != b {
                    continue 'outer;
                }
            }

            return true;
        }

        false
    } else {
        let lower = ext.to_lowercase();
        allowed_extensions.iter().any(|a| a == &lower)
    }
}

#[inline]
fn find_ascii_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();

    if n.is_empty() {
        return Some(0);
    }
    if n.len() > h.len() {
        return None;
    }

    let limit = h.len() - n.len();
    'outer: for i in 0..=limit {
        for j in 0..n.len() {
            let mut hb = h[i + j];
            let mut nb = n[j];

            if hb.is_ascii_uppercase() {
                hb += 32;
            }
            if nb.is_ascii_uppercase() {
                nb += 32;
            }

            if hb != nb {
                continue 'outer;
            }
        }
        return Some(i);
    }

    None
}

#[inline]
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

#[inline]
fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
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

    #[test]
    fn test_find_ascii_ignore_case() {
        assert_eq!(find_ascii_ignore_case("Hello Rust World", "rust"), Some(6));
        assert_eq!(find_ascii_ignore_case("Hello Rust World", "RUST"), Some(6));
        assert_eq!(find_ascii_ignore_case("Hello Rust World", "java"), None);
    }
}