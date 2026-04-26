// File: crates/common/src/utils.rs

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use std::path::Path;
use std::time::SystemTime;

pub fn system_time_to_utc(st: SystemTime) -> DateTime<Utc> {
    let duration = st.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
        .unwrap_or_default()
}

/// 轻量路径规范化：
/// - 不再调用 `canonicalize()`，避免每个文件一次昂贵的文件系统访问
/// - 统一分隔符
/// - Windows 下去掉 `\\?\` 前缀
pub fn normalize_path(path: &Path) -> String {
    let mut s = path.to_string_lossy().into_owned();

    if cfg!(target_os = "windows") {
        if s.starts_with("\\\\?\\") {
            s.drain(..4);
        }
        s = s.replace('/', "\\");
    } else {
        s = s.replace('\\', "/");
    }

    s
}

pub fn extract_extension(path: &Path) -> String {
    let ext = match path.extension().and_then(|ext| ext.to_str()) {
        Some(v) => v,
        None => return String::new(),
    };

    if ext.is_ascii() {
        let mut out = String::with_capacity(ext.len());
        for b in ext.bytes() {
            out.push((b as char).to_ascii_lowercase());
        }
        out
    } else {
        ext.to_lowercase()
    }
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
    if s.is_ascii() {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            out.push((b as char).to_ascii_lowercase());
        }
        out
    } else {
        s.to_lowercase()
    }
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

/// 把一个 trigram (3 个 char) packed 进 u32。
#[inline]
pub fn pack_trigram(c0: char, c1: char, c2: char) -> u32 {
    if c0.is_ascii() && c1.is_ascii() && c2.is_ascii() {
        ((c0 as u32) & 0x7F) << 14
            | ((c1 as u32) & 0x7F) << 7
            | ((c2 as u32) & 0x7F)
    } else {
        use std::hash::Hasher;

        let mut h = rustc_hash::FxHasher::default();
        let mut buf = [0u8; 12];
        let mut p = 0usize;

        for ch in [c0, c1, c2] {
            let s = ch.encode_utf8(&mut buf[p..]);
            p += s.len();
        }

        h.write(&buf[..p]);
        (h.finish() as u32) | 0x8000_0000
    }
}

/// 更偏热路径优化的 trigram 生成：
/// - ASCII 直接按字节滑窗，无 `Vec<char>`
/// - 非 ASCII 保持较安全路径
pub fn trigram_codes(s: &str) -> Vec<u32> {
    if s.is_empty() {
        return Vec::new();
    }

    if s.is_ascii() {
        let bytes = s.as_bytes();
        if bytes.len() < 3 {
            let c0 = bytes
                .first()
                .copied()
                .map(|b| (b as char).to_ascii_lowercase())
                .unwrap_or(' ');
            let c1 = bytes
                .get(1)
                .copied()
                .map(|b| (b as char).to_ascii_lowercase())
                .unwrap_or(' ');
            return vec![pack_trigram(c0, c1, ' ')];
        }

        let mut out = Vec::with_capacity(bytes.len().saturating_sub(2));
        for i in 0..=bytes.len() - 3 {
            let c0 = (bytes[i] as char).to_ascii_lowercase();
            let c1 = (bytes[i + 1] as char).to_ascii_lowercase();
            let c2 = (bytes[i + 2] as char).to_ascii_lowercase();
            out.push(pack_trigram(c0, c1, c2));
        }
        out.sort_unstable();
        out.dedup();
        return out;
    }

    let lower = s.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();

    if chars.len() < 3 {
        if chars.is_empty() {
            return Vec::new();
        }
        let c0 = chars.first().copied().unwrap_or(' ');
        let c1 = chars.get(1).copied().unwrap_or(' ');
        return vec![pack_trigram(c0, c1, ' ')];
    }

    let mut out = Vec::with_capacity(chars.len().saturating_sub(2));
    for w in chars.windows(3) {
        out.push(pack_trigram(w[0], w[1], w[2]));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// 旧 API 兼容保留。
pub fn generate_trigrams(s: &str) -> Vec<String> {
    let lower = s.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 3 {
        if !lower.is_empty() {
            return vec![lower];
        }
        return Vec::new();
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

pub fn trigram_hash(trigram: &str) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(trigram.as_bytes());
    hasher.finalize()
}

#[inline]
pub fn ascii_contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();

    if n.is_empty() {
        return true;
    }
    if n.len() > h.len() {
        return false;
    }

    let limit = h.len() - n.len();
    'outer: for i in 0..=limit {
        for j in 0..n.len() {
            let mut hb = h[i + j];
            if hb.is_ascii_uppercase() {
                hb += 32;
            }
            if hb != n[j] {
                continue 'outer;
            }
        }
        return true;
    }

    false
}

#[inline]
pub fn ascii_cmp_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let min_len = ab.len().min(bb.len());

    for i in 0..min_len {
        let mut x = ab[i];
        let mut y = bb[i];

        if x.is_ascii_uppercase() {
            x += 32;
        }
        if y.is_ascii_uppercase() {
            y += 32;
        }

        match x.cmp(&y) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }

    ab.len().cmp(&bb.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigram_codes() {
        let a = trigram_codes("hello");
        assert_eq!(a.len(), 3);
        assert_eq!(a, trigram_codes("HELLO"));
    }

    #[test]
    fn test_pack_ascii_distinct() {
        assert_ne!(pack_trigram('a', 'b', 'c'), pack_trigram('a', 'b', 'd'));
    }

    #[test]
    fn test_ascii_contains_ignore_case() {
        assert!(ascii_contains_ignore_case("HelloWorld", "world"));
        assert!(ascii_contains_ignore_case("ABCDEF", "bcd"));
        assert!(!ascii_contains_ignore_case("ABCDEF", "bd"));
    }

    #[test]
    fn test_ascii_cmp_ignore_case() {
        assert_eq!(ascii_cmp_ignore_case("Abc", "abc"), std::cmp::Ordering::Equal);
        assert_eq!(ascii_cmp_ignore_case("abc", "abd"), std::cmp::Ordering::Less);
    }
}