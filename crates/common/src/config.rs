// File: crates/common/src/config.rs

use crate::errors::HyperFindError;
use crate::models::AppConfig;
use crate::paths;
use std::fs;
use std::path::Path;
use tracing::{debug, info};

const CONFIG_FILE_NAME: &str = "config.json";

pub fn config_file_path() -> Result<std::path::PathBuf, HyperFindError> {
    let config_dir = paths::config_dir()?;
    Ok(config_dir.join(CONFIG_FILE_NAME))
}

pub fn load_config() -> Result<AppConfig, HyperFindError> {
    let path = config_file_path()?;
    if !path.exists() {
        info!("Config file not found at {:?}, using defaults", path);
        return Ok(AppConfig::default());
    }

    debug!("Loading config from {:?}", path);
    let content = fs::read_to_string(&path).map_err(|e| {
        HyperFindError::ConfigError(format!("Failed to read config file {:?}: {}", path, e))
    })?;

    let config: AppConfig = serde_json::from_str(&content).map_err(|e| {
        HyperFindError::ConfigError(format!("Failed to parse config file {:?}: {}", path, e))
    })?;

    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<(), HyperFindError> {
    let path = config_file_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            HyperFindError::ConfigError(format!(
                "Failed to create config directory {:?}: {}",
                parent, e
            ))
        })?;
    }

    let content = serde_json::to_string_pretty(config).map_err(|e| {
        HyperFindError::ConfigError(format!("Failed to serialize config: {}", e))
    })?;

    fs::write(&path, content).map_err(|e| {
        HyperFindError::ConfigError(format!("Failed to write config file {:?}: {}", path, e))
    })?;

    info!("Config saved to {:?}", path);
    Ok(())
}

pub fn init_config() -> Result<bool, HyperFindError> {
    let path = config_file_path()?;
    if path.exists() {
        return Ok(false);
    }
    let default_config = AppConfig::default();
    save_config(&default_config)?;
    info!("Initialized default config at {:?}", path);
    Ok(true)
}

pub fn validate_directory(path: &str) -> Result<(), HyperFindError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(HyperFindError::ConfigError(format!(
            "Directory does not exist: {}", path
        )));
    }
    if !p.is_dir() {
        return Err(HyperFindError::ConfigError(format!(
            "Path is not a directory: {}", path
        )));
    }
    Ok(())
}