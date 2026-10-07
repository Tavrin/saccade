//! CLI adapters for test-runner screenshot files. No test runner is executed.
use crate::agent::CliError;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Args)]
pub(crate) struct IngestArgs {
    #[command(subcommand)]
    operation: IngestOperation,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod containment_tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn manifest_paths_reject_parent_absolute_and_symlink_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("run");
        std::fs::create_dir(&root).unwrap();
        let outside = temp.path().join("outside.png");
        std::fs::write(&outside, b"image").unwrap();
        assert!(contained_source(&root, Path::new("../outside.png")).is_err());
        assert!(contained_source(&root, &outside).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("link.png")).unwrap();
            assert!(contained_source(&root, Path::new("link.png")).is_err());
        }
    }
}

#[derive(Subcommand)]
enum IngestOperation {
    /// Pair Blender render report category/ref images with category renders.
    Blender {
        root: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Pair Bevy screenshot-N.png files from two runs.
    Bevy {
        reference: PathBuf,
        capture: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Pair Unity Graphics Test Framework ReferenceImages and ActualImages.
    Unity {
        assets: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Read Unreal screenshot comparison result paths from JSON.
    Unreal {
        results: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
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

pub(crate) fn contained_source(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<PathBuf, CliError> {
    Ok(saccade_core::workflows::ingest::contained_source(
        root, relative,
    )?)
}
pub(crate) fn run(args: IngestArgs, absolute: bool) -> Result<u8, CliError> {
    match args.operation {
        IngestOperation::Blender { root, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Blender(root),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Bevy {
            reference,
            capture,
            out,
            json,
        } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Bevy(reference, capture),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Unity { assets, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Unity(assets),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Unreal { results, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Unreal(results),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Playwright {
            manifest,
            out,
            json,
            threshold,
        } => {
            let result = saccade_core::workflows::ingest::run_playwright(
                &manifest,
                &out,
                threshold,
                absolute,
                crate::signed_approval::policy(),
            )?;
            for warning in &result.warnings {
                eprintln!("{warning}");
            }
            if !json && let Some(coverage) = &result.coverage {
                crate::emit(&format!(
                    "capture coverage: {coverage}; see inventory.json\n"
                ))?;
            }
            crate::emit_run(&result.report, &result.report_dir, json, absolute)?;
            Ok(u8::from(result.report.is_regression() || result.incomplete))
        }
    }
}
