// File: crates/common/src/errors.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HyperFindError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Scan error: {0}")]
    ScanError(String),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Watcher error: {0}")]
    WatcherError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Platform error: {0}")]
    PlatformError(String),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Content error: {0}")]
    ContentError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type HyperFindResult<T> = Result<T, HyperFindError>;