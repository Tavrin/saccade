//! Aligns hash-checked optional replay worker evidence; never runs replay implicitly.
use crate::agent::CliError;
use saccade_core::renderdoc::Capture;
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    /// Baseline worker extraction.json; raw payloads must stay beneath its directory.
    baseline: PathBuf,
    candidate: PathBuf,
    /// New JSON localization report.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
fn read(path: &Path) -> Result<(Capture, String), CliError> {
    let bytes = std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?;
    let capture: Capture = serde_json::from_slice(&bytes)?;
    let root = path.parent().unwrap_or(Path::new("."));
    for resource in capture
        .actions
        .iter()
        .flat_map(|a| &a.resources)
        .filter(|r| r.error.is_none())
    {
        let relative = resource
            .payload
            .as_ref()
            .ok_or_else(|| CliError::usage("resource payload missing"))?;
        let file = crate::ingest::contained_source(root, Path::new(relative))?;
        let len = std::fs::metadata(&file)
            .map_err(|e| CliError::io(e.to_string()))?
            .len();
        let hash = saccade_core::run::sha256_file(&file)?;
        if resource.bytes != Some(len) || resource.sha256.as_deref() != Some(hash.as_str()) {
            return Err(CliError::usage(
                "replay raw payload hash/size differs from extraction evidence",
            ));
        }
    }
    Ok((capture, saccade_core::localized::digest(&bytes)))
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let (baseline, bhash) = read(&args.baseline)?;
    let (candidate, chash) = read(&args.candidate)?;
    let mut report = saccade_core::renderdoc::localize(&baseline, &candidate)?;
    report.extraction_sha256 = [bhash, chash];
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(args.out)
        .map_err(|e| CliError::io(e.to_string()))?;
    serde_json::to_writer_pretty(file, &report)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&report)?))?;
    } else {
        crate::emit(&format!(
            "RenderDoc localization: {} coverage, {} aligned observations; first observed divergence {}; root cause unproven\n",
            report.coverage,
            report.observations.len(),
            report
                .first_observed_divergence
                .as_ref()
                .map(|o| format!(
                    "event {} -> {}, role {}",
                    o.baseline_event, o.candidate_event, o.role
                ))
                .unwrap_or_else(|| "unresolved/none measured".into())
        ))?;
    }
    Ok(if report.coverage != "complete" {
        2
    } else {
        u8::from(report.first_observed_divergence.is_some())
    })
}
