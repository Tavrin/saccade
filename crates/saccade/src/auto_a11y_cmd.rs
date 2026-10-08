//! Automatic accessibility CLI policy mirror.
use crate::agent::CliError;
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand)]
enum Command {
    /// Detect text/UI candidates and check contrast, legibility, glyphs and colour loss offline.
    Auto(AutoArgs),
}
#[derive(Clone, Copy, clap::ValueEnum)]
enum Level {
    Aa,
    Aaa,
}
#[derive(clap::Args)]
struct AutoArgs {
    /// Opaque sRGB image or directory; no regions required.
    input: PathBuf,
    /// Report artifacts directory.
    #[arg(long)]
    out: PathBuf,
    /// Authoritative declared regions, checked separately and overriding overlap.
    #[arg(long)]
    config: Option<PathBuf>,
    /// WCAG text contrast target.
    #[arg(long, value_enum, ignore_case = true, default_value = "AA")]
    level: Level,
    /// Capture pixels per typographic point; omitted scale uses normal-text threshold.
    #[arg(long)]
    px_per_pt: Option<f64>,
    /// Provisioned PaddleOCR cache (default: existing model cache); never downloads.
    #[arg(long)]
    model_cache: Option<PathBuf>,
    /// Print versioned JSON evidence.
    #[arg(long)]
    json: bool,
    /// Optional JUnit (unknown evidence is skipped).
    #[arg(long)]
    junit: Option<PathBuf>,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let Command::Auto(a) = args.command;
    #[allow(unused_mut)]
    let mut model_cache = a.model_cache.clone();
    #[cfg(feature = "ocr")]
    if model_cache.is_none() {
        model_cache = Some(saccade_core::media::default_model_dir());
    }
    let options = saccade_a11y::Options {
        config: a.config.clone(),
        level: match a.level {
            Level::Aa => saccade_a11y::auto::Level::AA,
            Level::Aaa => saccade_a11y::auto::Level::AAA,
        },
        px_per_pt: a.px_per_pt.unwrap_or(96. / 72.),
        scale_known: a.px_per_pt.is_some(),
        model_cache: model_cache.clone(),
    };
    let report = saccade_a11y::run(&a.input, &a.out, &options)?;
    if let Some(path) = a.junit.as_deref() {
        let mut inputs = vec![a.input.as_path(), a.out.as_path()];
        if let Some(c) = a.config.as_deref() {
            inputs.push(c);
        }
        if let Some(c) = model_cache.as_deref() {
            inputs.push(c);
        }
        report.junit(path, &inputs)?;
    }
    if a.json {
        crate::local_cmd::print(
            &crate::local_cmd::analysis_result(&serde_json::to_value(&report)?, &a.out)?,
            true,
        )?;
    } else {
        crate::emit(&report.text())?;
    }
    Ok(u8::from(
        report.verdict == saccade_a11y::auto::Verdict::Fail,
    ))
}
