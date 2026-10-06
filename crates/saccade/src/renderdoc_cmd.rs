//! Aligns hash-checked optional replay worker evidence; never runs replay implicitly.
use crate::agent::CliError;
use saccade_core::renderdoc::Capture;
use sha2::{Digest, Sha256};
use std::io::Read;
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
fn bounded_regular_read(path: &Path, budget: u64) -> Result<Vec<u8>, CliError> {
    if !std::fs::metadata(path)
        .map_err(|e| CliError::io(e.to_string()))?
        .is_file()
    {
        return Err(CliError::usage("RenderDoc input must be a regular file"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|e| CliError::io(e.to_string()))?;
    let metadata = file.metadata().map_err(|e| CliError::io(e.to_string()))?;
    if !metadata.is_file() {
        return Err(CliError::usage("RenderDoc input must be a regular file"));
    }
    if metadata.len() > budget {
        return Err(CliError::usage("RenderDoc input byte budget exceeded"));
    }
    let mut data = Vec::new();
    file.take(budget + 1)
        .read_to_end(&mut data)
        .map_err(|e| CliError::io(e.to_string()))?;
    if data.len() as u64 > budget {
        return Err(CliError::usage("RenderDoc input byte budget exceeded"));
    }
    Ok(data)
}
fn read(path: &Path) -> Result<(Capture, String), CliError> {
    let bytes = bounded_regular_read(path, 16 * 1024 * 1024)?;
    let capture: Capture = crate::parse_contract(&bytes, "saccade-renderdoc-extract.v1")?;
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut remaining = 2 * 1024 * 1024 * 1024u64;
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
        let data = bounded_regular_read(&file, remaining.min(64 * 1024 * 1024))?;
        remaining -= data.len() as u64;
        let len = data.len() as u64;
        let hash = format!("{:x}", Sha256::digest(&data));
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
        .open(&args.out)
        .map_err(|e| CliError::io(e.to_string()))?;
    let linked = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
    serde_json::to_writer_pretty(file, &linked)?;
    saccade_core::report_links::index(&args.out, &linked)?;
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
