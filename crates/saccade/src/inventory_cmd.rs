//! Capture inventory adapter for generic comparison layouts.
use crate::agent::CliError;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Expected suite and supplied capture attempts, with stable case IDs.
    #[arg(long)]
    manifest: PathBuf,
    /// Existing comparison report.
    #[arg(long)]
    report: PathBuf,
    /// New inventory JSON file.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}

pub(crate) fn value(
    manifest: &saccade_core::inventory::Manifest,
    report: &saccade_core::Report,
    manifest_bytes: &[u8],
    report_bytes: &[u8],
) -> Result<saccade_core::inventory::Inventory, CliError> {
    let mut inventory = saccade_core::inventory::reconcile(manifest, report)?;
    inventory.manifest_sha256 = format!("{:x}", Sha256::digest(manifest_bytes));
    inventory.report_sha256 = format!("{:x}", Sha256::digest(report_bytes));
    Ok(inventory)
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let manifest_bytes = std::fs::read(&args.manifest).map_err(|e| CliError::io(e.to_string()))?;
    let manifest = crate::parse_contract(&manifest_bytes, "saccade-inventory.v1")?;
    let report_bytes = std::fs::read(&args.report).map_err(|e| CliError::io(e.to_string()))?;
    let report = crate::parse_contract(&report_bytes, "saccade-report.v1")?;
    let inventory = value(&manifest, &report, &manifest_bytes, &report_bytes)?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.out)
        .map_err(|e| CliError::io(e.to_string()))?;
    serde_json::to_writer_pretty(file, &inventory)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&inventory)?))?;
    } else {
        crate::emit(&format!(
            "inventory: {}; {} expected, {} supplied, {} compared\n",
            inventory.coverage, inventory.expected, inventory.supplied, inventory.compared
        ))?;
    }
    Ok(u8::from(inventory.coverage != "complete"))
}
