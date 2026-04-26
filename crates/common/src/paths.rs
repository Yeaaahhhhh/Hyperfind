// File: crates/common/src/paths.rs

use crate::errors::HyperFindError;
use std::path::PathBuf;

const APP_NAME: &str = "hyperfind";

pub fn data_dir() -> Result<PathBuf, HyperFindError> {
    let base = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_fallback_home().join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        dirs_fallback_home().join("Library").join("Application Support")
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_fallback_home().join(".local").join("share"))
    };
    Ok(base.join(APP_NAME))
}

pub fn config_dir() -> Result<PathBuf, HyperFindError> {
    let base = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_fallback_home().join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        dirs_fallback_home().join("Library").join("Application Support")
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs_fallback_home().join(".config"))
    };
    Ok(base.join(APP_NAME))
}

pub fn index_dir() -> Result<PathBuf, HyperFindError> {
    Ok(data_dir()?.join("index"))
}

pub fn log_dir() -> Result<PathBuf, HyperFindError> {
    Ok(data_dir()?.join("logs"))
}

pub fn segments_dir() -> Result<PathBuf, HyperFindError> {
    Ok(index_dir()?.join("segments"))
}

pub fn ensure_dirs() -> Result<(), HyperFindError> {
    let dirs = [data_dir()?, config_dir()?, index_dir()?, log_dir()?, segments_dir()?];
    for dir in &dirs {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

fn dirs_fallback_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}