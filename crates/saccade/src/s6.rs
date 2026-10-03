//! Analyze divergence among existing captures.
use clap::Args;
#[cfg(feature = "graphics")]
use saccade_core::bisect::{self, BisectOptions};
use std::path::PathBuf;

use crate::agent::CliError;

#[derive(Args)]
#[cfg(feature = "graphics")]
pub(crate) struct BisectArgs {
    /// Ordered run directories, oldest first (repeatable).
    #[arg(long, num_args = 1..)]
    pub runs: Vec<PathBuf>,
    /// One ordered run path per line.
    #[arg(long)]
    pub runs_from: Option<PathBuf>,
    /// Reference for existing runs (default: first run).
    #[arg(long)]
    pub good: Option<PathBuf>,
    /// Explicit FLIP threshold relaxes native sample identity.
    #[arg(long)]
    pub threshold: Option<f64>,
    /// mean, p95, p99 or max (default max).
    #[arg(long)]
    pub metric: Option<String>,
    /// Select image names by glob.
    #[arg(long)]
    pub entries: Option<String>,
    /// Report directory, separate from inputs.
    #[arg(long, default_value = "bisect-report")]
    pub out: PathBuf,
    /// Print saccade-bisect.v1 JSON.
    #[arg(long)]
    pub json: bool,
}

#[cfg(feature = "graphics")]
pub(crate) fn options(
    threshold: Option<f64>,
    metric: Option<&str>,
    entries: Option<String>,
) -> Result<BisectOptions, CliError> {
    let metric = metric
        .map(|m| match m {
            "mean" => Ok(saccade_core::Metric::Mean),
            "p95" => Ok(saccade_core::Metric::P95),
            "p99" => Ok(saccade_core::Metric::P99),
            "max" => Ok(saccade_core::Metric::Max),
            _ => Err(CliError::usage("metric must be mean, p95, p99 or max")),
        })
        .transpose()?;
    if threshold.is_some_and(|t| !t.is_finite() || !(0.0..=1.0).contains(&t)) {
        return Err(CliError::usage(
            "threshold must be finite and between 0 and 1",
        ));
    }
    Ok(BisectOptions {
        threshold,
        metric,
        entries,
    })
}

#[cfg(feature = "graphics")]
pub(crate) fn bisect(args: BisectArgs) -> Result<u8, CliError> {
    let opts = options(args.threshold, args.metric.as_deref(), args.entries)?;
    let result = {
        let mut runs = args.runs;
        if let Some(file) = args.runs_from {
            if !runs.is_empty() {
                return Err(CliError::usage("use --runs or --runs-from"));
            }
            runs = std::fs::read_to_string(file)
                .map_err(|e| CliError::io(format!("reading runs file: {e}")))?
                .lines()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .collect();
        }
        bisect::runs(&runs, args.good.as_deref(), &args.out, &opts)?
    };
    if args.json {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&crate::local_cmd::analysis_result(
                &serde_json::to_value(&result)?,
                &args.out
            )?)?
        ))?;
    } else {
        crate::emit(&result.text())?;
    }
    Ok(result.exit_code())
}
