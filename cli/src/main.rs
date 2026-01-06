use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
    Radius {
        /// Function name, Class name, or File path (relative)
        symbol: String,

        #[arg(long, default_value_t = true)]
        no_tests: bool,

        /// Max depth of graph traversal. If missing or 0, traversal is infinite.
        /// 1 = Immediate neighbors only.
        #[arg(short, long)]
        depth: Option<usize>,
    },
    /// Find symbols or files using fuzzy search
    Find {
        #[arg(short, long)]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// Manage Workspaces
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
    /// Refresh/Sync the workspace
    Sync,
}

#[derive(Subcommand, Debug)]
enum RecipeAction {
    List,
    Add { 
        #[arg(short, long)]
        json: String 
    },
    Remove { 
        name: String 
    },
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

fn run() -> Result<()> {
    let cli = Cli::parse();

    // 1. Initialize Manager based on input path type
    let mut manager = if cli.path.is_file() {
        // Assume it's a .cblast file
        WorkspaceManager::from_file(cli.path.clone())
            .context("Failed to load workspace file")?
    } else if cli.path.is_dir() {
        // Assume Ad-Hoc directory mode
        WorkspaceManager::new_in_memory(vec![cli.path.clone()])
            .context("Failed to initialize workspace from directory")?
    } else {
        anyhow::bail!("Path must be a valid directory or .cblast file");
    };

    // 2. SYNC (Incremental Update)
    manager.sync();
    
    // Auto-save index for next time (optimization)
    if manager.backing_file.is_some() {
        manager.save().context("Failed to save workspace state")?;
    }

    match &cli.command {
        // --- WORKSPACE MANAGEMENT ---
        Commands::Workspace { action } => {
            match action {
                WorkspaceAction::Init { name } => {
                    manager.config.name = name.clone();
                    if manager.backing_file.is_some() {
                        manager.save()?;
                        println!("Workspace config updated.");
                    } else {
                        println!("Initialized in-memory workspace '{}'. Use GUI to save.", name);
                    }
                }
                WorkspaceAction::Add { root } => {
                    manager.add_root(root.clone());
                    if manager.backing_file.is_some() {
                        manager.save()?;
                        println!("Added root. Total roots: {}", manager.config.roots.len());
                    } else {
                        println!("Added root to temporary session. Changes will be lost on exit.");
                    }
                }
                WorkspaceAction::Remove { root } => {
                    manager.remove_root(root.clone());
                    if manager.backing_file.is_some() {
                        manager.save()?;
                    }
                    println!("Removed root.");
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
                    if manager.backing_file.is_some() {
                        manager.save()?;
                        println!("Recipe '{}' saved.", name);
                    } else {
                        eprintln!("Warning: Adding recipe to ad-hoc session. It will be lost.");
                    }
                }
                RecipeAction::Remove { name } => {
                    if manager.config.recipes.remove(name).is_some() {
                        if manager.backing_file.is_some() {
                            manager.save()?;
                        }
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
                    
                    // Uses the XML method defined in the engine
                    println!("{}", output.to_xml());
                }
            }
        }

        // --- QUERY: RADIUS ---
        Commands::Radius { symbol, no_tests, depth } => {
            let indexer = &manager.indexer;
            
            // Step A: Check if input is a File Path
            let matching_file_id = indexer.index.files.values()
                .find(|f| f.path.ends_with(symbol) || f.path == *symbol)
                .map(|f| f.id);

            let mut start_ids = Vec::new();

            if let Some(fid) = matching_file_id {
                for sym in indexer.index.symbols.values() {
                    if sym.file_id == fid {
                        start_ids.push(sym.id);
                    }
                }
            } else {
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
                TraversalMode::Impact,
                *depth
            );
            
            let mut related_ids = walker.walk_deep(&start_ids);

            if *no_tests {
                related_ids.retain(|&id| {
                    indexer.index.symbols.get(&id).map_or(true, |s| !s.is_test)
                });
            }

            let output = generate_context_output(&indexer.index, &related_ids);
            println!("{}", serde_json::to_string_pretty(&output)?);
        }

        // --- QUERY: FIND ---
        Commands::Find { query, limit } => {
            let indexer = &manager.indexer;
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
            results.truncate(*limit);

            println!("{}", serde_json::to_string_pretty(&results)?);
        }
    }

    Ok(())
}