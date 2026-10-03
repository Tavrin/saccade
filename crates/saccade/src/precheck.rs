//! CLI registrations for deterministic safety/accessibility pre-checks.

use crate::agent::CliError;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct SafetyArgs {
    /// Numbered frames or mp4/mov/mkv (requires external ffmpeg).
    pub input: PathBuf,
    /// Frame rate override; otherwise metadata, or 60 for frame directories.
    #[arg(long)]
    pub fps: Option<f64>,
    /// WxH@diagonal_inches,distance_metres (default 1920x1080@55,4).
    #[arg(long)]
    pub display: Option<String>,
    /// itu-bt1702 or wcag. PRE-CHECK only, never certification.
    #[arg(long, default_value = "itu-bt1702")]
    pub standard: String,
    /// Print full saccade-safety.v1 JSON.
    #[arg(long)]
    pub json: bool,
    /// Output directory for JSON, text, HTML, static frames and risk heatmaps.
    #[arg(long, default_value = "safety-report")]
    pub out: PathBuf,
    /// Optional JUnit XML destination.
    #[arg(long)]
    pub junit: Option<PathBuf>,
}

#[derive(clap::Args)]
pub(crate) struct A11yArgs {
    /// Opaque sRGB image or image directory.
    pub input: PathBuf,
    /// Explicit saccade.toml with [[region]] kind="text" or "ui".
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Print full saccade-a11y.v1 JSON.
    #[arg(long)]
    pub json: bool,
    /// Output directory for JSON, text, HTML and simulation/heatmap artifacts.
    #[arg(long, default_value = "a11y-report")]
    pub out: PathBuf,
    /// Optional JUnit XML destination.
    #[arg(long)]
    pub junit: Option<PathBuf>,
    /// Explicitly upload 16 crops/image to Gemini for unconfirmed region proposals.
    #[arg(long)]
    pub suggest_regions: bool,
    /// Judge key policy: gemini.env/SACCADE_GEMINI_API_KEY, never ambient keys.
    #[arg(long, requires = "suggest_regions")]
    pub keys_dir: Option<PathBuf>,
}

pub(crate) fn safety(args: SafetyArgs) -> Result<u8, CliError> {
    let options = saccade_core::safety::Options {
        fps: args.fps,
        display: args
            .display
            .as_deref()
            .map(saccade_core::safety::Display::parse)
            .transpose()?
            .unwrap_or_default(),
        standard: saccade_core::safety::Standard::parse(&args.standard)?,
    };
    let report = saccade_core::safety::run(&args.input, &args.out, &options)?;
    if let Some(path) = &args.junit {
        saccade_core::safety::output::junit(
            path,
            "saccade safety pre-check",
            &report.junit_cases(),
            &[&args.input],
        )?;
    }
    if args.json {
        crate::local_cmd::print(
            &crate::local_cmd::analysis_result(&serde_json::to_value(&report)?, &args.out)?,
            true,
        )?;
    } else {
        crate::emit(&report.text())?;
    }
    Ok(u8::from(report.verdict == "FAIL"))
}

pub(crate) fn a11y(args: A11yArgs) -> Result<u8, CliError> {
    let options = saccade_core::a11y::Options {
        config: args.config.clone(),
        suggest_regions: args.suggest_regions,
        keys_dir: args.keys_dir,
    };
    let report = saccade_core::a11y::run(&args.input, &args.out, &options)?;
    if let Some(path) = &args.junit {
        let mut inputs = vec![args.input.as_path()];
        if let Some(config) = &args.config {
            inputs.push(config.as_path());
        }
        saccade_core::safety::output::junit(
            path,
            "saccade accessibility pre-check",
            &report.junit_cases(),
            &inputs,
        )?;
    }
    if args.json {
        crate::local_cmd::print(
            &crate::local_cmd::analysis_result(&serde_json::to_value(&report)?, &args.out)?,
            true,
        )?;
    } else {
        crate::emit(&report.text())?;
    }
    Ok(u8::from(report.verdict == "FAIL"))
}
