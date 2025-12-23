use clap::Parser;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

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
    println!("Target: {:?}", cli.path);
    
    // Define the index file path (e.g., inside the repo or a global cache)
    // For now, we put it in the root of the scanned folder for simplicity
    let index_path = cli.path.join(".index");
    
    // 1. Load or Initialize Indexer
    let start_load = Instant::now();
    let mut indexer = match Indexer::load_from_file(&index_path) {
        Ok(idx) => {
            println!("Index loaded in {:.2?}", start_load.elapsed());
            idx
        },
        Err(e) => {
            eprintln!("Failed to load index (starting fresh): {}", e);
            Indexer::new()
        }
    };
    
    // 2. Scan (Incremental)
    // If loaded, this only parses files where the hash changed.
    let start_scan = Instant::now();
    indexer.scan(&cli.path);
    println!("Scan complete in {:.2?}", start_scan.elapsed());

    // 3. Save Index (Persistence)
    if let Err(e) = indexer.save(&index_path) {
        eprintln!("Warning: Failed to save index: {}", e);
    } else {
        println!("Index saved to {:?}", index_path);
    }
    
    // 4. Export & Logic
    let graph = indexer.export_graph();
    println!("Graph contains {} functions.", graph.len());

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