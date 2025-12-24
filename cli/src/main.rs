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

    #[arg(short, long)]
    function_name: String,

    #[arg(long, default_value_t = true)] // Default to true for better LLM context
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
    
    println!("Extracting full semantic context for `{}`...", cli.function_name);
    
    // Use the new bidirectional "related symbols" function
    if let Some(symbol_ids) = find_related_symbols(&indexer.index, &cli.function_name) {
        println!("Found {} related symbols across the workspace.", symbol_ids.len());
        
        let context = generate_context_from_ids(&indexer.index, &symbol_ids, cli.include_docs);
        println!("\n--- SEMANTIC CONTEXT ---\n");
        println!("{}", context);
    } else {
        eprintln!("Error: Could not find symbol `{}`.", cli.function_name);
        process::exit(1);
    }
}