use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use anyhow::{Context, Result};

use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;
use blast_radius_engine::query::output::generate_context_output;
use blast_radius_engine::workspace::WorkspaceManager;
use nucleo_matcher::{Matcher, Config, Utf32String};

#[derive(Parser, Debug)]
#[command(name = "cblast", version, about = "Code Blast Radius - Analyze code dependencies and impact")]
struct Cli {
    /// Path to a folder OR a .cblast workspace file
    #[arg(short, long)]
    path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze blast radius for a symbol
    Radius {
        #[arg(short, long)]
        function_name: String,

        #[arg(long, default_value_t = true)]
        no_tests: bool,

        #[arg(long)]
        impact: Option<PathBuf>,
    },
    /// Find symbols or files using fuzzy search
    Find {
        #[arg(short, long)]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// Manage Workspaces (Multi-Root Support)
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    }
}

#[derive(Subcommand, Debug)]
enum WorkspaceAction {
    /// Initialize a new empty workspace file
    Init { name: String },
    /// Add a root folder to the workspace
    Add { root: PathBuf },
    /// Remove a root folder from the workspace
    Remove { root: PathBuf },
    /// Refresh/Sync the workspace (Check for file changes)
    Sync,
}

#[derive(serde::Serialize)]
struct MatchResult {
    name: String,
    kind: String,
    path: String,
    score: u16,
}

fn report_error(err: anyhow::Error) {
    eprintln!("{}", serde_json::json!({ "error": format!("{:?}", err) }));
    process::exit(1);
}

fn main() {
    if let Err(err) = run() {
        report_error(err);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // --- WORKSPACE MANAGEMENT COMMANDS ---
    // These operate on the .cblast file specifically
    if let Commands::Workspace { action } = cli.command {
        // For workspace commands, cli.path MUST be the .cblast file path (or where we want to create it)
        let mut manager = WorkspaceManager::new(cli.path.clone())
            .context("Failed to load/create workspace context")?;

        match action {
            WorkspaceAction::Init { name } => {
                manager.config.name = name;
                println!("Initialized workspace: {}", cli.path.display());
            }
            WorkspaceAction::Add { root } => {
                manager.add_root(root);
                println!("Added root. Total roots: {}", manager.config.roots.len());
            }
            WorkspaceAction::Remove { root } => {
                manager.remove_root(root);
                println!("Removed root. Total roots: {}", manager.config.roots.len());
            }
            WorkspaceAction::Sync => {
                manager.sync();
                println!("Workspace synced successfully.");
            }
        }
        
        manager.save().context("Failed to save workspace state")?;
        return Ok(());
    }

    // --- READ-ONLY COMMANDS (Radius / Find) ---
    // We need to load the index correctly depending on whether the user gave us a Folder or a .cblast File
    
    let index_path = if cli.path.is_file() && cli.path.extension().map_or(false, |e| e == "cblast") {
        // Multi-root mode: Index is adjacent (project.cblast.index)
        cli.path.with_extension("cblast.index")
    } else if cli.path.is_dir() {
        // Single-folder mode: Index is inside (.cblast/index.bin)
        cli.path.join(".cblast").join("index.bin")
    } else {
        anyhow::bail!("Path must be a directory or a .cblast file: {:?}", cli.path);
    };

    if !index_path.exists() {
        // If index doesn't exist, we try to scan on the fly for single folder, or fail for workspace
        if cli.path.is_dir() {
            eprintln!("Index not found. Scanning now...");
            let mut indexer = Indexer::new();
            let mut pipeline = Pipeline::new();
            pipeline.run(&mut indexer, &cli.path);
            let _ = indexer.save(&index_path); // Try save but ignore failure
            // Continue with this in-memory indexer...
            // (In a real app refactor, we would unify this logic, but this keeps existing behavior working)
        } else {
             anyhow::bail!("Index not found at {:?}. Run 'cblast workspace sync' first.", index_path);
        }
    }

    // Load the index for querying
    let indexer = Indexer::load_from_file(&index_path).unwrap_or_else(|_| Indexer::new());

    match cli.command {
        Commands::Radius { function_name, no_tests, impact: _ } => {
            let symbol_ids = find_related_symbols(
                &indexer.index,
                &indexer.lookup,
                &indexer.reverse_graph,
                &function_name
            ).ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", function_name))?;
            
            let mut symbol_ids = symbol_ids;

            if no_tests {
                 symbol_ids.retain(|&id| {
                    if let Some(sym) = indexer.index.symbols.get(&id) {
                        !sym.is_test
                    } else {
                        true
                    }
                 });
            }

            let output = generate_context_output(&indexer.index, &symbol_ids);
            println!("{}", serde_json::to_string_pretty(&output).context("Failed to serialize output")?);
        }
        Commands::Find { query, limit } => {
            let mut matcher = Matcher::new(Config::DEFAULT);
            let mut results = Vec::new();
            let query_utf32 = Utf32String::from(query.as_str());

            for sym in indexer.index.symbols.values() {
                if let Some(score) = matcher.fuzzy_match(Utf32String::from(sym.name.as_str()).slice(..), query_utf32.slice(..)) {
                    let file_path = indexer.index.files.values()
                        .find(|f| f.id == sym.file_id)
                        .map(|f| f.path.as_str())
                        .unwrap_or("unknown");

                    results.push(MatchResult {
                        name: sym.name.clone(),
                        kind: format!("{:?}", sym.kind),
                        path: file_path.to_string(),
                        score,
                    });
                }
            }

            for file in indexer.index.files.values() {
                if let Some(score) = matcher.fuzzy_match(Utf32String::from(file.path.as_str()).slice(..), query_utf32.slice(..)) {
                    results.push(MatchResult {
                        name: file.path.clone(),
                        kind: "File".to_string(),
                        path: file.path.clone(),
                        score,
                    });
                }
            }

            results.sort_by(|a, b| b.score.cmp(&a.score));
            results.truncate(limit);

            println!("{}", serde_json::to_string_pretty(&results).context("Failed to serialize search results")?);
        }
        _ => {} // Workspace handled above
    }

    Ok(())
}