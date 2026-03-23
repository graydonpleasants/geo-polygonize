pub mod docs;

use std::env;
use std::fs;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 || args[1] != "docs" {
        println!("Usage:");
        println!("  cargo run -p xtask -- docs extract");
        println!("  cargo run -p xtask -- docs render");
        return Ok(());
    }

    match args[2].as_str() {
        "extract" => {
            println!("Extracting docs metadata...");
            let meta = docs::extract_docs()?;
            fs::create_dir_all("docs/reference/generated")?;
            fs::write(
                "docs/reference/generated/metadata.json",
                serde_json::to_string_pretty(&meta)?,
            )?;
            println!("Wrote docs/reference/generated/metadata.json");
        }
        "render" => {
            println!("Rendering markdown from metadata...");
            let content = fs::read_to_string("docs/reference/generated/metadata.json")?;
            let meta: docs::DocsMetadata = serde_json::from_str(&content)?;
            docs::render_markdown::render(&meta)?;
            println!("Rendered markdown to docs/reference");
        }
        _ => {
            println!("Unknown command: {}", args[2]);
        }
    }

    Ok(())
}
