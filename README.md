<!-- File: README.md -->
# HyperFind

A cross-platform, high-performance local file search tool — inspired by Everything, built in Rust.

## Features

- **Blazing fast file name search** across multiple directories
- **Cross-platform**: Windows 10/11, macOS, Linux
- **DSL-based query filters**: `ext:`, `path:`, `size:`, `modified:`, `type:`
- **Real-time incremental updates** via filesystem watcher
- **Desktop GUI** powered by Tauri + React + TypeScript
- **CLI** for automation and scripting
- **Extensible architecture** designed for future AI-enhanced search, content indexing, and platform-native optimizations

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Core Engine | Rust, tokio, rayon |
| Index | Custom JSONL-based (MVP), future segment-based |
| Desktop GUI | Tauri v1, React, TypeScript, Vite |
| CLI | Rust, clap |
| File Watching | notify (cross-platform) |
| Configuration | JSON config files |

## Project Structure

```text
hyperfind/
├── apps/
│   ├── cli/          # Command-line interface
│   └── desktop/      # Tauri desktop application
├── crates/
│   ├── common/           # Shared models, config, errors, utilities
│   ├── core-engine/      # Search engine, DSL parser, ranking
│   ├── index-engine/     # Index storage, loading, writing
│   ├── collector/        # File scanning, watching, scheduling
│   ├── platform-windows/ # Windows-specific adapters (future MFT/USN)
│   ├── platform-macos/   # macOS-specific adapters (future FSEvents)
│   ├── platform-linux/   # Linux-specific adapters (future fanotify)
│   └── benchmark-suite/  # Performance benchmarks
├── docs/             # Architecture, development, performance docs
├── scripts/          # Build and init scripts
└── testdata/         # Sample files for testing