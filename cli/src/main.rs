use clap::Parser;
use std::path::PathBuf;
use std::process;

// Import from our engine library
use rfc_engine::analyzer::{build_codebase_graph, find_call_chain, generate_context};
use rfc_engine::language::get_language_configs;

/// A tool to build an LLM context based on a function's call chain.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The path to the directory of the code repository.
    #[arg(short, long)]
    path: PathBuf,

    /// The name of the target function.
    #[arg(short, long)]
    function_name: String,

    /// Include documentation comments in the context.
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
    
    // 1. Get configurations
    let configs = get_language_configs();
    
    // 2. Build the graph (Using the logic from analyzer.rs)
    println!("Building codebase graph...");
    let graph = build_codebase_graph(&cli.path, &configs);
    println!("Graph built. Found {} functions.", graph.len());

    // 3. Find the chain
    println!("Finding call chain for `{}`...", cli.function_name);
    if let Some(chain) = find_call_chain(&graph, &cli.function_name) {
        println!("Call chain found: {}", chain.join(" -> "));
        
        // 4. Generate output
        let context = generate_context(&graph, &chain, cli.include_docs);
        println!("\n--- Generated Context ---\n");
        println!("{}", context);
    } else {
        eprintln!("Error: Could not find function `{}` or its call chain.", cli.function_name);
        process::exit(1);
    }
}