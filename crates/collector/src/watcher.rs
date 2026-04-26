// File: crates/collector/src/watcher.rs

use hyperfind_common::errors::HyperFindError;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub type WatchCallback = Box<dyn Fn(WatchEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(String),
    Modified(String),
    Removed(String),
    Renamed { from: String, to: String },
}

/// Starts a filesystem watcher. Blocks the calling thread.
pub fn start_watcher(
    directories: &[String],
    callback: WatchCallback,
) -> Result<(), HyperFindError> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, Config::default().with_poll_interval(Duration::from_secs(2)))
            .map_err(|e| HyperFindError::WatcherError(format!("Failed to create watcher: {}", e)))?;

    for dir in directories {
        let path = Path::new(dir);
        if path.exists() && path.is_dir() {
            watcher.watch(path, RecursiveMode::Recursive)
                .map_err(|e| HyperFindError::WatcherError(format!("Failed to watch {}: {}", dir, e)))?;
            info!("Watching directory: {}", dir);
        } else {
            warn!("Skipping non-existent directory for watching: {}", dir);
        }
    }

    info!("Filesystem watcher started");

    for result in rx {
        match result {
            Ok(event) => {
                let events = convert_event(&event);
                for watch_event in events {
                    debug!("Watch event: {:?}", watch_event);
                    callback(watch_event);
                }
            }
            Err(e) => {
                error!("Watcher error: {}. Consider triggering a partial rescan.", e);
            }
        }
    }

    Ok(())
}

fn convert_event(event: &Event) -> Vec<WatchEvent> {
    let paths: Vec<String> = event.paths.iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();

    match &event.kind {
        EventKind::Create(_) => paths.into_iter().map(WatchEvent::Created).collect(),
        EventKind::Modify(_) => paths.into_iter().map(WatchEvent::Modified).collect(),
        EventKind::Remove(_) => paths.into_iter().map(WatchEvent::Removed).collect(),
        _ => Vec::new(),
    }
}