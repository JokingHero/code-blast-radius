use clap::Parser;
use std::path::PathBuf;
use std::process;
use anyhow::{Context, Result};
use rfc_engine::resolution::Indexer;
use rfc_engine::models::StagingArea;
use rfc_engine::query::traversal::find_related_symbols;
use rfc_engine::query::output::generate_context_output;

#[derive(Parser, Debug)]
#[command(name = "cfb", version, about = "Context Management")]
struct Cli {
    #[arg(short, long)]
    path: PathBuf,

    #[arg(short, long)]
    function_name: Option<String>,

    #[arg(long)]
    impact: Option<PathBuf>,
    
    #[arg(long, default_value_t = true)]
    no_tests: bool,
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
    
    // Create the transient staging area
    let mut staging = StagingArea::default();
    
    // Pass it to scan
    indexer.scan(&cli.path, &mut staging);
    
    // Pass it to resolution
    indexer.resolve_references(&mut staging);
    
    indexer.save(&index_path).context("Failed to save index")?;

    // --- Context Generation ---
    if let Some(func_name) = cli.function_name {
        // Find Symbols: Pass components explicitly
        let symbol_ids = find_related_symbols(
            &indexer.index,
            &indexer.lookup,
            &indexer.reverse_graph,
            &func_name
        ).ok_or_else(|| anyhow::anyhow!("Symbol not found: {}", func_name))?;
        
        let mut symbol_ids = symbol_ids;

        // Apply filtering here in main before generating output
        if cli.no_tests {
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

    Ok(())
}