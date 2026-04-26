// File: crates/plugin-api/src/lib.rs

//! HyperFind Plugin API
//!
//! Provides a trait-based plugin system for extending HyperFind functionality.
//! Plugins can be loaded dynamically from shared libraries (.dll/.so/.dylib).

use hyperfind_common::errors::HyperFindError;
use hyperfind_common::models::{FileDocument, SearchResult};
use std::path::Path;
use tracing::{info, warn};

/// The plugin trait that all plugins must implement.
pub trait HyperFindPlugin: Send + Sync {
    /// Returns the plugin name.
    fn name(&self) -> &str;

    /// Returns the plugin version.
    fn version(&self) -> &str;

    /// Called when the plugin is loaded.
    fn on_init(&self) -> Result<(), HyperFindError>;

    /// Called for each document during indexing. Plugins can extract additional metadata.
    fn on_index_document(&self, doc: &mut FileDocument) -> Result<(), HyperFindError> {
        let _ = doc;
        Ok(())
    }

    /// Called during search. Plugins can add or re-rank results.
    fn on_search(
        &self,
        query: &str,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), HyperFindError> {
        let _ = (query, results);
        Ok(())
    }

    /// Called when the plugin is unloaded.
    fn on_shutdown(&self) -> Result<(), HyperFindError> {
        Ok(())
    }
}

/// Plugin manager that loads and manages plugins.
pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
}

struct LoadedPlugin {
    plugin: Box<dyn HyperFindPlugin>,
    _library: Option<libloading::Library>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Loads a plugin from a shared library.
    ///
    /// The library must export a function:
    /// `extern "C" fn hyperfind_plugin_create() -> *mut dyn HyperFindPlugin`
    pub fn load_plugin(&mut self, path: &Path) -> Result<(), HyperFindError> {
        unsafe {
            let library = libloading::Library::new(path).map_err(|e| {
                HyperFindError::PluginError(format!("Failed to load plugin {:?}: {}", path, e))
            })?;

            let constructor: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn HyperFindPlugin> =
                library.get(b"hyperfind_plugin_create").map_err(|e| {
                    HyperFindError::PluginError(format!(
                        "Plugin {:?} missing 'hyperfind_plugin_create' symbol: {}", path, e
                    ))
                })?;

            let raw = constructor();
            let plugin = Box::from_raw(raw);

            plugin.on_init()?;
            info!("Plugin loaded: {} v{}", plugin.name(), plugin.version());

            self.plugins.push(LoadedPlugin {
                plugin,
                _library: Some(library),
            });
        }

        Ok(())
    }

    /// Registers a built-in plugin (no dynamic loading).
    pub fn register_plugin(&mut self, plugin: Box<dyn HyperFindPlugin>) {
        if let Err(e) = plugin.on_init() {
            warn!("Plugin {} failed to init: {}", plugin.name(), e);
            return;
        }
        info!("Plugin registered: {} v{}", plugin.name(), plugin.version());
        self.plugins.push(LoadedPlugin {
            plugin,
            _library: None,
        });
    }

    /// Calls `on_index_document` on all plugins.
    pub fn on_index_document(&self, doc: &mut FileDocument) {
        for loaded in &self.plugins {
            if let Err(e) = loaded.plugin.on_index_document(doc) {
                warn!("Plugin {} index error: {}", loaded.plugin.name(), e);
            }
        }
    }

    /// Calls `on_search` on all plugins.
    pub fn on_search(&self, query: &str, results: &mut Vec<SearchResult>) {
        for loaded in &self.plugins {
            if let Err(e) = loaded.plugin.on_search(query, results) {
                warn!("Plugin {} search error: {}", loaded.plugin.name(), e);
            }
        }
    }

    /// Shuts down all plugins.
    pub fn shutdown(&self) {
        for loaded in &self.plugins {
            if let Err(e) = loaded.plugin.on_shutdown() {
                warn!("Plugin {} shutdown error: {}", loaded.plugin.name(), e);
            }
        }
    }

    /// Returns the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginManager {
    fn default() -> Self { Self::new() }
}