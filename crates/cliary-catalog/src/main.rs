use anyhow::{Result, bail};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new("catalog");
    match args.get(1).map(String::as_str) {
        Some("validate") => {
            let (tools, _, _) = cliary_catalog::load_sources(root)?;
            println!("validated {} tools", tools.len());
        }
        Some("build") => {
            let version = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(1);
            let output = args.get(2).map(String::as_str).unwrap_or("dist/catalog");
            let manifest = cliary_catalog::publish(root, Path::new(output), version)?;
            println!(
                "catalog version {}: {} tools",
                manifest.version, manifest.tool_count
            );
        }
        _ => bail!("usage: cliary-catalog-build validate | build [output-dir] [version]"),
    }
    Ok(())
}
