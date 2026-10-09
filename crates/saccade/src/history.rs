//! Opt-in, local history of comparable report measurements.
use crate::agent::CliError;
use clap::{Args, Subcommand};
use saccade_core::workflows::history::{self, Trial};
use std::path::{Path, PathBuf};
#[derive(Args)]
pub(crate) struct HistoryArgs {
    #[command(subcommand)]
    operation: HistoryOperation,
}

#[derive(Subcommand)]
enum HistoryOperation {
    /// Find candidate performance onsets in qualified, comparable history observations.
    Onset {
        #[arg(long)]
        store: PathBuf,
        /// Most recent distinct observations per partition; exact DP is bounded to 120.
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u16).range(10..=120))]
        limit: u16,
        #[arg(long)]
        json: bool,
    },
    /// Add one existing comparison report to the local history store.
    Record {
        report: PathBuf,
        /// Producer-assigned independent capture run, never an image or report hash.
        #[arg(long, requires = "environment_id")]
        run_id: Option<String>,
        /// Frozen browser/device, fonts, viewport, warmup and temporal protocol identity.
        #[arg(long, requires = "run_id")]
        environment_id: Option<String>,
        /// Declare an unchanged-build repeat eligible for normal-variation advice.
        #[arg(long, requires = "run_id")]
        unchanged_build: bool,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show measured variation and threshold advice for comparable entries.
    Analyze {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        entry: Option<String>,
        /// Diagnose sustained anchor-relative drift in recorded run order.
        #[arg(long)]
        drift: bool,
        /// New file containing the complete witness for the selected groups.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Maximum runs to list, 1-20 (default 10).
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=20))]
        limit: u8,
        #[arg(long)]
        json: bool,
    },
}

fn run_onset(store: &Path, limit: usize, json_output: bool) -> Result<u8, CliError> {
    let value = history::analyze_onset(store, limit)?.value;
    if json_output {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        let mut text =
            String::from("Performance onset candidates (fresh qualified repeats required):\n");
        let mut count = 0;
        for partition in value["partitions"].as_array().into_iter().flatten() {
            if partition["status"] == "insufficient_history" {
                text.push_str(&format!(
                    "{}: insufficient qualified history\n",
                    partition["identity"]
                ));
            }
            for candidate in partition["analysis"]["evidence"]["candidates"]
                .as_array()
                .into_iter()
                .flatten()
            {
                count += 1;
                text.push_str(&format!("candidate onset at run {} / commit {}; interval after run {} / commit {}; effect {} ms ({}%).\n", candidate["first_changed"]["run"], candidate["first_changed"]["commit"], candidate["last_before"]["run"], candidate["last_before"]["commit"], candidate["effect_ms"], candidate["effect_pct"]));
            }
        }
        text.push_str(&format!("{count} candidates; {} excluded observations. Use --json for the numerical witness and exclusion reasons.\n", value["excluded_observations"].as_array().map_or(0, Vec::len)));
        crate::emit(&text)?;
    }
    Ok(0)
}

pub(crate) fn run(args: HistoryArgs) -> Result<u8, CliError> {
    let (value, json_output) = match args.operation {
        HistoryOperation::Onset { store, limit, json } => {
            return run_onset(&store, limit as usize, json);
        }
        HistoryOperation::Record {
            report,
            store,
            json,
            run_id,
            environment_id,
            unchanged_build,
        } => (
            serde_json::to_value(history::record_report(
                &report,
                &store,
                run_id
                    .zip(environment_id)
                    .map(|(run_id, environment_id)| Trial {
                        run_id,
                        environment_id,
                        unchanged_build,
                    }),
            )?)?,
            json,
        ),
        HistoryOperation::Analyze {
            store,
            entry,
            drift,
            out,
            limit,
            json,
        } => (
            history::analyze_history(&history::AnalyzeOptions {
                store: &store,
                entry: entry.as_deref(),
                drift,
                out: out.as_deref(),
                limit: limit as usize,
            })?
            .witness,
            json,
        ),
    };
    if json_output {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&history::AnalysisResult { witness: value }.preview()?)?
        ))?;
    } else if value["operation"] == "record" {
        crate::emit(&format!(
            "history: {} ({})\n",
            if value["recorded"] == true {
                "recorded"
            } else {
                "already recorded"
            },
            value["report_sha256"].as_str().unwrap_or("unknown")
        ))?;
    } else {
        let mut out = format!(
            "history: {} artifact groups, {} capture-run groups\n",
            value["groups"], value["run_analysis"]["groups"]
        );
        for item in value["run_analysis"]["entries"]
            .as_array()
            .into_iter()
            .flatten()
        {
            out.push_str(&format!(
                "{}: {} independent runs; drift {}; recommendation {}\n",
                item["entry"].as_str().unwrap_or("?"),
                item["independent_runs"],
                item["drift"].as_str().unwrap_or("?"),
                item["recommendation"].as_str().unwrap_or("?")
            ));
        }
        for item in value["entries"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "{}: {} ({} distinct captures, span {:.5}, threshold {:.5})\n",
                item["entry"].as_str().unwrap_or("?"),
                item["finding"].as_str().unwrap_or("?"),
                item["distinct_captures"].as_u64().unwrap_or(0),
                item["variation_span"].as_f64().unwrap_or(0.0),
                item["threshold"].as_f64().unwrap_or(0.0)
            ));
        }
        crate::emit(&out)?;
    }
    Ok(0)
}
