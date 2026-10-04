//! CLI adapters for test-runner screenshot files. No test runner is executed.
use crate::agent::CliError;
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(crate) struct IngestArgs {
    #[command(subcommand)]
    operation: IngestOperation,
}

#[derive(Subcommand)]
enum IngestOperation {
    /// Compare expected and actual Playwright screenshot attachments.
    Playwright {
        /// Manifest written by integrations/playwright/reporter.cjs.
        manifest: PathBuf,
        /// New directory for paired inputs and the comparison report.
        #[arg(long)]
        out: PathBuf,
        /// Print the bounded comparison result.
        #[arg(long)]
        json: bool,
        /// FLIP threshold for the comparison.
        #[arg(long)]
        threshold: Option<f64>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    entries: Vec<Snapshot>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    test_id: String,
    project: String,
    browser: String,
    viewport: Option<[u32; 2]>,
    expected: String,
    actual: String,
    diff: Option<String>,
}

fn source(manifest: &Path, relative: &str) -> Result<PathBuf, CliError> {
    if relative.is_empty() {
        return Err(CliError::usage("Playwright attachment path is empty"));
    }
    let path = manifest.parent().unwrap_or(Path::new(".")).join(relative);
    let path = std::fs::canonicalize(&path).map_err(|e| CliError::io(e.to_string()))?;
    if !path.is_file() {
        return Err(CliError::usage(
            "Playwright attachment is not a regular file",
        ));
    }
    Ok(path)
}

fn copy_image(from: &Path, to: &Path) -> Result<(), CliError> {
    image::image_dimensions(from)
        .map_err(|e| CliError::usage(format!("invalid Playwright screenshot: {e}")))?;
    std::fs::copy(from, to).map_err(|e| CliError::io(e.to_string()))?;
    Ok(())
}

pub(crate) fn run(args: IngestArgs, absolute: bool) -> Result<u8, CliError> {
    match args.operation {
        IngestOperation::Playwright {
            manifest,
            out,
            json,
            threshold,
        } => {
            if std::fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(CliError::usage("ingest --out must not be a symlink"));
            }
            let raw = std::fs::read(&manifest).map_err(|e| CliError::io(e.to_string()))?;
            let source_manifest: Manifest = serde_json::from_slice(&raw)?;
            if source_manifest.schema != "saccade-playwright.v1"
                || source_manifest.entries.is_empty()
            {
                return Err(CliError::usage(
                    "expected nonempty saccade-playwright.v1 manifest",
                ));
            }
            if out.exists()
                && std::fs::read_dir(&out)
                    .map_err(|e| CliError::io(e.to_string()))?
                    .next()
                    .is_some()
            {
                return Err(CliError::usage("ingest --out must be empty or absent"));
            }
            let mut seen = BTreeSet::new();
            let mut validated = Vec::new();
            for entry in &source_manifest.entries {
                if entry.test_id.is_empty()
                    || !seen.insert((entry.test_id.clone(), entry.project.clone()))
                {
                    return Err(CliError::usage(
                        "Playwright test IDs must be nonempty and unique per project",
                    ));
                }
                validated.push((
                    source(&manifest, &entry.expected)?,
                    source(&manifest, &entry.actual)?,
                    entry
                        .diff
                        .as_deref()
                        .map(|d| source(&manifest, d))
                        .transpose()?,
                ));
            }
            let baseline = out.join("baseline");
            let capture = out.join("capture");
            let diffs = out.join("playwright-diffs");
            for dir in [&baseline, &capture, &diffs] {
                std::fs::create_dir_all(dir).map_err(|e| CliError::io(e.to_string()))?;
            }
            let mut mapping = Vec::new();
            for (index, (entry, (expected, actual, diff))) in source_manifest
                .entries
                .iter()
                .zip(validated.iter())
                .enumerate()
            {
                let name = format!("{index:04}.png");
                copy_image(expected, &baseline.join(&name))?;
                copy_image(actual, &capture.join(&name))?;
                if let Some(diff) = diff {
                    copy_image(diff, &diffs.join(&name))?;
                }
                let sidecar = json!({
                    "playwright_test_id":entry.test_id,
                    "playwright_project":entry.project,
                    "playwright_browser":entry.browser,
                    "playwright_viewport_width":entry.viewport.map(|v|v[0]),
                    "playwright_viewport_height":entry.viewport.map(|v|v[1])
                });
                let sidecar_name = format!("{index:04}.saccade-meta.json");
                for dir in [&baseline, &capture] {
                    crate::local_cmd::write_value(&dir.join(&sidecar_name), &sidecar)?;
                }
                mapping.push(json!({"entry":name,"test_id":entry.test_id,"project":entry.project,"browser":entry.browser,"viewport":entry.viewport,"diff":diff.as_ref().map(|_|format!("playwright-diffs/{index:04}.png"))}));
            }
            crate::local_cmd::write_value(
                &out.join("playwright-mapping.json"),
                &json!({"schema":"saccade-playwright-mapping.v1","entries":mapping}),
            )?;
            let mut config = saccade_core::config::RunConfig {
                record_absolute_paths: absolute,
                ..Default::default()
            };
            if let Some(threshold) = threshold {
                config.default_threshold = threshold;
            }
            let report_dir = out.join("report");
            let report = saccade_core::run::run(&baseline, &capture, &report_dir, &config)?;
            crate::emit_run(&report, &report_dir, json, absolute)?;
            Ok(u8::from(report.is_regression()))
        }
    }
}
