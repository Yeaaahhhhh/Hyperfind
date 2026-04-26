// File: crates/core-engine/src/parser/filters.rs

use hyperfind_common::models::{EntryType, FileDocument, SearchFilters};
use hyperfind_index_engine::bitmap::BitmapIndex;
use roaring::RoaringTreemap;

/// 直接返回 Roaring（位图运算 SIMD 友好），避免 HashSet 克隆。
pub fn compile_bitmap_filter(
    filters: &SearchFilters,
    bitmap: &BitmapIndex,
) -> Option<RoaringTreemap> {
    let mut acc: Option<RoaringTreemap> = None;

    // Extension filter
    if let Some(ref ext) = filters.extension {
        if let Some(ext_arc) = bitmap.get_by_extension_arc(ext) {
            // 第一个集合直接 clone 一次（不可避免）
            acc = Some(match acc {
                Some(existing) => existing & &*ext_arc,
                None => (*ext_arc).clone(),
            });
        } else {
            return Some(RoaringTreemap::new()); // 没该扩展名 → 空集
        }
    }

    if let Some(ref et) = filters.entry_type {
        let arc = match et {
            EntryType::File => bitmap.get_files_arc(),
            EntryType::Directory => bitmap.get_dirs_arc(),
        };
        acc = Some(match acc {
            Some(existing) => existing & &*arc,
            None => (*arc).clone(),
        });
    }

    acc
}

/// post-filter：把 path_contains 提前小写化，避免每条 doc 都 to_lowercase。
pub fn compile_post_filter(
    filters: &SearchFilters,
) -> Box<dyn Fn(&FileDocument) -> bool + Send + Sync> {
    // 预编译关键值
    let path_contains_lower: Option<String> =
        filters.path_contains.as_ref().map(|p| p.to_lowercase());
    let size_min = filters.size_min;
    let size_max = filters.size_max;
    let modified_after = filters.modified_after;
    let modified_before = filters.modified_before;

    Box::new(move |doc: &FileDocument| -> bool {
        if let Some(ref needle) = path_contains_lower {
            // 大多数路径是 ASCII，按 ASCII 比较即可避免完整 to_lowercase 分配
            let path: &str = doc.path.as_ref();
            if path.is_ascii() {
                if !ascii_contains_ignore_case(path, needle) {
                    return false;
                }
            } else {
                if !path.to_lowercase().contains(needle) {
                    return false;
                }
            }
        }
        if let Some(min) = size_min {
            if doc.size < min { return false; }
        }
        if let Some(max) = size_max {
            if doc.size > max { return false; }
        }
        if let Some(after) = modified_after {
            if doc.modified < after { return false; }
        }
        if let Some(before) = modified_before {
            if doc.modified > before { return false; }
        }
        true
    })
}

/// ASCII 不区分大小写的 contains（needle 必须已是小写）。
/// 比 `haystack.to_lowercase().contains(needle)` 快很多——零分配。
fn ascii_contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() { return true; }
    if n.len() > h.len() { return false; }
    let limit = h.len() - n.len();
    'outer: for i in 0..=limit {
        for j in 0..n.len() {
            let mut hb = h[i + j];
            if hb >= b'A' && hb <= b'Z' { hb += 32; }
            if hb != n[j] { continue 'outer; }
        }
        return true;
    }
    false
}