//! Stock-build critical region/string policy transport.
use crate::agent::CliError;
use saccade_core::{critical_text as ct, general::input, localized::digest};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    baseline: PathBuf,
    candidate: PathBuf,
    /// Frozen saccade-critical-text-policy.v1 region/string thresholds.
    #[arg(long)]
    policy: PathBuf,
    /// Image-bound baseline saccade-ui-source.v1 (works on stock builds).
    #[arg(long, requires = "b_source", conflicts_with = "ocr")]
    a_source: Option<PathBuf>,
    /// Image-bound candidate source; requires --a-source.
    #[arg(long, requires = "a_source", conflicts_with = "ocr")]
    b_source: Option<PathBuf>,
    /// Execute cached local OCR; never downloads (requires ocr feature/runtime/models).
    #[arg(long)]
    ocr: bool,
    /// Optional report directory, must be empty and outside inputs.
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let policy_bytes = input::bytes(&args.policy, 1 << 20)?;
    let policy: ct::Policy = serde_json::from_slice(&policy_bytes)?;
    let a = input::bytes(&args.baseline, input::MAX_BYTES)?;
    let b = input::bytes(&args.candidate, input::MAX_BYTES)?;
    let images = [input::decode(&a)?, input::decode(&b)?];
    let mut sources = [None, None];
    let mut source_hashes = [None, None];
    for (i, (path, bytes)) in [
        (args.a_source.as_deref(), &a),
        (args.b_source.as_deref(), &b),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(path) = path {
            let encoded = input::bytes(path, 16 << 20)?;
            source_hashes[i] = Some(digest(&encoded));
            sources[i] = Some(serde_json::from_slice(&encoded)?);
        } else if args.ocr {
            sources[i] = Some(crate::text_cmd::source(
                None,
                None,
                bytes,
                [images[i].width(), images[i].height()],
                false,
            )?);
        }
    }
    let report = ct::evaluate(
        [&images[0], &images[1]],
        [digest(&a), digest(&b)],
        sources,
        policy,
        digest(&policy_bytes),
        source_hashes,
    )?;
    let value = serde_json::to_value(&report)?;
    if let Some(out) = &args.out {
        let mut inputs = vec![
            args.baseline.as_path(),
            args.candidate.as_path(),
            args.policy.as_path(),
        ];
        inputs.extend(args.a_source.as_deref());
        inputs.extend(args.b_source.as_deref());
        crate::general_cmd::prepare_out(out, &inputs)?;
        crate::general_cmd::persist_document(&value, out)?;
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        crate::emit(&format!(
            "critical-text: {} ({} required regions)\n",
            value["state"].as_str().unwrap_or("insufficient_evidence"),
            report.regions.len()
        ))?;
    }
    Ok(match report.state {
        ct::State::Pass => 0,
        ct::State::Fail => 1,
        ct::State::InsufficientEvidence => 4,
    })
}
