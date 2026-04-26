// File: crates/platform-linux/src/lib.rs

//! # HyperFind Linux Platform Adapter
//!
//! This crate provides Linux-specific functionality for HyperFind.
//!
//! ## Current MVP
//!
//! The MVP uses cross-platform `walkdir` + `notify` from the `collector` crate.
//! The `notify` crate uses `inotify` on Linux, but is limited by the number of
//! watches and the need to register each directory individually.
//!
//! ## Future: inotify Enhancement
//!
//! The current `notify` usage adds recursive inotify watches. This hits the
//! `fs.inotify.max_user_watches` limit on large directory trees.
//!
//! Implementation plan:
//! 1. Increase `max_user_watches` or gracefully degrade.
//! 2. Use a two-tier approach: inotify for hot directories, periodic rescan for cold ones.
//!
//! ## Future: fanotify
//!
//! `fanotify` (file access notification) provides system-wide or mount-point-wide
//! monitoring without per-directory watch limits.
//!
//! Implementation plan:
//! 1. Use `fanotify_init` with `FAN_CLASS_NOTIF` and `FAN_REPORT_FID`.
//! 2. Mark mount points with `fanotify_mark`.
//! 3. Read events from the fanotify file descriptor.
//! 4. Requires `CAP_SYS_ADMIN` capability.
//!
//! ## Future: eBPF-based monitoring
//!
//! For ultimate performance, eBPF programs can trace VFS operations at kernel level
//! with negligible overhead. This would require:
//! - BPF CO-RE (Compile Once, Run Everywhere) for kernel compatibility.
//! - A user-space daemon to receive BPF events via ring buffer.

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::FileDocument;

/// Trait for Linux-specific file enumeration.
pub trait LinuxScanner {
    fn scan(&self, root: &str) -> Result<Vec<FileDocument>, HyperFindError>;
    fn name(&self) -> &str;
}

/// Trait for Linux-specific change monitoring.
pub trait LinuxWatcher {
    fn start(&self) -> Result<(), HyperFindError>;
    fn stop(&self) -> Result<(), HyperFindError>;
}

/// Placeholder Linux scanner.
/// TODO: Implement getdents64-based fast scanning or /proc/self/fd tricks.
pub struct LinuxFsScanner;

impl LinuxScanner for LinuxFsScanner {
    fn scan(&self, _root: &str) -> Result<Vec<FileDocument>, HyperFindError> {
        // TODO: Implement Linux-native fast scanning.
        Err(HyperFindError::PlatformError(
            "Linux native scanner not yet implemented. Use the cross-platform scanner.".to_string(),
        ))
    }

    fn name(&self) -> &str {
        "linux-fs"
    }
}

/// Placeholder Linux watcher.
/// TODO: Implement fanotify-based monitoring.
pub struct LinuxFanotifyWatcher;

impl LinuxWatcher for LinuxFanotifyWatcher {
    fn start(&self) -> Result<(), HyperFindError> {
        // TODO: Implement fanotify monitoring.
        Err(HyperFindError::PlatformError(
            "Linux fanotify watcher not yet implemented.".to_string(),
        ))
    }

    fn stop(&self) -> Result<(), HyperFindError> {
        Ok(())
    }
}