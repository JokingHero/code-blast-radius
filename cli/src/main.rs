use clap::Parser;
use std::path::PathBuf;
use std::process;
use rfc_engine::indexer::Indexer;
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
    
    // We keep this because tests can pollute context, 
    // but the Agent might explicitly want them.
    #[arg(long, default_value_t = true)]
    no_tests: bool,
}

fn main() {
    let cli = Cli::parse();
    // ... path checking ...

    let index_path = cli.path.join(".index");
    let mut indexer = Indexer::load_from_file(&index_path).unwrap_or_else(|_| Indexer::new());
    
    indexer.scan(&cli.path);
    indexer.resolve_references();
    let _ = indexer.save(&index_path);

    // ... impact analysis ...

    // --- Context Generation ---
    if let Some(func_name) = cli.function_name {
        // Find Symbols
        if let Some(mut symbol_ids) = find_related_symbols(&indexer.index, &func_name) {
            
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
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            // Valid JSON error for Agent
            println!("{}", serde_json::json!({ "error": "Symbol not found" }));
            process::exit(1);
        }
    }
}