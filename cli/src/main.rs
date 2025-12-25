use clap::Parser;
use std::path::PathBuf;
use std::process;
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::{find_related_symbols, generate_context_from_ids};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(short, long)]
    path: PathBuf,

    /// Name of function to get context for
    #[arg(short, long)]
    function_name: Option<String>, // Made optional

    /// File path to perform impact analysis on
    #[arg(long)]
    impact: Option<PathBuf>,

    #[arg(long, default_value_t = true)]
    include_docs: bool,
}

fn main() {
    let cli = Cli::parse();

    if !cli.path.is_dir() {
        eprintln!("Error: Path is not a directory: {:?}", cli.path);
        process::exit(1);
    }

    let index_path = cli.path.join(".index");
    let mut indexer = Indexer::load_from_file(&index_path).unwrap_or_else(|_| Indexer::new());
    
    indexer.scan(&cli.path);
    indexer.resolve_references();
    let _ = indexer.save(&index_path);
    
    // --- Impact Analysis Mode ---
    if let Some(target_file) = cli.impact {
        println!("Analyzing impact for file: {:?}", target_file);
        let impacted = indexer.get_impacted_files(&target_file);
        
        if impacted.is_empty() {
            println!("No direct downstream impact found (no other files import this file).");
        } else {
            println!("\n--- IMPACTED FILES (Dependents) ---");
            for f in impacted {
                println!(" - {}", f);
            }
        }
        return; // Exit after impact analysis
    }

    // --- Context Generation Mode ---
    if let Some(func_name) = cli.function_name {
        println!("Extracting full semantic context for `{}`...", func_name);
        if let Some(symbol_ids) = find_related_symbols(&indexer.index, &func_name) {
            let context = generate_context_from_ids(&indexer.index, &symbol_ids, cli.include_docs);
            println!("\n--- SEMANTIC CONTEXT ---\n");
            println!("{}", context);
        } else {
            eprintln!("Error: Could not find symbol `{}`.", func_name);
            process::exit(1);
        }
    } else {
        eprintln!("Error: Please provide either --function-name or --impact.");
        process::exit(1);
    }
}