// File: apps/cli/src/main.rs

use anyhow::Result;
use clap::{Parser, Subcommand};
use hyperfind_common::config;
use hyperfind_common::paths;
use hyperfind_common::utils;
use hyperfind_core_engine::service::SearchService;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "hyperfind-cli")]
#[command(about = "HyperFind - Lightning-fast local file search")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize configuration with defaults
    Init,

    /// Scan a directory and add it to the index
    Scan {
        /// Directory path to scan
        dir: String,
    },

    /// Rebuild the entire index from configured directories
    Rebuild,

    /// Search for files
    Search {
        /// Search query (supports DSL: ext:, path:, size:, modified:, type:)
        query: String,

        /// Output results as JSON
        #[arg(long)]
        json: bool,

        /// Maximum number of results
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Show index statistics
    Stats,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Ensure app directories exist
    paths::ensure_dirs()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Scan { dir } => cmd_scan(&dir)?,
        Commands::Rebuild => cmd_rebuild()?,
        Commands::Search { query, json, limit } => cmd_search(&query, json, limit)?,
        Commands::Stats => cmd_stats()?,
    }

    Ok(())
}

fn cmd_init() -> Result<()> {
    let created = config::init_config()?;
    if created {
        println!("✅ Configuration initialized at {:?}", config::config_file_path()?);
        println!("Edit the config file to add directories for indexing.");
    } else {
        println!("Configuration already exists at {:?}", config::config_file_path()?);
    }
    Ok(())
}

fn cmd_scan(dir: &str) -> Result<()> {
    let cfg = config::load_config()?;
    let service = SearchService::new(cfg);

    // Try to load existing index first
    let _ = service.load_index();

    println!("Scanning directory: {}", dir);
    let count = service.scan_directory(dir)?;
    println!("✅ Scanned {} entries from {}", count, dir);

    // Also add to config if not already there
    match service.add_directory(dir) {
        Ok(_) => println!("Directory added to configuration."),
        Err(_) => println!("Directory already in configuration."),
    }

    Ok(())
}

fn cmd_rebuild() -> Result<()> {
    let cfg = config::load_config()?;

    if cfg.directories.is_empty() {
        println!("No directories configured. Use 'hyperfind-cli scan <dir>' to add one.");
        return Ok(());
    }

    let service = SearchService::new(cfg);

    println!("Rebuilding index...");
    let stats = service.rebuild_index()?;
    println!("✅ Index rebuilt successfully:");
    print_stats(&stats);

    Ok(())
}

fn cmd_search(query: &str, json_output: bool, limit: usize) -> Result<()> {
    let cfg = config::load_config()?;
    let service = SearchService::new(cfg);

    // Load existing index
    match service.load_index() {
        Ok(stats) => {
            if stats.total_documents == 0 {
                println!("Index is empty. Run 'hyperfind-cli scan <dir>' or 'hyperfind-cli rebuild' first.");
                return Ok(());
            }
        }
        Err(e) => {
            println!("Failed to load index: {}. Run 'hyperfind-cli rebuild' first.", e);
            return Ok(());
        }
    }

    let results = service.search(query)?;
    let results: Vec<_> = results.into_iter().take(limit).collect();

    if json_output {
        let json = serde_json::to_string_pretty(&results)?;
        println!("{}", json);
    } else {
        if results.is_empty() {
            println!("No results found for: {}", query);
            return Ok(());
        }

        println!("Found {} results for: {}\n", results.len(), query);

        for (i, result) in results.iter().enumerate() {
            let doc = &result.document;
            let type_indicator = if doc.is_dir { "📁" } else { "📄" };
            let size_str = if doc.is_dir {
                "-".to_string()
            } else {
                utils::format_size(doc.size)
            };
            let modified_str = doc.modified.format("%Y-%m-%d %H:%M").to_string();

            println!(
                "{:>4}. {} {} ({}, {})",
                i + 1,
                type_indicator,
                doc.path,
                size_str,
                modified_str
            );
        }
    }

    Ok(())
}

fn cmd_stats() -> Result<()> {
    let cfg = config::load_config()?;
    let service = SearchService::new(cfg);

    match service.load_index() {
        Ok(stats) => {
            print_stats(&stats);
        }
        Err(_) => {
            println!("No index found. Run 'hyperfind-cli rebuild' to create one.");
        }
    }

    Ok(())
}

fn print_stats(stats: &hyperfind_common::models::IndexStats) {
    println!("Index Statistics:");
    println!("  Total documents: {}", stats.total_documents);
    println!("  Files:           {}", stats.total_files);
    println!("  Directories:     {}", stats.total_directories);
    println!(
        "  Total size:      {}",
        utils::format_size(stats.total_size_bytes)
    );
    println!("  Indexed roots:");
    for root in &stats.indexed_roots {
        println!("    - {}", root);
    }
}