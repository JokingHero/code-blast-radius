use clap::Parser;
use std::path::PathBuf;
use std::process;

// Use the new Indexer instead of analyzer directly
use rfc_engine::indexer::Indexer;
use rfc_engine::analyzer::{find_call_chain, generate_context};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    path: PathBuf,

    #[arg(short, long)]
    function_name: String,

    #[arg(long, default_value_t = false)]
    include_docs: bool,
}

fn main() {
    let cli = Cli::parse();

    if !cli.path.is_dir() {
        eprintln!("Error: Provided path is not a directory: {:?}", cli.path);
        process::exit(1);
    }

    println!("--- Reverse Flow Context ---");
    println!("Scanning: {:?}", cli.path);
    
    // 1. Initialize Indexer
    let mut indexer = Indexer::new();
    
    // 2. Scan (this hashes files and parses only what is needed)
    indexer.scan(&cli.path);
    
    // 3. Export to the simple graph format for now (Bridge to old logic)
    let graph = indexer.export_graph();
    
    println!("Graph built. Found {} functions.", graph.len());

    // 4. Find the chain
    println!("Finding call chain for `{}`...", cli.function_name);
    if let Some(chain) = find_call_chain(&graph, &cli.function_name) {
        println!("Call chain found: {}", chain.join(" -> "));
        
        let context = generate_context(&graph, &chain, cli.include_docs);
        println!("\n--- Generated Context ---\n");
        println!("{}", context);
    } else {
        eprintln!("Error: Could not find function `{}` or its call chain.", cli.function_name);
        process::exit(1);
    }
}