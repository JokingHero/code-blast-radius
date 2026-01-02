use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use anyhow::{Context, Result};
use blast_radius_engine::resolution::{Indexer, pipeline::Pipeline};
use blast_radius_engine::query::traversal::find_related_symbols;
use blast_radius_engine::query::output::generate_context_output;
use nucleo_matcher::{Matcher, Config, Utf32String};

#[derive(Parser, Debug)]
#[command(name = "cblast", version, about = "Code Blast Radius - Analyze code dependencies and impact")]
struct Cli {
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

    if !cli.path.exists() {
        anyhow::bail!("Path does not exist: {:?}", cli.path);
    }

    let index_path = cli.path.join(".index");
    let mut indexer = Indexer::load_from_file(&index_path).unwrap_or_else(|_| Indexer::new());
    
    // Use the new Pipeline orchestrator
    let mut pipeline = Pipeline::new();
    pipeline.run(&mut indexer, &cli.path);
    
    indexer.save(&index_path).context("Failed to save index")?;

    match cli.command {
        Commands::Radius { function_name, no_tests, impact: _ } => {
            // Find Symbols: Pass components explicitly
            let symbol_ids = find_related_symbols(
                &indexer.index,
                &indexer.lookup,
                &indexer.reverse_graph,
                &function_name
            ).ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", function_name))?;
            
            let mut symbol_ids = symbol_ids;

            // Apply filtering here in main before generating output
            if no_tests {
                 symbol_ids.retain(|&id| {
                    if let Some(sym) = indexer.index.symbols.get(&id) {
                        !sym.is_test
                    } else {
                        true
                    }
                 });
            }

            // Generate Output
            let output = generate_context_output(&indexer.index, &symbol_ids);

            // Print JSON
            println!("{}", serde_json::to_string_pretty(&output).context("Failed to serialize output")?);
        }
        Commands::Find { query, limit } => {
            let mut matcher = Matcher::new(Config::DEFAULT);
            let mut results = Vec::new();
            let query_utf32 = Utf32String::from(query.as_str());

            // Symbols
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

            // Files
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
    }

    Ok(())
}