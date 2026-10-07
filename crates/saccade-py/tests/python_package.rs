//! Targeted development package proof through Cargo's integration-test cdylib build.
//! Release/manylinux wheel construction remains in the heavy gate.
#![cfg(target_os = "linux")]
#[test]
fn light_python_package() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(_native::VERSION, env!("CARGO_PKG_VERSION"));
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .ok_or("set CARGO_TARGET_DIR for native package proof")?;
    // Integration targets rebuild the dependency here; a top-level library may be stale.
    let lib = target.join("debug/deps/lib_native.so");
    if !lib.is_file() {
        return Err("Cargo did not produce the integration-test native cdylib".into());
    }
    let package = tempfile::tempdir()?;
    let dir = package.path().join("saccade");
    std::fs::create_dir(&dir)?;
    std::fs::copy(&lib, dir.join("_native.abi3.so"))?;
    for name in ["__init__.py", "__init__.pyi", "py.typed"] {
        std::fs::copy(crate_dir.join("python/saccade").join(name), dir.join(name))?;
    }
    let python = std::env::var_os("SACCADE_W8_PYTHON").unwrap_or_else(|| "python3".into());
    let output = std::process::Command::new(python)
        .args(["-m", "pytest", "-q", "tests/test_light.py"])
        .current_dir(&crate_dir)
        .env("PYTHONPATH", package.path())
        .env_remove("SACCADE_BIN")
        .output()?;
    assert!(
        output.status.success(),
        "light pytest failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
