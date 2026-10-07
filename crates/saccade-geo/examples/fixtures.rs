//! Generate original procedural raster and tile fixtures, without downloads.
#[path = "../tests/support/mod.rs"]
mod support;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("supply fixture directory")?;
    support::fixtures(std::path::Path::new(&path));
    Ok(())
}
