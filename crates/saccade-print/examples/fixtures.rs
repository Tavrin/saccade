//! Generate original synthetic CMYK/ICC fixtures; no downloaded profiles.
#[path = "../tests/support/mod.rs"]
mod support;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: fixtures OUTPUT_DIRECTORY")?;
    let dir = std::path::Path::new(&path);
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("synthetic.icc"), support::icc())?;
    for name in ["packaging-label", "magazine-page"] {
        support::fixture(dir, name);
    }
    image::RgbImage::new(8, 8).save(dir.join("rgb.png"))?;
    Ok(())
}
