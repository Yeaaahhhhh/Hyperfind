// File: crates/index-engine/src/wal.rs

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::paths;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use tracing::debug;

const WAL_FILE: &str = "wal.log";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEvent {
    pub event_type: WalEventType,
    pub path: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalEventType {
    Added,
    Modified,
    Removed,
    Renamed { from: String },
}

pub fn append_event(event: &WalEvent) -> Result<(), HyperFindError> {
    let wal_path = paths::index_dir()?.join(WAL_FILE);
    if let Some(parent) = wal_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event).map_err(|e| {
        HyperFindError::SerializationError(format!("Failed to serialize WAL event: {}", e))
    })?;
    let mut file = OpenOptions::new().create(true).append(true).open(&wal_path)?;
    writeln!(file, "{}", line)?;
    debug!("WAL event appended: {:?}", event.event_type);
    Ok(())
}

pub fn read_events() -> Result<Vec<WalEvent>, HyperFindError> {
    let wal_path = paths::index_dir()?.join(WAL_FILE);
    if !wal_path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&wal_path)?;
    let mut events = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        match serde_json::from_str::<WalEvent>(trimmed) {
            Ok(event) => events.push(event),
            Err(e) => { tracing::warn!("Skipping corrupt WAL line: {}", e); }
        }
    }
    Ok(events)
}

pub fn clear_wal() -> Result<(), HyperFindError> {
    let wal_path = paths::index_dir()?.join(WAL_FILE);
    if wal_path.exists() {
        fs::write(&wal_path, "")?;
    }
    Ok(())
}