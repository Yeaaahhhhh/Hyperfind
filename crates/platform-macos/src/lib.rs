// File: crates/platform-macos/src/lib.rs

//! # HyperFind macOS Platform Adapter
//!
//! This crate provides macOS-specific functionality for HyperFind.
//!
//! ## Current MVP
//!
//! The MVP uses cross-platform `walkdir` + `notify` from the `collector` crate.
//! The `notify` crate already uses FSEvents on macOS, but with a generic interface.
//!
//! ## Future: FSEvents Deep Integration
//!
//! macOS FSEvents API provides a persistent, system-managed event stream for
//! filesystem changes. It can also provide historical events since a given event ID.
//!
//! Implementation plan:
//! 1. Use the `fsevent-sys` crate or raw CoreServices bindings.
//! 2. Register for events on indexed directories with `kFSEventStreamCreateFlagFileEvents`.
//! 3. Use `kFSEventStreamCreateFlagUseCFTypes` for efficient event processing.
//! 4. Persist the last event ID to enable "catch-up" on restart.
//! 5. Handle volume mount/unmount events for removable media.
//!
//! ## Future: Spotlight Metadata
//!
//! macOS Spotlight maintains a comprehensive metadata index.
//! We could use `MDQuery` to bootstrap the initial index quickly.

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;

/// Trait for macOS-specific file enumeration.
pub trait MacOsScanner {
    fn scan(&self, root: &str) -> Result<Vec<FileDocument>, HyperFindError>;
    fn name(&self) -> &str;
}

/// Trait for macOS-specific change monitoring.
pub trait MacOsWatcher {
    fn start(&self) -> Result<(), HyperFindError>;
    fn stop(&self) -> Result<(), HyperFindError>;
}

/// Placeholder macOS scanner.
/// TODO: Implement Spotlight metadata or optimized FSEvents-based scanning.
pub struct MacOsFsScanner;

impl MacOsScanner for MacOsFsScanner {
    fn scan(&self, _root: &str) -> Result<Vec<FileDocument>, HyperFindError> {
        // TODO: Implement macOS-native scanning.
        Err(HyperFindError::PlatformError(
            "macOS native scanner not yet implemented. Use the cross-platform scanner.".to_string(),
        ))
    }

    fn name(&self) -> &str {
        "macos-fs"
    }
}

/// Placeholder macOS watcher.
/// TODO: Implement FSEvents with persistent event IDs.
pub struct MacOsFsWatcher;

impl MacOsWatcher for MacOsFsWatcher {
    fn start(&self) -> Result<(), HyperFindError> {
        // TODO: Implement FSEvents monitoring.
        Err(HyperFindError::PlatformError(
            "macOS FSEvents watcher not yet implemented.".to_string(),
        ))
    }

    fn stop(&self) -> Result<(), HyperFindError> {
        Ok(())
    }
}