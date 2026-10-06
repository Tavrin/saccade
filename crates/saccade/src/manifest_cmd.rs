//! `saccade manifest`: find, link and re-check the outputs of a report directory.
use crate::agent::CliError;
use crate::local_cmd::{base_result, reference};
use saccade_core::manifest::{self, Anchors};
use serde_json::json;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(clap::Subcommand)]
enum Operation {
    /// Write saccade-manifest.json for a report directory: artifacts by hash, reports by
    /// report_id, duplicates listed once. Passing is never recorded as approval.
    Build {
        /// The output directory to describe.
        dir: PathBuf,
        /// Record a baseline a human approved (separate from last-good).
        #[arg(long)]
        approved_anchor: Option<PathBuf>,
        /// Record the last passing run (a different role; not approval).
        #[arg(long)]
        last_good: Option<PathBuf>,
        /// Print a JSON result.
        #[arg(long)]
        json: bool,
    },
    /// Re-hash everything a manifest or link names; fails with `link_missing` or
    /// `stale_link` when a recorded file moved or changed.
    Verify {
        /// A report directory, a saccade-manifest.json or a saccade-link.json.
        target: PathBuf,
        /// Print a JSON result.
        #[arg(long)]
        json: bool,
    },
    /// Write a stable link to one report of a directory, by report_id.
    Link {
        /// A report directory that has a manifest.
        dir: PathBuf,
        /// The report_id to link (see `reports` in the manifest).
        #[arg(long)]
        report_id: String,
        /// Where to write the link document.
        #[arg(long)]
        out: PathBuf,
        /// Print a JSON result.
        #[arg(long)]
        json: bool,
    },
    /// Say whether a path is a report directory, a JSON document or an API response.
    Classify {
        /// Path to inspect.
        path: PathBuf,
        /// Print JSON (the default output is already one line of JSON).
        #[arg(long)]
        json: bool,
    },
}

pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    match args.operation {
        Operation::Build {
            dir,
            approved_anchor,
            last_good,
            json,
        } => {
            let (path, value) = manifest::write(
                &dir,
                Anchors {
                    approved: approved_anchor.as_deref(),
                    last_good: last_good.as_deref(),
                },
            )?;
            let mut result = base_result("manifest.build");
            result["artifact"] = reference(&path)?;
            result["counts"] = value["counts"].clone();
            result["data"] = json!({"approval":value["approval"]["state"]});
            if json {
                crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
            } else {
                crate::emit(&format!(
                    "wrote {} ({} artifacts, {} reports, {} duplicate files; approval: {})\n",
                    crate::escape_control(&path.display().to_string()),
                    value["counts"]["unique_artifacts"],
                    value["counts"]["reports"],
                    value["counts"]["duplicate_files"],
                    value["approval"]["state"].as_str().unwrap_or("none"),
                ))?;
            }
            Ok(0)
        }
        Operation::Verify { target, json } => {
            let findings = manifest::verify(&target)?;
            if !findings.is_empty() {
                let code = if findings
                    .iter()
                    .any(|f| f["code"] == manifest::CODE_STALE_LINK)
                {
                    "stale_link"
                } else {
                    "link_missing"
                };
                let listed = findings
                    .iter()
                    .take(5)
                    .filter_map(|f| f["message"].as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                let mut error = CliError::new(
                    code,
                    format!("{} link failure(s): {listed}", findings.len()),
                );
                error.hint = "rebuild the manifest with `saccade manifest build` and re-link, or restore the recorded file".into();
                return Err(error);
            }
            let mut result = base_result("manifest.verify");
            result["data"] = json!({"verified":true});
            if json {
                crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
            } else {
                crate::emit(
                    "verified: every recorded artifact is present with its recorded bytes\n",
                )?;
            }
            Ok(0)
        }
        Operation::Link {
            dir,
            report_id,
            out,
            json,
        } => {
            manifest::link(&dir, &report_id, &out)?;
            let mut result = base_result("manifest.link");
            result["artifact"] = reference(&out)?;
            if json {
                crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
            } else {
                crate::emit(&format!(
                    "wrote {}\n",
                    crate::escape_control(&out.display().to_string())
                ))?;
            }
            Ok(0)
        }
        Operation::Classify { path, json: _ } => {
            let value = manifest::classify(&path)?;
            crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
            Ok(0)
        }
    }
}
