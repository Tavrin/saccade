//! Dense-flow review and pinned renderer-vector diagnostics.
use crate::agent::CliError;
use saccade_core::{dense_motion, localized};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    reference: PathBuf,
    candidate: PathBuf,
    /// Row-major saccade-vector-buffer.v1 JSON (requires --sidecar).
    #[arg(long, requires = "sidecar")]
    vectors: Option<PathBuf>,
    /// Pinned units, direction, origin and jitter contract (requires --vectors).
    #[arg(long, requires = "vectors")]
    sidecar: Option<PathBuf>,
    /// Viewing condition in pixels per degree of visual angle (default 67).
    #[arg(long, default_value_t = 67.0)]
    ppd: f32,
    /// Raw full-frame mean FLIP threshold; motion cannot relax it.
    #[arg(long, default_value_t = 0.01)]
    maximum_raw_mean: f32,
    #[arg(long)]
    out: PathBuf,
}
fn bytes(path: &Path) -> Result<Vec<u8>, CliError> {
    let meta = std::fs::metadata(path).map_err(|e| CliError::io(e.to_string()))?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err(CliError::usage(
            "motion inputs must be regular files <=64 MiB",
        ));
    }
    std::fs::read(path).map_err(|e| CliError::io(e.to_string()))
}
pub(crate) fn run(args: Args, json: bool) -> Result<u8, CliError> {
    if !cfg!(feature = "dense-motion") {
        return Err(CliError::new(
            "feature_unavailable",
            "motion review requires dense-motion",
        ));
    }
    let before = bytes(&args.reference)?;
    let after = bytes(&args.candidate)?;
    let decode = |b: &[u8]| -> Result<image::RgbaImage, CliError> {
        let image = image::load_from_memory(b).map_err(|e| CliError::usage(e.to_string()))?;
        if !matches!(
            image.color(),
            image::ColorType::Rgb8
                | image::ColorType::Rgba8
                | image::ColorType::L8
                | image::ColorType::La8
        ) {
            return Err(CliError::usage(
                "motion review supports encoded 8-bit SDR captures only",
            ));
        }
        Ok(image.to_rgba8())
    };
    let reference = decode(&before)?;
    let candidate = decode(&after)?;
    let supplied: Option<(dense_motion::Sidecar, dense_motion::Buffer, String)> =
        match (&args.sidecar, &args.vectors) {
            (Some(s), Some(v)) => {
                let b = bytes(v)?;
                Some((
                    crate::parse_contract(&bytes(s)?, "saccade-motion-vectors.v1")?,
                    crate::parse_contract(&b, "saccade-vector-buffer.v1")?,
                    localized::digest(&b),
                ))
            }
            (None, None) => None,
            _ => {
                return Err(CliError::usage(
                    "motion vectors and sidecar must be supplied together",
                ));
            }
        };
    let report = dense_motion::review(
        &reference,
        &candidate,
        [localized::digest(&before), localized::digest(&after)],
        supplied.as_ref().map(|(s, b, d)| (s, b, d.as_str())),
        args.ppd,
        args.maximum_raw_mean,
    )?;
    crate::local_cmd::write_value(&args.out, &serde_json::to_value(&report)?)?;
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    } else {
        crate::emit(&format!(
            "Raw FLIP mean {:.6}; regression: {}. Dense correspondence support: {} / {} reference pixels. {}\n",
            report.raw_flip.mean,
            report.raw_regression,
            report.fields[0].qualified_pixels,
            report.fields[0].vectors.len(),
            args.out.display()
        ))?;
    }
    Ok(u8::from(report.raw_regression))
}
