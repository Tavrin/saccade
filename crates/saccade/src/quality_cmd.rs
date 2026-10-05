//! External encoder quality sweep adapter.
use crate::agent::CliError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Frozen sweep manifest. All artifacts must be beneath its directory.
    manifest: PathBuf,
    /// New JSON report file; existing files are preserved.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}

pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let bytes = std::fs::read(&args.manifest).map_err(|e| CliError::io(e.to_string()))?;
    let manifest: saccade_core::quality::Manifest =
        crate::parse_contract(&bytes, "saccade-quality-sweep.v1")?;
    let mut report = saccade_core::quality::sweep(
        args.manifest
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
        manifest,
    )?;
    report.manifest_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.out)
        .map_err(|e| CliError::io(e.to_string()))?;
    serde_json::to_writer_pretty(file, &report)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&report)?))?;
    } else {
        crate::emit(&format!(
            "quality sweep: {}; selected {}; human visual review pending\n",
            report.coverage,
            report.selected_candidate.as_deref().unwrap_or("none")
        ))?;
    }
    Ok(if report.coverage == "complete" { 0 } else { 1 })
}
