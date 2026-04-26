// File: crates/collector/src/normalize.rs

use std::path::{Path, PathBuf};

pub fn normalize_path(path: &Path) -> PathBuf {
    let result = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if cfg!(target_os = "windows") {
        let s = result.to_string_lossy();
        if let Some(stripped) = s.strip_prefix("\\\\?\\") {
            return PathBuf::from(stripped);
        }
    }
    result
}

pub fn extract_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern == "*" { return true; }
    if let Some(suffix) = pattern.strip_prefix('*') { return name.ends_with(suffix); }
    if let Some(prefix) = pattern.strip_suffix('*') { return name.starts_with(prefix); }
    name == pattern
}