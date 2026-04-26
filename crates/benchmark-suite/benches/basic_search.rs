// File: benches/basic_search.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chrono::Utc;
use hyperfind_common::models::FileDocument;
use hyperfind_index_engine::index_store::IndexStore;

/// Generates a vector of test documents for benchmarking.
fn generate_test_documents(count: usize) -> Vec<FileDocument> {
    let extensions = ["rs", "txt", "md", "json", "toml", "py", "js", "ts", "html", "css"];
    let dirs = [
        "/project/src",
        "/project/tests",
        "/project/docs",
        "/project/config",
        "/home/user/downloads",
        "/home/user/documents",
        "/var/log",
        "/tmp/build",
    ];

    (0..count)
        .map(|i| {
            let ext = extensions[i % extensions.len()];
            let dir = dirs[i % dirs.len()];
            let name = format!("file_{}.{}", i, ext);
            let path = format!("{}/{}", dir, name);

            FileDocument {
                id: i as u64,
                name: name.clone(),
                name_lower: name.to_lowercase(),
                path: path.clone(),
                parent: dir.to_string(),
                extension: ext.to_string(),
                size: ((i * 137 + 42) % 1048576) as u64,
                modified: Utc::now(),
                is_dir: false,
            }
        })
        .collect()
}

fn bench_search_substring(c: &mut Criterion) {
    let docs = generate_test_documents(100_000);
    let store = IndexStore::new();
    store.load(docs);

    c.bench_function("search_substring_100k_docs", |b| {
        b.iter(|| {
            let results = store.search_with(|doc| {
                doc.name_lower.contains(black_box("file_500"))
            });
            black_box(results);
        });
    });
}

fn bench_search_with_extension_filter(c: &mut Criterion) {
    let docs = generate_test_documents(100_000);
    let store = IndexStore::new();
    store.load(docs);

    c.bench_function("search_ext_filter_100k_docs", |b| {
        b.iter(|| {
            let results = store.search_with(|doc| {
                doc.extension == "rs" && doc.name_lower.contains(black_box("file"))
            });
            black_box(results);
        });
    });
}

fn bench_index_load(c: &mut Criterion) {
    let docs = generate_test_documents(100_000);

    c.bench_function("index_load_100k_docs", |b| {
        b.iter(|| {
            let store = IndexStore::new();
            store.load(black_box(docs.clone()));
        });
    });
}

criterion_group!(
    benches,
    bench_search_substring,
    bench_search_with_extension_filter,
    bench_index_load
);
criterion_main!(benches);