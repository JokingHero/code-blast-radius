use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process;
use anyhow::{Context, Result};

use blast_radius_engine::query::traversal::{GraphWalker, TraversalMode};
use blast_radius_engine::query::output::generate_context_output;
use blast_radius_engine::workspace::WorkspaceManager;
use blast_radius_engine::recipes::executor::RecipeExecutor;
use blast_radius_engine::recipes::models::Recipe;
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
    /// Analyze blast radius for a symbol OR a file path.
    /// If the argument matches a file path, it calculates the combined impact of all symbols in that file.
    Radius {
        /// Function name, Class name, or File path (relative)
        symbol: String,

        #[arg(long, default_value_t = true)]
        no_tests: bool,
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
    },
    /// Manage and Run Context Recipes
    Recipe {
        #[command(subcommand)]
        action: RecipeAction,
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
    /// Refresh/Sync the workspace (Explicit command, though Sync happens automatically now)
    Sync,
}

#[derive(Subcommand, Debug)]
enum RecipeAction {
    /// List all recipes defined in the workspace
    List,
    /// Add or Overwrite a recipe. Expects a JSON string definition.
    Add { 
        #[arg(short, long)]
        json: String 
    },
    /// Remove a recipe by name
    Remove { 
        name: String 
    },
    /// Run a specific recipe and output the context JSON
    Run { 
        name: String 
    },
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

/// Helper to normalize where Config and Index live based on user input
fn resolve_paths(input: &Path) -> Result<(PathBuf, PathBuf)> {
    if input.is_file() && input.extension().map_or(false, |e| e == "cblast") {
        // Explicit Workspace File: ./my-project.cblast
        let config_path = input.to_path_buf();
        let index_path = input.with_extension("cblast.index");
        Ok((config_path, index_path))
    } else if input.is_dir() {
        // Implicit Directory Workspace: ./my-project/.cblast/
        let cblast_dir = input.join(".cblast");
        if !cblast_dir.exists() {
            std::fs::create_dir(&cblast_dir).ok(); 
        }
        let config_path = cblast_dir.join("workspace.json");
        let index_path = cblast_dir.join("index.bin");
        Ok((config_path, index_path))
    } else {
        anyhow::bail!("Path must be a directory or a .cblast file");
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // 1. Resolve Paths
    let (config_path, _) = resolve_paths(&cli.path)?;

    // 2. Load Manager (Handles Config + Index loading)
    let mut manager = WorkspaceManager::new(config_path.clone())
        .context("Failed to load workspace")?;

    // 3. Auto-Add Root in Directory Mode
    // If we are in directory mode and the config is empty (newly created), 
    // automatically add the directory itself as a root.
    if cli.path.is_dir() && manager.config.roots.is_empty() {
        let abs_root = std::fs::canonicalize(&cli.path).unwrap_or(cli.path.clone());
        manager.add_root(abs_root);
    }

    // 4. SYNC (Incremental Update)
    // Always sync to ensure freshness before any operation.
    // This is critical for Recipes to ensure AST byte offsets match disk content.
    manager.sync();
    manager.save().context("Failed to save workspace state")?;

    match &cli.command {
        // --- WORKSPACE MANAGEMENT ---
        Commands::Workspace { action } => {
            match action {
                WorkspaceAction::Init { name } => {
                    manager.config.name = name.clone();
                    manager.save()?;
                    println!("Initialized workspace config at {:?}", config_path);
                }
                WorkspaceAction::Add { root } => {
                    manager.add_root(root.clone());
                    manager.save()?;
                    println!("Added root. Total roots: {}", manager.config.roots.len());
                }
                WorkspaceAction::Remove { root } => {
                    manager.remove_root(root.clone());
                    manager.save()?;
                    println!("Removed root. Total roots: {}", manager.config.roots.len());
                }
                WorkspaceAction::Sync => {
                    println!("Workspace synced successfully.");
                }
            }
        }

        // --- RECIPE MANAGEMENT ---
        Commands::Recipe { action } => {
            match action {
                RecipeAction::List => {
                    let names: Vec<String> = manager.config.recipes.keys().cloned().collect();
                    println!("{}", serde_json::to_string_pretty(&names)?);
                }
                RecipeAction::Add { json } => {
                    let recipe: Recipe = serde_json::from_str(json)
                        .context("Invalid recipe JSON")?;
                    let name = recipe.name.clone();
                    manager.config.recipes.insert(name.clone(), recipe);
                    manager.save()?;
                    println!("Recipe '{}' saved.", name);
                }
                RecipeAction::Remove { name } => {
                    if manager.config.recipes.remove(name).is_some() {
                        manager.save()?;
                        println!("Recipe '{}' Removed.", name);
                    } else {
                        eprintln!("Recipe '{}' not found.", name);
                        process::exit(1);
                    }
                }
                RecipeAction::Run { name } => {
                    let recipe = manager.config.recipes.get(name)
                        .ok_or_else(|| anyhow::anyhow!("Recipe '{}' not found", name))?;

                    let executor = RecipeExecutor::new(&manager.indexer);
                    let output = executor.execute(recipe)?;
                    
                    println!("{}", serde_json::to_string_pretty(&output)?);
                }
            }
        }

        // --- QUERY: RADIUS (Smart Impact) ---
        Commands::Radius { symbol, no_tests } => {
            let indexer = &manager.indexer;
            
            // Step A: Check if input is a File Path
            // We search for files ending with the input string (fuzzy path match)
            let matching_file_id = indexer.index.files.values()
                .find(|f| f.path.ends_with(symbol) || f.path == *symbol)
                .map(|f| f.id);

            let mut start_ids = Vec::new();

            if let Some(fid) = matching_file_id {
                // It's a file! Gather all symbols defined in this file.
                for sym in indexer.index.symbols.values() {
                    if sym.file_id == fid {
                        start_ids.push(sym.id);
                    }
                }
                if start_ids.is_empty() {
                    // Fallback: If file exists but has no symbols (e.g. pure script), 
                    // we can't traverse graph from it, but maybe we should output the file itself?
                    // For now, warn.
                    eprintln!("File found, but it contains no indexed symbols to trace from.");
                    return Ok(());
                }
            } else {
                // It's a Symbol Name! Lookup IDs.
                if let Some(ids) = indexer.lookup.symbol_map.get(symbol) {
                    start_ids.extend(ids.iter());
                } else {
                    anyhow::bail!("Symbol or File not found: {}", symbol);
                }
            }

            // Step B: Traverse Graph
            let walker = GraphWalker::new(
                &indexer.index, 
                &indexer.reverse_graph, 
                TraversalMode::Impact
            );
            let mut related_ids = walker.walk_deep(&start_ids);

            // Step C: Filter
            if *no_tests {
                related_ids.retain(|&id| {
                    indexer.index.symbols.get(&id).map_or(true, |s| !s.is_test)
                });
            }

            // Step D: Output
            let output = generate_context_output(&indexer.index, &related_ids);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }

        // --- QUERY: FIND (Fuzzy Search) ---
        Commands::Find { query, limit } => {
            let indexer = &manager.indexer;
            let mut matcher = Matcher::new(Config::DEFAULT);
            let mut results = Vec::new();
            let query_utf32 = Utf32String::from(query.as_str());

            // Match Symbols
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

            // Match Files
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
            results.truncate(*limit);

            println!("{}", serde_json::to_string_pretty(&results)?);
        }
    }

    Ok(())
}