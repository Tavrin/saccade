//! Embed every shipped schema, including in registry crate builds.
use std::{env, error::Error, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=schemas");
    let mut paths = fs::read_dir("schemas")?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut source = String::from("&[\n");
    for path in paths {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(id) = name.strip_suffix(".schema.json") {
            source.push_str(&format!("({id:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/schemas/{name}\"))),\n"));
        }
    }
    source.push_str("]\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR")?).join("schema_catalog.rs"),
        source,
    )?;
    Ok(())
}
