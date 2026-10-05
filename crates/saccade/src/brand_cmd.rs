//! Unified brand/theme/accessibility front door.
use crate::agent::CliError;
use saccade_core::{brand, evidence::canonical::Digest};
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct Args {
    /// Capture-bound source JSON, exported by the colour/DOM/layout producer.
    source: PathBuf,
    /// Project swatches, profiles and CVD tolerances.
    #[arg(long, default_value = "saccade.toml")]
    config: PathBuf,
    /// New review packet JSON file.
    #[arg(long)]
    out: PathBuf,
}
pub(crate) fn run(args: Args, json: bool) -> Result<u8, CliError> {
    let cfg = saccade_core::config::RunConfig::from_toml_file(&args.config)?;
    let source: brand::Evidence = crate::parse_contract(
        &std::fs::read(&args.source).map_err(|e| CliError::io(e.to_string()))?,
        "saccade-brand-source.v1",
    )?;
    let path = args
        .source
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(&source.source_artifact);
    let bytes = std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?;
    if Digest::of_bytes(&bytes) != source.source_sha256 {
        return Err(CliError::usage("brand source artifact hash mismatch"));
    }
    let report = brand::review(
        &cfg.brand,
        &source,
        cfg.config_dir
            .as_deref()
            .unwrap_or(std::path::Path::new(".")),
    )?;
    crate::local_cmd::write_value(&args.out, &serde_json::to_value(&report)?)?;
    let changed = report.findings.iter().any(|f| {
        f.status == "fail"
            || f.status == "review"
            || (f.status == "unavailable" && f.rule != "apca_wcag_3_draft")
    });
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    } else {
        crate::emit(&format!(
            "brand review: {} findings; {}\n",
            report.findings.len(),
            args.out.display()
        ))?;
    }
    Ok(u8::from(changed))
}
