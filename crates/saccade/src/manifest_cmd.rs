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
        /// Explicit cases and axes (saccade-cases.v1); creates manifest v2.
        #[arg(long)]
        cases: Option<PathBuf>,
        /// Print a JSON result.
        #[arg(long)]
        json: bool,
    },
    /// Group declared variants, show missing cases and reference health in JSON and HTML.
    Views {
        /// A saccade-manifest.v2 file, or its report directory.
        target: PathBuf,
        /// Output directory for coverage.json and index.html.
        #[arg(long)]
        out: PathBuf,
        /// Declared axes to group by (default: all axes).
        #[arg(long, value_delimiter = ',')]
        group_by: Vec<String>,
        /// Observation time for reproducible health findings (default: current time).
        #[arg(long)]
        now_unix: Option<u64>,
        /// Age limit for recorded runs and known approval times.
        #[arg(long, default_value_t = 2_592_000)]
        max_age_seconds: u64,
        /// Print a bounded JSON result; full rows stay in coverage.json.
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
            cases,
            json,
        } => {
            if crate::signed_approval::required()
                && let Some(anchor) = &approved_anchor
            {
                crate::signed_approval::check_anchor(anchor, None)?;
            }
            let anchors = Anchors {
                approved: approved_anchor.as_deref(),
                last_good: last_good.as_deref(),
            };
            let (path, value) = if let Some(cases) = cases {
                let declaration = saccade_core::coverage::read_declaration(&cases)?;
                saccade_core::coverage::write_manifest(&dir, anchors, &declaration)?
            } else {
                manifest::write(&dir, anchors)?
            };
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
        Operation::Views {
            target,
            out,
            group_by,
            now_unix,
            max_age_seconds,
            json,
        } => {
            let path = if target.is_dir() {
                target.join(manifest::MANIFEST_FILE)
            } else {
                target
            };
            let now = match now_unix {
                Some(now) => now,
                None => std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| CliError::new("config", e.to_string()))?
                    .as_secs(),
            };
            let report = saccade_core::coverage::analyze(&path, &group_by, now, max_age_seconds)?;
            saccade_core::run::guard_output_dir(&out, &[&path], &["coverage.json"])?;
            std::fs::create_dir_all(&out).map_err(|e| CliError::io(e.to_string()))?;
            let artifact = out.join("coverage.json");
            let linked = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
            manifest::write_owned(&artifact, saccade_core::coverage::REPORT_SCHEMA, &linked)?;
            saccade_core::report_links::index(&artifact, &linked)?;
            saccade_core::render::render_coverage_html(&report, &path, &out)?;
            let mut result = base_result("manifest.views");
            result["verdict"] = report.coverage.clone().into();
            result["artifact"] = reference(&artifact)?;
            result["counts"] = json!({"expected":report.counts.expected,"captured":report.counts.captured,"measured":report.counts.measured,"refused":report.counts.refused});
            result["data"] = json!({"outcomes":report.counts.outcomes,"health_cases":report.rows.iter().filter(|r| !r.health.is_empty()).count(), "groups":report.groups.len(), "html":saccade_core::paths::portable(&out.join("index.html"))});
            if json {
                crate::emit(&format!("{}\n", serde_json::to_string(&result)?))?;
            } else {
                crate::emit(&format!(
                    "coverage: {}; {} expected, {} captured, {} measured, {} refused; {} health cases; {}\n",
                    report.coverage,
                    report.counts.expected,
                    report.counts.captured,
                    report.counts.measured,
                    report.counts.refused,
                    result["data"]["health_cases"],
                    crate::escape_control(&out.join("index.html").display().to_string())
                ))?;
            }
            Ok(u8::from(report.coverage != "complete"))
        }
        Operation::Verify { target, json } => {
            if crate::signed_approval::required() {
                crate::signed_approval::check_manifest(&target)?;
            }
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
            if crate::signed_approval::required() {
                crate::signed_approval::check_manifest(&dir)?;
            }
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
            if crate::signed_approval::required()
                && (value["manifest"] == true
                    || value["kind"] == "manifest"
                    || value["kind"] == "link")
            {
                crate::signed_approval::check_manifest(&path)?;
            }
            crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
            Ok(0)
        }
    }
}
