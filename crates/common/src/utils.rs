// File: crates/common/src/utils.rs

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use std::path::Path;
use std::time::SystemTime;

pub fn system_time_to_utc(st: SystemTime) -> DateTime<Utc> {
    let duration = st.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
        .unwrap_or_default()
}

pub fn normalize_path(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy().to_string();
    if cfg!(target_os = "windows") && s.starts_with("\\\\?\\") {
        s[4..].to_string()
    } else {
        s
    }
}

pub fn extract_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default()
}

pub fn extract_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn extract_parent(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn normalize_string(s: &str) -> String {
    s.to_lowercase()
}

pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|date| {
            date.and_hms_opt(0, 0, 0)
                .map(|naive| Utc.from_utc_datetime(&naive))
        })
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Generates trigrams from a string for inverted index.
/// "hello" → ["hel", "ell", "llo"]
pub fn generate_trigrams(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 3 {
        // For short strings, return the string itself as a single "trigram"
        if !lower.is_empty() {
            return vec![lower];
        }
        return Vec::new();
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Compute a simple CRC32-based hash for a trigram to use as a key.
pub fn trigram_hash(trigram: &str) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(trigram.as_bytes());
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trigrams() {
        let tris = generate_trigrams("hello");
        assert_eq!(tris, vec!["hel", "ell", "llo"]);
    }

    #[test]
    fn test_generate_trigrams_short() {
        let tris = generate_trigrams("hi");
        assert_eq!(tris, vec!["hi"]);
    }

    #[test]
    fn test_generate_trigrams_empty() {
        let tris = generate_trigrams("");
        assert!(tris.is_empty());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
    }
}