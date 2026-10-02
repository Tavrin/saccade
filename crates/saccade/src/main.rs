//! `saccade` command-line interface: `compare`, `identity`, `approve`, `view`, `unblind`, `summary`.
//!
//! Exit codes: `0` no regression, `1` regression, `2` usage/config/IO error.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use saccade_core::config::RunConfig;
use saccade_core::render::{MarkdownOptions, is_valid_comment_key, render_markdown};
use saccade_core::report::{Labels, Metric, Mode, Report, Status};
use saccade_core::view::{
    Verdict, ViewOptions, build_view, is_safe_name, read_blind_key, read_decisions, unblind,
};

mod agent;
mod agent_ui;
mod f1;
mod judge_cmd;
mod mcp;
mod outdirs;
mod perf_cmd;
mod precheck;
mod precheck_mcp;
mod review_cmd;
mod runs_cmd;
mod s6;
mod s6_mcp;

use agent::CliError;

#[derive(Parser)]
#[command(
    name = "saccade",
    version,
    about = "Perceptual (FLIP) visual-regression diffing"
)]
struct Cli {
    /// Silence warnings when --out is next to capture metadata.
    #[arg(long, global = true)]
    allow_out_near_captures: bool,
    /// Opt in to absolute local paths in reports and machine-readable output.
    #[arg(long, global = true)]
    record_absolute_paths: bool,
    #[command(subcommand)]
    command: Command,
}

/// HDR-FLIP flags shared by `compare` and `view`.
#[derive(clap::Args, Clone, Default)]
struct HdrArgs {
    /// Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard.
    #[arg(long, value_name = "NAME")]
    hdr_tonemapper: Option<String>,
    /// Exposure range in stops and count, `START:STOP:N` (default: computed
    /// from the baseline image).
    #[arg(long, value_name = "START:STOP:N", allow_hyphen_values = true)]
    hdr_exposures: Option<String>,
}

impl HdrArgs {
    fn apply(&self, hdr: &mut saccade_core::hdr::HdrConfig) -> Result<(), CliError> {
        if let Some(t) = &self.hdr_tonemapper {
            hdr.tonemapper = saccade_core::hdr::Tonemapper::parse(t)?;
        }
        if let Some(e) = &self.hdr_exposures {
            hdr.parse_exposures(e)?;
        }
        Ok(())
    }
}

/// Metadata-sidecar flags shared by `compare`, `identity` and `view`.
#[derive(clap::Args, Clone, Default)]
struct MetaArgs {
    /// Sidecar file name (default `saccade-meta.json`); the per-image sidecar is
    /// `<stem>.<name>` and overrides the directory-level one.
    #[arg(long, value_name = "NAME")]
    meta_name: Option<String>,
    /// Extra sidecar key globs to ignore, added to the built-in timing,
    /// timestamp and run-id defaults.
    #[arg(long, value_delimiter = ',', value_name = "GLOB,...")]
    meta_ignore: Vec<String>,
}

/// Metadata enforcement flags for `compare` and `identity`.
#[derive(clap::Args, Clone, Default)]
struct MetaRequireArgs {
    /// Make an entry an error when a sidecar key differs and is not declared.
    #[arg(long)]
    require_matching_meta: bool,
    /// Sidecar keys (or globs) that may differ with --require-matching-meta.
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "KEY,...",
        requires = "require_matching_meta"
    )]
    declare: Vec<String>,
}

impl MetaArgs {
    fn apply(&self, meta: &mut saccade_core::meta::MetaOptions) {
        if let Some(n) = &self.meta_name {
            meta.name.clone_from(n);
        }
        meta.ignore.extend(self.meta_ignore.iter().cloned());
    }
}

impl MetaRequireArgs {
    fn apply(&self, meta: &mut saccade_core::meta::MetaOptions) {
        meta.required |= self.require_matching_meta;
        meta.declared.extend(self.declare.iter().cloned());
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum MetricArg {
    Mean,
    P95,
    P99,
    Max,
}

impl From<MetricArg> for Metric {
    fn from(m: MetricArg) -> Self {
        match m {
            MetricArg::Mean => Metric::Mean,
            MetricArg::P95 => Metric::P95,
            MetricArg::P99 => Metric::P99,
            MetricArg::Max => Metric::Max,
        }
    }
}

/// How much `compare --json` and `identity --json` print.
#[derive(Clone, Copy, ValueEnum)]
enum RunJson {
    /// The lean `saccade-result.v1`.
    Lean,
    /// The whole `saccade-report.v1`.
    Full,
    /// A `saccade-decision-request.v1` for every failing entry.
    Decision,
}

/// How much `sequence --json` and `rank --json` print.
#[derive(Clone, Copy, ValueEnum)]
enum JsonMode {
    /// The lean `saccade-result.v1`.
    Lean,
    /// The whole `saccade-report.v1`.
    Full,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Markdown,
    Text,
    Json,
}

#[derive(Subcommand)]
enum Command {
    /// Photosensitivity PRE-CHECK only; not certification or formal compliance.
    Safety(precheck::SafetyArgs),
    /// Accessibility PRE-CHECK only; not certification or formal compliance.
    A11y(precheck::A11yArgs),
    /// Bootstrap a commented configuration and print baseline adoption steps.
    Init(f1::InitArgs),
    /// Inspect effective configuration and the sources of image-specific settings.
    Config(f1::ConfigArgs),
    /// Filter and paginate full entries from an existing report.
    Entries(f1::EntriesArgs),
    /// Calibrate thresholds from repeated captures of an unchanged build.
    Noise(f1::NoiseArgs),
    /// Compare ablation arms against a base with image and performance evidence.
    Ablate(perf_cmd::AblateArgs),
    /// Run the bundled example and explain its expected regression.
    Demo(f1::DemoArgs),
    /// Judge a report or ranking with a panel of models and humans.
    Judge(judge_cmd::JudgeArgs),
    /// Run the proposal-only AI review cascade on a report.
    Review(review_cmd::ReviewArgs),
    /// Find the first diverging run or revision in an ordered series.
    Bisect(s6::BisectArgs),
    /// Compare initially and after debounced capture-directory changes.
    Watch(s6::WatchArgs),
    /// Post a closed-answer question to a local human inbox.
    Ask(s6::AskArgs),
    /// Compare numbered colour frames by sorted index and measure added flicker.
    Sequence {
        baseline_dir: PathBuf,
        capture_dir: PathBuf,
        /// Relative-name glob; frames must end in an integer before the extension.
        #[arg(long, default_value = "*")]
        pattern: String,
        #[arg(long, default_value = "sequence-report")]
        out: PathBuf,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        ppd: Option<f32>,
        #[arg(long)]
        fail_on_new: bool,
        #[arg(long)]
        allow_empty: bool,
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        #[arg(long, value_enum, num_args = 0..=1, require_equals = true, default_missing_value = "lean")]
        json: Option<JsonMode>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
    },
    /// Rank candidate directories against one common FLIP reference.
    Rank {
        reference_dir: PathBuf,
        #[arg(required = true, num_args = 1..)]
        candidate_dirs: Vec<PathBuf>,
        /// One unique, safe directory label per candidate, comma separated.
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        #[arg(long, value_enum, default_value = "mean")]
        metric: MetricArg,
        #[arg(long, default_value = "rank-report")]
        out: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        threshold: Option<f64>,
        #[arg(long)]
        ppd: Option<f32>,
        #[arg(long)]
        fail_on_new: bool,
        #[arg(long)]
        allow_empty: bool,
        #[arg(long, value_enum, num_args = 0..=1, require_equals = true, default_missing_value = "lean")]
        json: Option<JsonMode>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
    },
    /// Compare a directory of captures against a directory of baselines.
    Compare {
        /// Directory of approved baseline images.
        baseline_dir: PathBuf,
        /// Directory of fresh captures.
        capture_dir: PathBuf,
        /// Report output directory.
        #[arg(long, default_value = "report")]
        out: PathBuf,
        /// Default pass threshold (overrides the config file's top level).
        #[arg(long)]
        threshold: Option<f64>,
        /// Default deciding metric (overrides the config file's top level).
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./saccade.toml when it exists.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Treat new images (no baseline) as a regression.
        #[arg(long)]
        fail_on_new: bool,
        /// Accept a run that compared no pair (for example the first run, with
        /// an empty baseline directory). Without it, nothing compared exits 1.
        #[arg(long)]
        allow_empty: bool,
        /// Print JSON instead of the table: a lean `saccade-result.v1` (verdict,
        /// totals, failing entries, paths, next step), or with `--json=full`
        /// the whole report.
        #[arg(
            long,
            value_enum,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "lean",
            value_name = "full"
        )]
        json: Option<RunJson>,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Display names of the two sides, `baseline,capture`.
        #[arg(long, value_delimiter = ',', value_name = "A,B")]
        labels: Option<Vec<String>>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long, value_name = "GLOB")]
        entries: Vec<String>,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Check that a candidate build matches its parent: strict defaults
    /// (metric max, threshold 0), bit-identity reported per image.
    Identity {
        /// Directory of images from the parent build.
        parent_dir: PathBuf,
        /// Directory of images from the candidate build.
        candidate_dir: PathBuf,
        /// Report output directory.
        #[arg(long, default_value = "report")]
        out: PathBuf,
        /// Accept a run that compared no pair. Without it, nothing compared
        /// exits 1.
        #[arg(long)]
        allow_empty: bool,
        /// Pass threshold for images that are not bit-identical.
        #[arg(long)]
        threshold: Option<f64>,
        /// Deciding metric (default: max).
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./saccade.toml when it exists.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print JSON instead of the table: a lean `saccade-result.v1` (verdict,
        /// totals, failing entries, paths, next step), or with `--json=full`
        /// the whole report.
        #[arg(
            long,
            value_enum,
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "lean",
            value_name = "full"
        )]
        json: Option<RunJson>,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Display names of the two sides, `parent,candidate`.
        #[arg(long, value_delimiter = ',', value_name = "A,B")]
        labels: Option<Vec<String>>,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long, value_name = "GLOB")]
        entries: Vec<String>,
        /// Write one JUnit testcase per entry.
        #[arg(long, value_name = "FILE.xml")]
        junit: Option<PathBuf>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Copy captures over baselines.
    Approve {
        /// Directory of fresh captures.
        capture_dir: Option<PathBuf>,
        /// Baseline directory to update.
        baseline_dir: Option<PathBuf>,
        /// Derive the input directories from this report.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Image names (relative paths) to approve.
        names: Vec<String>,
        /// Also approve every fail and new entry of this report JSON.
        #[arg(long, value_name = "REPORT_JSON", num_args = 0..=1, default_missing_value = "__report__")]
        all_failing: Option<PathBuf>,
        /// Also approve every "accept" entry of a decisions file exported by
        /// `saccade view`.
        #[arg(long, value_name = "DECISIONS_JSON")]
        decisions: Option<PathBuf>,
        /// With --all-failing: also approve `error` entries (for example a size
        /// change) whose capture exists and decodes.
        #[arg(long)]
        include_errors: bool,
        /// With --all-failing: delete the baselines of every `missing` entry
        /// of the report (capture absent). Only files inside the baseline
        /// directory are removed; each removal is printed.
        #[arg(long)]
        prune_missing: bool,
        /// Print `{"schema":"saccade-approve.v1","copied":[...],"pruned":[...]}`
        /// instead of one line per file.
        #[arg(long)]
        json: bool,
        /// Approve even when the directories or the capture files differ from
        /// what the report or decisions file recorded (they are checked by
        /// default: approving unreviewed pixels is refused).
        #[arg(long)]
        force: bool,
    },
    /// Write a self-contained review viewer for 2 to 6 image directories.
    View {
        /// Directories to compare, paired by relative image path (2 to 6).
        #[arg(required = true, num_args = 2..=6)]
        dirs: Vec<PathBuf>,
        /// Comma-separated labels, one per directory (default: directory names).
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        /// FLIP reference: a label or one of the directories (default: the first).
        #[arg(long)]
        reference: Option<String>,
        /// Pairwise judging: shuffle panes and hide labels until "Reveal".
        #[arg(long)]
        blind: bool,
        /// Seed for the blind shuffle (default: random). A blind page never
        /// embeds it; it is recorded in the key.
        #[arg(long)]
        seed: Option<u64>,
        /// Where a blind view's key goes (default: `blind-key.json` inside
        /// `--out`; put it elsewhere to hand the view directory to a judge).
        #[arg(long, value_name = "PATH", requires = "blind")]
        key_out: Option<PathBuf>,
        /// Output directory.
        #[arg(long, default_value = "view")]
        out: PathBuf,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Config file whose `[[region]]` tables become preset ROIs
        /// (default: `./saccade.toml` when present).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print a JSON summary (`saccade-view-summary.v1`) instead of text.
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long, value_name = "GLOB")]
        entries: Vec<String>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Serve a local web app for browsing a capture archive and comparing runs
    /// (127.0.0.1 only; the archive is never written to).
    Serve {
        /// Archive roots to browse (read-only). With several, each is a
        /// top-level entry named after its directory.
        #[arg(required = true, num_args = 1..)]
        roots: Vec<PathBuf>,
        /// Let a symlink that resolves inside any of the roots be browsed and
        /// served; a symlink to anywhere else stays refused.
        #[arg(long)]
        follow_symlinks_within_roots: bool,
        /// Allow symlinks reached below a root to resolve into DIR (repeatable).
        #[arg(long = "symlink-target")]
        symlink_targets: Vec<PathBuf>,
        /// Storage deadline in milliseconds (default: 3000).
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        fs_timeout_ms: Option<u64>,
        /// Port on 127.0.0.1 (0 picks a free one).
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Cache directory for sessions, thumbnails and uploads
        /// (default: `$XDG_CACHE_HOME/saccade`).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Directory the viewer's decisions are written to
        /// (default: `$XDG_DATA_HOME/saccade/decisions`).
        #[arg(long)]
        decisions_dir: Option<PathBuf>,
        /// Config file for sidecar settings and preset regions
        /// (default: `./saccade.toml` when present).
        #[arg(long)]
        config: Option<PathBuf>,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Open the page in the default browser.
        #[arg(long)]
        open: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Compare whole runs against a reference run: per-run summary, an image
    /// matrix tinted by FLIP severity and a contact sheet, as a static HTML
    /// page (`--out`) or `saccade-runs.v1` JSON (`--json`).
    Runs {
        /// The reference run: a directory of images.
        ref_dir: PathBuf,
        /// The runs to compare against it (1 to 5), paired by relative image
        /// path.
        #[arg(required = true, num_args = 1..=5)]
        run_dirs: Vec<PathBuf>,
        /// Comma-separated labels, one per directory, the reference first
        /// (default: directory names).
        #[arg(long, value_delimiter = ',')]
        labels: Option<Vec<String>>,
        /// Pair the images of every run with the reference's by sorted
        /// position instead of by name (for runs whose file names differ).
        #[arg(long)]
        pair_by_position: bool,
        /// Print `saccade-runs.v1` JSON instead of text; the page is then
        /// written only when `--out` is given too.
        #[arg(long)]
        json: bool,
        /// Directory for the static page (`index.html`, thumbnails,
        /// `saccade-runs.v1.json`); default `runs`.
        #[arg(long)]
        out: Option<PathBuf>,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Config file for sidecar settings (default: `./saccade.toml` when
        /// present).
        #[arg(long)]
        config: Option<PathBuf>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long, value_name = "GLOB")]
        entries: Vec<String>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        perf: perf_cmd::PerfArgs,
    },
    /// Turn a decisions file exported from a `--blind` view into one with the
    /// true directory labels, using the view's `blind-key.json`.
    Unblind {
        /// Decisions file exported by the blind viewer.
        decisions_json: PathBuf,
        /// `blind-key.json` written next to the view.
        blind_key_json: PathBuf,
        /// Write the result here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Write crops and data that explain where and how a report's failing
    /// images differ: per-hotspot strips for agents and vision-model judges.
    Explain {
        /// Path to `saccade-report.v1.json`.
        report_json: PathBuf,
        /// Output directory (default: `explain/` next to the report JSON).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Hotspots per entry.
        #[arg(long, default_value_t = 3)]
        top: usize,
        /// Pixels of context around each hotspot.
        #[arg(long, default_value_t = 16)]
        pad: u32,
        /// Contrast-stretch dark crops (same gain on both images).
        #[arg(long)]
        stretch: bool,
        /// Shuffle which side is A or B per hotspot and omit the heatmap; the
        /// pack names neither the report nor the sides, and the key goes to
        /// `--key-out`.
        #[arg(long, requires = "key_out")]
        blind: bool,
        /// Where the blind key is written (required with `--blind`; must be
        /// outside `--out`, so the pack can be handed to a judge as is).
        #[arg(long, value_name = "PATH", requires = "blind")]
        key_out: Option<PathBuf>,
        /// Seed for the blind shuffle (default: from the clock; recorded in the key).
        #[arg(long, requires = "blind")]
        seed: Option<u64>,
        /// Leave out hotspots carrying less than this share of the total
        /// error, 0 to 1.
        #[arg(long, default_value_t = 0.01, value_name = "SHARE")]
        hotspot_min_share: f64,
        /// Explain only these entries (default: every failing entry).
        #[arg(long, value_delimiter = ',', value_name = "NAME,...")]
        entries: Vec<String>,
        /// Print the pack's `explain.json` instead of `explain.md`.
        #[arg(long)]
        json: bool,
    },
    /// Serve the Model Context Protocol over stdio, so an AI agent can run
    /// comparisons and read their hotspots as a tool. Every path an agent
    /// passes must resolve under `--root`.
    Mcp {
        /// Directory the agent may read and write (default: the working directory).
        #[arg(long, value_name = "DIR")]
        root: Option<PathBuf>,
        /// Watch baseline:capture inside the server root (repeatable).
        #[arg(long, value_name = "BASE:CAP")]
        watch: Vec<String>,
    },
    /// Render a view state (layout, split, zoom, heatmap...) of a report entry
    /// or view set to a PNG, with no browser.
    Snapshot(agent_ui::SnapshotArgs),
    /// Print the bounded questions an agent or decision model can answer about
    /// a report's entries (`saccade-decision-request.v1`).
    DecisionRequest(agent_ui::DecisionRequestArgs),
    /// Record an answer to a decision-request question; a model's answer is a
    /// proposal a person confirms, never a baseline change.
    Decide(agent_ui::DecideArgs),
    /// Print a summary of a report JSON.
    Summary {
        /// Path to `saccade-report.v1.json`.
        report_json: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value = "markdown")]
        format: Format,
        /// Link to the uploaded report artifact (markdown only).
        #[arg(long)]
        artifact_url: Option<String>,
        /// Marker key for the sticky comment, so matrix jobs keep separate
        /// comments (ASCII letters, digits, `.`, `_`, `-`; at most 64).
        #[arg(long)]
        comment_key: Option<String>,
    },
}

/// Whether the command line asks for JSON output (`--json`, `--json=full`,
/// `--format json`), judged from the raw arguments so that even a command line
/// clap rejects gets a JSON error.
fn args_want_json(args: &[std::ffi::OsString]) -> bool {
    let args: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.iter().enumerate().any(|(i, a)| {
        a == "--json"
            || a.starts_with("--json=")
            || a == "--format=json"
            || (a == "--format" && args.get(i + 1).is_some_and(|v| v == "json"))
    })
}

/// Prints `err` as `saccade-error.v1` on stdout.
fn emit_json_error(err: &CliError) {
    let text = serde_json::to_string_pretty(&err.value()).unwrap_or_default();
    let _ = emit(&format!("{text}\n"));
}

fn main() -> ExitCode {
    // The debug clap command builder alone uses almost 1 MiB of stack. Windows
    // gives the process's main thread 1 MiB, so parse and execute on an explicit
    // stack on every platform, independent of linker defaults or RUST_MIN_STACK.
    match std::thread::Builder::new()
        .name("saccade-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(cli_main)
    {
        Ok(thread) => match thread.join() {
            Ok(code) => code,
            Err(panic) => std::panic::resume_unwind(panic),
        },
        Err(error) => {
            let error = CliError::io(format!("starting CLI thread: {error}"));
            let args: Vec<_> = std::env::args_os().collect();
            if args_want_json(&args) {
                emit_json_error(&error);
            } else {
                eprintln!("saccade: error: {error}\n  hint: {}", error.hint);
            }
            ExitCode::from(2)
        }
    }
}

fn cli_main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            if e.use_stderr() && args_want_json(&args) {
                let text = e.to_string();
                // The message is the text before clap's `Usage:` block.
                let message = text
                    .split("\n\n")
                    .next()
                    .unwrap_or("invalid arguments")
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                emit_json_error(&CliError::usage(
                    message.trim_start_matches("error: ").to_string(),
                ));
                return ExitCode::from(2);
            }
            if e.use_stderr() {
                eprintln!("{e}hint: {}", agent::CliError::usage(e.to_string()).hint);
                return ExitCode::from(2);
            }
            e.exit()
        }
    };
    let json_errors = args_want_json(&args);
    match outdirs::warn(&cli.command, cli.allow_out_near_captures)
        .and_then(|()| dispatch(cli.command, cli.record_absolute_paths))
    {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            if json_errors {
                emit_json_error(&err);
            } else {
                eprintln!(
                    "saccade: error: {}\n  hint: {}",
                    escape_multiline(&err.message),
                    escape_control(&err.hint)
                );
            }
            ExitCode::from(2)
        }
    }
}

/// Prints a run's result: the table, the lean result or the whole report.
fn emit_run(report: &Report, out: &Path, json: Option<RunJson>) -> Result<(), CliError> {
    match json {
        None => emit(&text_table(report)),
        Some(RunJson::Lean) => {
            let report_json = out.join(saccade_core::report::REPORT_FILE_NAME);
            // An empty run prints an error object, on stdout, with exit code 1.
            if let Some(err) = agent::nothing_compared(report, &report_json) {
                return emit(&format!(
                    "{}\n",
                    serde_json::to_string_pretty(&err.value())?
                ));
            }
            let value = agent::result_value(
                report,
                &out.join(saccade_core::report::REPORT_FILE_NAME),
                agent::DEFAULT_TOP_FAILING,
                false,
            );
            emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))
        }
        Some(RunJson::Full) => emit(&format!("{}\n", serde_json::to_string_pretty(report)?)),
        Some(RunJson::Decision) => {
            let request = saccade_core::decision::build_request(
                report,
                &saccade_core::decision::RequestOptions {
                    all_failing: true,
                    ..Default::default()
                },
            )?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&request)?))
        }
    }
}

fn dispatch(command: Command, record_absolute_paths: bool) -> Result<u8, CliError> {
    match command {
        Command::Safety(args) => precheck::safety(args),
        Command::A11y(args) => precheck::a11y(args),
        Command::Init(args) => f1::init(args),
        Command::Config(args) => f1::config(args),
        Command::Entries(args) => f1::entries(args),
        Command::Noise(args) => f1::noise(args, record_absolute_paths),
        Command::Ablate(args) => perf_cmd::ablate(args, record_absolute_paths),
        Command::Demo(args) => f1::demo(args, record_absolute_paths),
        Command::Judge(args) => judge_cmd::judge(args),
        Command::Review(args) => review_cmd::review(args),
        Command::Bisect(args) => s6::bisect(args),
        Command::Watch(args) => s6::watch(args),
        Command::Ask(args) => s6::ask(args),
        Command::Sequence {
            baseline_dir,
            capture_dir,
            pattern,
            out,
            threshold,
            metric,
            config,
            ppd,
            fail_on_new,
            allow_empty,
            labels,
            json,
            hdr,
            junit,
            meta,
            require,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(m) = metric {
                cfg.default_metric = m.into();
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            cfg.fail_on_new |= fail_on_new;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&baseline_dir, &capture_dir], &out)?;
            let report = saccade_core::sequence::run_sequence(
                &baseline_dir,
                &capture_dir,
                &out,
                &pattern,
                &cfg,
            )?;
            if let Some(path) = junit {
                f1::junit_reports(&out, &path, false)?;
            }
            match json {
                None => emit(&report.text())?,
                Some(mode) => {
                    let mut value = serde_json::to_value(match mode {
                        JsonMode::Lean => report.lean(),
                        JsonMode::Full => report.clone(),
                    })?;
                    if matches!(mode, JsonMode::Lean) {
                        agent::round_floats(&mut value);
                    }
                    emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
                }
            }
            Ok(u8::from(report.is_regression()))
        }
        Command::Rank {
            reference_dir,
            candidate_dirs,
            labels,
            metric,
            out,
            config,
            threshold,
            ppd,
            fail_on_new,
            allow_empty,
            json,
            hdr,
            junit,
            meta,
            require,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            cfg.fail_on_new |= fail_on_new;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(
                junit.as_deref(),
                &std::iter::once(reference_dir.as_path())
                    .chain(candidate_dirs.iter().map(PathBuf::as_path))
                    .collect::<Vec<_>>(),
                &out,
            )?;
            let report = saccade_core::rank::run_rank(
                &reference_dir,
                &candidate_dirs,
                labels.as_deref(),
                metric.into(),
                &out,
                &cfg,
            )?;
            if let Some(path) = junit {
                f1::junit_reports(&out, &path, true)?;
            }
            match json {
                None => emit(&report.text())?,
                Some(mode) => {
                    let mut value = serde_json::to_value(match mode {
                        JsonMode::Lean => report.lean(),
                        JsonMode::Full => report.clone(),
                    })?;
                    if matches!(mode, JsonMode::Lean) {
                        agent::round_floats(&mut value);
                    }
                    emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
                }
            }
            Ok(u8::from(report.is_regression()))
        }
        Command::Compare {
            baseline_dir,
            capture_dir,
            out,
            threshold,
            metric,
            config,
            fail_on_new,
            allow_empty,
            json,
            ppd,
            labels,
            hdr,
            junit,
            entries,
            meta,
            require,
            perf,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            perf.apply(&mut cfg.perf)?;
            cfg.entries = entries;
            cfg.allow_empty |= allow_empty;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&baseline_dir, &capture_dir], &out)?;
            if let Some(t) = threshold {
                cfg.default_threshold = t;
            }
            if let Some(m) = metric {
                cfg.default_metric = m.into();
            }
            if fail_on_new {
                cfg.fail_on_new = true;
            }
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            let report = saccade_core::run::run(&baseline_dir, &capture_dir, &out, &cfg)?;
            if let Some(path) = junit {
                saccade_core::ergonomics::junit(&report, &path)?;
            }
            emit_run(&report, &out, json)?;
            Ok(u8::from(report.is_regression()))
        }
        Command::Identity {
            parent_dir,
            candidate_dir,
            out,
            allow_empty,
            threshold,
            metric,
            config,
            json,
            ppd,
            labels,
            junit,
            entries,
            meta,
            require,
            perf,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            perf.apply(&mut cfg.perf)?;
            cfg.entries = entries;
            cfg.allow_empty |= allow_empty;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&parent_dir, &candidate_dir], &out)?;
            if config.is_none() && !cfg.overrides.is_empty() {
                // `./saccade.toml` is auto-loaded; it must not silently relax identity.
                eprintln!(
                    "saccade: note: ignoring [[override]] entries ({}) from the auto-loaded saccade.toml; \
                     pass --config to apply them to identity",
                    cfg.overrides
                        .iter()
                        .map(|o| format!("{:?}", o.glob))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                cfg.overrides.clear();
            } else if !cfg.overrides.is_empty() {
                eprintln!(
                    "saccade: note: applying [[override]] entries ({}) to identity because --config was given",
                    cfg.overrides
                        .iter()
                        .map(|o| format!("{:?}", o.glob))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            cfg.mode = Mode::Identity;
            cfg.labels = Labels {
                baseline: "parent".into(),
                capture: "candidate".into(),
            };
            // Identity defaults are strict; only explicit flags relax them.
            cfg.default_threshold = threshold.unwrap_or(0.0);
            cfg.default_metric = metric.map_or(Metric::Max, Into::into);
            if let Some(p) = ppd {
                cfg.pixels_per_degree = p;
            }
            if let Some(l) = labels {
                cfg.labels = parse_labels(&l)?;
            }
            let report = saccade_core::run::run(&parent_dir, &candidate_dir, &out, &cfg)?;
            if let Some(path) = junit {
                saccade_core::ergonomics::junit(&report, &path)?;
            }
            emit_run(&report, &out, json)?;
            Ok(u8::from(report.is_regression()))
        }
        Command::Approve {
            capture_dir,
            baseline_dir,
            names,
            report,
            all_failing,
            decisions,
            include_errors,
            prune_missing,
            json,
            force,
        } => {
            let report_path = report.or_else(|| {
                all_failing
                    .as_ref()
                    .filter(|p| p.as_os_str() != "__report__")
                    .cloned()
            });
            if all_failing
                .as_ref()
                .is_some_and(|p| p.as_os_str() == "__report__")
                && report_path.is_none()
            {
                return Err(CliError::usage(
                    "--all-failing needs --report REPORT_JSON or a legacy REPORT_JSON value",
                ));
            }
            if (include_errors || prune_missing) && report_path.is_none() {
                return Err(CliError::usage(
                    "--include-errors and --prune-missing need --report REPORT_JSON or --all-failing REPORT_JSON",
                ));
            }
            let (capture, baseline) = f1::approve_dirs(
                capture_dir,
                baseline_dir,
                report_path.as_deref(),
                decisions.as_deref(),
            )?;
            approve(
                &capture,
                &baseline,
                names,
                report_path.as_deref(),
                decisions.as_deref(),
                ApproveFlags {
                    include_errors,
                    prune_missing,
                    json,
                    force,
                    record_absolute_paths,
                },
            )
        }
        Command::Serve {
            mut roots,
            follow_symlinks_within_roots,
            symlink_targets,
            fs_timeout_ms,
            port,
            cache_dir,
            decisions_dir,
            config,
            ppd,
            open,
            hdr,
            meta,
            perf,
        } => {
            let loaded = load_config(config.as_deref())?;
            let mut opts = saccade_core::serve::ServeOptions::new(roots.remove(0));
            opts.extra_roots = roots;
            opts.follow_symlinks_within_roots = follow_symlinks_within_roots;
            opts.symlink_targets = loaded.symlink_targets;
            opts.symlink_targets.extend(symlink_targets);
            opts.fs_timeout_ms = fs_timeout_ms.unwrap_or(loaded.fs_timeout_ms);
            opts.port = port;
            if let Some(d) = cache_dir {
                opts.cache_dir = d;
            }
            if let Some(d) = decisions_dir {
                opts.decisions_dir = d;
            }
            opts.view.record_absolute_paths = record_absolute_paths;
            opts.view.regions = loaded.regions;
            opts.view.meta = loaded.meta;
            opts.view.diagnostics = loaded.diagnostics;
            opts.view.perf = loaded.perf;
            perf.apply(&mut opts.view.perf)?;
            hdr.apply(&mut opts.view.hdr)?;
            meta.apply(&mut opts.view.meta);
            if let Some(p) = ppd {
                opts.view.pixels_per_degree = p;
            }
            let handle = saccade_core::serve::start(opts)?;
            let url = format!("http://127.0.0.1:{}/", handle.port());
            emit(&format!(
                "saccade serve: {url}\n  cache:     {}\n  decisions: {}\n  (Ctrl-C to stop)\n",
                escape_control(&handle.cache_dir().display().to_string()),
                escape_control(&handle.decisions_dir().display().to_string())
            ))?;
            if open {
                open_browser(&url);
            }
            handle.wait();
            Ok(0)
        }
        Command::Unblind {
            decisions_json,
            blind_key_json,
            out,
        } => {
            let decisions = read_decisions(&decisions_json)?;
            let key = read_blind_key(&blind_key_json)?;
            let mut resolved = unblind(&decisions, &key)?;
            let source = key
                .view_dir
                .as_deref()
                .map(|d| saccade_core::paths::resolve(d, &blind_key_json).join("index.html"))
                .unwrap_or_else(|| blind_key_json.clone());
            let destination = out.as_deref().unwrap_or(&decisions_json);
            saccade_core::paths::rebase_decisions(
                &mut resolved,
                &source,
                destination,
                record_absolute_paths,
            );
            let text = serde_json::to_string_pretty(&resolved)?;
            match out {
                Some(path) => std::fs::write(&path, format!("{text}\n"))
                    .map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))?,
                None => emit(&format!("{text}\n"))?,
            }
            Ok(0)
        }
        Command::View {
            dirs,
            labels,
            reference,
            blind,
            seed,
            key_out,
            out,
            ppd,
            config,
            json,
            hdr,
            entries,
            meta,
            perf,
        } => {
            let loaded = load_config(config.as_deref())?;
            let mut opts = ViewOptions {
                entries,
                record_absolute_paths,
                regions: loaded.regions,
                meta: loaded.meta,
                diagnostics: loaded.diagnostics,
                perf: loaded.perf,
                labels,
                reference,
                blind,
                seed,
                key_out: key_out.clone(),
                ..ViewOptions::default()
            };
            perf.apply(&mut opts.perf)?;
            hdr.apply(&mut opts.hdr)?;
            meta.apply(&mut opts.meta);
            if let Some(p) = ppd {
                opts.pixels_per_degree = p;
            }
            let model = build_view(&dirs, &out, &opts)?;
            let abs = |p: &Path| PathBuf::from(saccade_core::paths::cwd(p, record_absolute_paths));
            let key = key_out.unwrap_or_else(|| out.join(saccade_core::view::BLIND_KEY_FILE));
            if json {
                let value = serde_json::json!({
                    "schema": "saccade-view-summary.v1",
                    "index_html": abs(&out.join("index.html")).display().to_string(),
                    "out_dir": abs(&out).display().to_string(),
                    "sets": model.sets.len(),
                    "set_names": model.sets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                    "directories": model.labels.len(),
                    // A blind view's labels stay out of the output.
                    "labels": (!blind).then(|| model.labels.clone()),
                    "blind": blind,
                    // A blind page's token is not the shuffle seed; the key has both.
                    "seed": (!blind).then_some(model.seed),
                    "blind_key": blind.then(|| abs(&key).display().to_string()),
                });
                emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
                return Ok(0);
            }
            emit(&format!(
                "wrote {} ({} image sets, {} directories)\n",
                escape_control(&out.join("index.html").display().to_string()),
                model.sets.len(),
                model.labels.len()
            ))?;
            if blind {
                emit(&format!(
                    "blind key (keep it away from the judge): {}\n",
                    escape_control(&key.display().to_string())
                ))?;
            }
            Ok(0)
        }
        Command::Runs {
            ref_dir,
            run_dirs,
            labels,
            pair_by_position,
            json,
            out,
            ppd,
            config,
            hdr,
            entries,
            meta,
            perf,
        } => {
            let loaded = load_config(config.as_deref())?;
            let mut opts = saccade_core::runs::RunsOptions {
                entries,
                meta: loaded.meta,
                perf: loaded.perf,
                ..saccade_core::runs::RunsOptions::default()
            };
            perf.apply(&mut opts.perf)?;
            hdr.apply(&mut opts.hdr)?;
            meta.apply(&mut opts.meta);
            if let Some(p) = ppd {
                opts.pixels_per_degree = p;
            }
            runs_cmd::run(&runs_cmd::RunsRequest {
                ref_dir,
                run_dirs,
                labels,
                json,
                out,
                by_position: pair_by_position,
                opts,
                record_absolute_paths,
            })
        }
        Command::Mcp { root, watch } => {
            mcp::serve_stdio(root.as_deref(), &watch)?;
            Ok(0)
        }
        Command::Explain {
            report_json,
            out,
            top,
            pad,
            stretch,
            blind,
            key_out,
            seed,
            hotspot_min_share,
            entries,
            json,
        } => {
            let out = out.unwrap_or_else(|| {
                report_json
                    .parent()
                    .map_or_else(|| PathBuf::from("explain"), |p| p.join("explain"))
            });
            let opts = saccade_core::explain::ExplainOptions {
                top,
                pad,
                stretch,
                blind,
                seed,
                entries,
                key_out: key_out.clone(),
                hotspot_min_share,
                record_absolute_paths,
            };
            if let Some(key) = &key_out {
                saccade_core::explain::check_key_out(&out, key)
                    .map_err(|e| CliError::new("unsafe_path", strip_config_prefix(&e)))?;
            }
            let pack = saccade_core::explain::explain(&report_json, &out, &opts)?;
            if json {
                let text = serde_json::to_string_pretty(&pack)?;
                emit(&format!("{text}\n"))?;
            } else {
                let md = std::fs::read_to_string(out.join(saccade_core::explain::EXPLAIN_MD_FILE))
                    .map_err(|e| CliError::io(format!("reading explain.md: {e}")))?;
                emit(&md)?;
                emit(&format!(
                    "\npack: {}\n",
                    escape_control(&out.display().to_string())
                ))?;
                if let Some(key) = &key_out {
                    emit(&format!(
                        "blind key (keep it away from the judge): {}\n",
                        escape_control(&key.display().to_string())
                    ))?;
                }
            }
            Ok(0)
        }
        Command::Snapshot(args) => agent_ui::snapshot(&args),
        Command::DecisionRequest(args) => agent_ui::decision_request(&args),
        Command::Decide(args) => agent_ui::decide(&args),
        Command::Summary {
            report_json,
            format,
            artifact_url,
            comment_key,
        } => {
            if comment_key
                .as_deref()
                .is_some_and(|k| !is_valid_comment_key(k))
            {
                return Err(CliError::usage(
                    "--comment-key must be 1-64 characters of A-Z a-z 0-9 . _ -",
                ));
            }
            let report = read_report(&report_json)?;
            match format {
                Format::Markdown => {
                    let mut text = render_markdown(
                        &report,
                        &MarkdownOptions {
                            artifact_url,
                            comment_key,
                            max_bytes: None,
                        },
                    );
                    let review = report_json
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join("saccade-review.md");
                    if review.is_file() {
                        text.push_str(
                            &std::fs::read_to_string(&review).map_err(|e| {
                                CliError::io(format!("reading review summary: {e}"))
                            })?,
                        );
                    }
                    emit(&text)?;
                }
                Format::Text => emit(&text_table(&report))?,
                Format::Json => {
                    let value =
                        agent::summary_value(&report, &report_json, agent::DEFAULT_TOP_FAILING);
                    let text = serde_json::to_string_pretty(&value)?;
                    emit(&format!("{text}\n"))?;
                }
            }
            Ok(0)
        }
    }
}

/// Parses `--labels a,b` into the two side names.
fn parse_labels(parts: &[String]) -> Result<Labels, String> {
    match parts {
        [a, b] if !a.trim().is_empty() && !b.trim().is_empty() => Ok(Labels {
            baseline: a.trim().to_string(),
            capture: b.trim().to_string(),
        }),
        _ => Err("--labels takes exactly two non-empty names, `A,B`".into()),
    }
}

fn load_config(explicit: Option<&Path>) -> Result<RunConfig, CliError> {
    let path = match explicit {
        Some(p) => Some(p),
        None => Some(Path::new("saccade.toml")).filter(|p| p.is_file()),
    };
    match path {
        Some(p) => Ok(RunConfig::from_toml_file(p)?),
        None => Ok(RunConfig::default()),
    }
}

/// The message of a configuration error without its "invalid configuration: " prefix.
fn strip_config_prefix(e: &saccade_core::Error) -> String {
    let text = e.to_string();
    text.strip_prefix("invalid configuration: ")
        .map_or_else(|| text.clone(), str::to_string)
}

/// Opens `url` in the default browser; failures are ignored.
fn open_browser(url: &str) {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn read_report(path: &Path) -> Result<Report, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::io(format!("reading report {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::io(format!("parsing report {}: {e}", path.display())))
}

/// Writes `text` to stdout. A closed pipe (for example `| head`) is not an error.
fn emit(text: &str) -> Result<(), CliError> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(CliError::io(format!("writing to stdout: {e}"))),
    }
}

/// Escapes control characters (newline, escape, ...) so a file name or error
/// text cannot forge terminal output or workflow commands.
fn escape_control(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

/// Like [`escape_control`] but keeps line breaks, so multi-line errors (a TOML
/// parse error with its source excerpt) stay readable. Every continuation line
/// is indented, so file-controlled text cannot start a line with a workflow
/// command such as `::error::`.
fn escape_multiline(s: &str) -> String {
    s.lines()
        .map(escape_control)
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// Refuses a destination whose existing components below `baseline_dir` include a symlink.
fn check_no_symlinks(baseline_dir: &Path, rel: &Path) -> Result<(), CliError> {
    let mut cur = baseline_dir.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CliError::new(
                    "unsafe_path",
                    format!("refusing to write through symlink {}", cur.display()),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(CliError::io(format!("inspecting {}: {e}", cur.display()))),
        }
    }
    Ok(())
}

/// The switches of `approve`.
struct ApproveFlags {
    include_errors: bool,
    prune_missing: bool,
    json: bool,
    force: bool,
    record_absolute_paths: bool,
}

/// Whether `recorded` (a path stored in a report or decisions file) and
/// `given` (a path on the command line) name the same directory.
fn same_dir(recorded: &str, given: &Path) -> bool {
    saccade_core::run::normalise_path(Path::new(recorded))
        == saccade_core::run::normalise_path(given)
}

/// Checks the directories recorded in a report against the ones `approve` was
/// given. A report from before directories were recorded cannot be checked.
fn bind_report_dirs(report: &Report, capture_dir: &Path, baseline_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for (what, recorded, given) in [
        ("capture", report.capture_dir.as_deref(), capture_dir),
        ("baseline", report.baseline_dir.as_deref(), baseline_dir),
    ] {
        match recorded {
            Some(r) if !same_dir(r, given) => problems.push(format!(
                "the report compared the {what} directory {r}, not {}",
                given.display()
            )),
            Some(_) => {}
            None => eprintln!(
                "saccade: warning: the report records no {what} directory, so it cannot be checked against {}",
                given.display()
            ),
        }
    }
    problems
}

/// Checks a decisions file against the directories `approve` was given and
/// records, per accepted set, the capture hash that was judged.
fn bind_decisions(
    d: &saccade_core::view::Decisions,
    capture_dir: &Path,
    baseline_dir: &Path,
    expected_capture: &mut BTreeMap<String, String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    if d.dirs.is_empty() {
        eprintln!(
            "saccade: warning: the decisions file records no directories, so it cannot be checked against {} and {}",
            capture_dir.display(),
            baseline_dir.display()
        );
        return problems;
    }
    let find = |dir: &Path| d.dirs.iter().position(|r| same_dir(r, dir));
    let cap_idx = find(capture_dir);
    for (what, dir, idx) in [
        ("capture", capture_dir, cap_idx),
        ("baseline", baseline_dir, find(baseline_dir)),
    ] {
        if idx.is_none() {
            problems.push(format!(
                "the {what} directory {} is not one of the directories the decisions were made on ({})",
                dir.display(),
                d.dirs.join(", ")
            ));
        }
    }
    for set in d
        .sets
        .iter()
        .filter(|s| s.decision == Some(Verdict::Accept))
    {
        if let Some(chosen) = set
            .chosen_dir
            .as_deref()
            .filter(|c| !same_dir(c, capture_dir))
        {
            problems.push(format!(
                "{}: the judge preferred {chosen}, which is not the capture directory",
                set.name
            ));
        }
        let Some(idx) = cap_idx else { continue };
        match set.sha256.get(idx) {
            Some(Some(hash)) => {
                expected_capture.insert(set.name.clone(), hash.clone());
            }
            _ if set.sha256.is_empty() => {}
            _ => problems.push(format!(
                "{}: the capture directory had no such image when it was judged",
                set.name
            )),
        }
    }
    problems
}

/// Compares recorded file hashes with the files on disk (only for names that
/// are about to be touched), returning one problem per difference.
fn check_hashes(dir: &Path, what: &str, expected: &BTreeMap<String, String>) -> Vec<String> {
    expected
        .iter()
        .filter_map(|(name, recorded)| {
            let path = dir.join(name);
            if !path.is_file() {
                return None;
            }
            match saccade_core::run::sha256_file(&path) {
                Ok(actual) if actual == *recorded => None,
                Ok(_) => Some(format!(
                    "{name}: the {what} file changed since it was reviewed"
                )),
                Err(e) => Some(format!("{name}: {e}")),
            }
        })
        .collect()
}

fn approve(
    capture_dir: &Path,
    baseline_dir: &Path,
    mut names: Vec<String>,
    all_failing: Option<&Path>,
    decisions: Option<&Path>,
    flags: ApproveFlags,
) -> Result<u8, CliError> {
    let ApproveFlags {
        include_errors,
        prune_missing,
        json,
        force,
        record_absolute_paths,
    } = flags;
    let mut copied: Vec<serde_json::Value> = Vec::new();
    let mut pruned: Vec<String> = Vec::new();
    let mut prune: Vec<String> = Vec::new();
    // What the report or decisions recorded about the files being approved.
    let mut problems: Vec<String> = Vec::new();
    let mut expected_capture: BTreeMap<String, String> = BTreeMap::new();
    let mut expected_baseline: BTreeMap<String, String> = BTreeMap::new();
    if let Some(path) = decisions {
        let mut d = read_decisions(path)?;
        for dir in &mut d.dirs {
            *dir = saccade_core::paths::resolve(dir, path)
                .display()
                .to_string();
        }
        for set in &mut d.sets {
            if let Some(dir) = &mut set.chosen_dir {
                *dir = saccade_core::paths::resolve(dir, path)
                    .display()
                    .to_string();
            }
        }
        if d.blind {
            return Err(CliError::new(
                "approve_mismatch",
                format!(
                    "{} is a blind decisions file: its labels and directories are still hidden. \
                     Run `saccade unblind` on it first and approve the unblinded file",
                    path.display()
                ),
            ));
        }
        problems.extend(bind_decisions(
            &d,
            capture_dir,
            baseline_dir,
            &mut expected_capture,
        ));
        names.extend(d.accepted());
    }
    if let Some(path) = all_failing {
        let mut report = read_report(path)?;
        for dir in [&mut report.baseline_dir, &mut report.capture_dir]
            .into_iter()
            .flatten()
        {
            *dir = saccade_core::paths::resolve(dir, path)
                .display()
                .to_string();
        }
        problems.extend(bind_report_dirs(&report, capture_dir, baseline_dir));
        if prune_missing {
            prune = report
                .entries
                .iter()
                .filter(|e| e.status == Status::Missing)
                .map(|e| e.name.clone())
                .collect();
            prune.sort();
            prune.dedup();
            for e in report
                .entries
                .iter()
                .filter(|e| e.status == Status::Missing)
            {
                if let Some(h) = &e.baseline_sha256 {
                    expected_baseline.insert(e.name.clone(), h.clone());
                }
            }
        }
        let chosen: Vec<_> = report
            .entries
            .into_iter()
            .filter(|e| match e.status {
                Status::Fail | Status::New => true,
                Status::Error => {
                    include_errors
                        && e.paths.capture.is_some()
                        && is_safe_name(&e.name)
                        && saccade_core::run::is_decodable(&capture_dir.join(&e.name))
                }
                _ => false,
            })
            .collect();
        for e in &chosen {
            if let Some(h) = &e.capture_sha256 {
                expected_capture.insert(e.name.clone(), h.clone());
            }
            if let Some(h) = &e.baseline_sha256 {
                expected_baseline.insert(e.name.clone(), h.clone());
            }
        }
        names.extend(chosen.into_iter().map(|e| e.name));
    } else if names.is_empty() && decisions.is_none() {
        return Err(CliError::usage(
            "name at least one image, or pass --all-failing <REPORT_JSON> or --decisions <FILE>",
        ));
    }
    names.sort();
    names.dedup();
    // Validate every name before copying anything.
    for name in &names {
        if !is_safe_name(name) {
            return Err(CliError::new(
                "unsafe_path",
                format!("unsafe image name {name:?}"),
            ));
        }
        let src = capture_dir.join(name);
        if !src.is_file() {
            return Err(CliError::io(format!(
                "capture {} does not exist",
                src.display()
            )));
        }
        check_no_symlinks(baseline_dir, Path::new(name))?;
    }
    for name in &prune {
        if !is_safe_name(name) {
            return Err(CliError::new(
                "unsafe_path",
                format!("unsafe image name {name:?}"),
            ));
        }
        check_no_symlinks(baseline_dir, Path::new(name))?;
    }
    // Only files about to be copied over or removed are checked.
    expected_capture.retain(|n, _| names.contains(n));
    expected_baseline.retain(|n, _| names.contains(n) || prune.contains(n));
    problems.extend(check_hashes(capture_dir, "capture", &expected_capture));
    problems.extend(check_hashes(baseline_dir, "baseline", &expected_baseline));
    if !problems.is_empty() {
        if !force {
            return Err(CliError::new(
                "approve_mismatch",
                format!(
                    "refusing to approve what was not reviewed: {}; pass --force to override",
                    problems.join("; ")
                ),
            ));
        }
        for p in &problems {
            eprintln!("saccade: warning: --force overrides: {}", escape_control(p));
        }
    }
    for name in &names {
        let rel = Path::new(name);
        let src = capture_dir.join(rel);
        let dest = baseline_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::io(format!("creating {}: {e}", parent.display())))?;
        }
        std::fs::copy(&src, &dest).map_err(|e| {
            CliError::io(format!(
                "copying {} to {}: {e}",
                src.display(),
                dest.display()
            ))
        })?;
        if json {
            copied.push(serde_json::json!({
                "name": name,
                "from": saccade_core::paths::cwd(&src, record_absolute_paths),
                "to": saccade_core::paths::cwd(&dest, record_absolute_paths),
            }));
        } else {
            emit(&format!(
                "{} -> {}\n",
                escape_control(&src.display().to_string()),
                escape_control(&dest.display().to_string())
            ))?;
        }
    }
    for name in &prune {
        let target = baseline_dir.join(name);
        // The report says `missing`; only delete when the capture is still absent.
        if capture_dir.join(name).exists() || !target.is_file() {
            continue;
        }
        std::fs::remove_file(&target)
            .map_err(|e| CliError::io(format!("removing {}: {e}", target.display())))?;
        if json {
            pruned.push(saccade_core::paths::cwd(&target, record_absolute_paths));
        } else {
            emit(&format!(
                "removed {}\n",
                escape_control(&target.display().to_string())
            ))?;
        }
    }
    if json {
        let value = serde_json::json!({
            "schema": "saccade-approve.v1",
            "copied": copied,
            "pruned": pruned,
        });
        let text = serde_json::to_string_pretty(&value)?;
        emit(&format!("{text}\n"))?;
    }
    Ok(0)
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "FAIL",
        Status::New => "new",
        Status::Missing => "MISSING",
        Status::Error => "ERROR",
    }
}

fn metric_label(m: Metric) -> &'static str {
    match m {
        Metric::Mean => "mean",
        Metric::P95 => "p95",
        Metric::P99 => "p99",
        Metric::Max => "max",
    }
}

/// Aligned plain-text table, non-pass rows first (then by name).
fn text_table(report: &Report) -> String {
    let mut rows: Vec<&saccade_core::Entry> = report.entries.iter().collect();
    rows.sort_by_key(|e| (e.status == Status::Pass, e.name.clone()));
    let mut cells: Vec<[String; 5]> = vec![[
        "STATUS".into(),
        "NAME".into(),
        "METRIC".into(),
        "VALUE".into(),
        "THRESHOLD".into(),
    ]];
    // Everything that is not a table column goes on `↳` lines under its row.
    let mut notes: Vec<Vec<String>> = vec![Vec::new()];
    for e in rows {
        let mut lines = Vec::new();
        if let Some(d) = &e.diagnostics {
            if e.status == Status::Pass {
                // A passing pair says something only when timings come with it.
                if !d.perf.is_empty() || !d.perf_not_comparable.is_empty() {
                    lines.push(escape_control(&d.verdict_line(e.bit_identical)));
                }
            } else {
                lines.push(format!(
                    "{}: {}",
                    d.class.as_str(),
                    escape_control(&d.description)
                ));
                lines.extend(
                    d.perf_summary()
                        .map(|p| format!("perf: {}", escape_control(&p))),
                );
            }
        }
        if e.status == Status::Fail {
            lines.extend(saccade_core::hotspots::summary_line(&e.hotspots));
        }
        if let Some(note) = e.local_hotspot_note() {
            lines.push(note);
        }
        if let Some(msg) = &e.error {
            lines.push(format!("error: {}", escape_control(msg)));
        }
        // An undeclared-difference error already names the keys.
        let error_names_keys = e
            .error
            .as_deref()
            .is_some_and(|m| m.contains("configuration differs"));
        if !e.meta_diff.is_empty() && !error_names_keys {
            let keys: Vec<&str> = e.meta_diff.iter().map(|d| d.key.as_str()).collect();
            lines.push(format!(
                "config differs: {}",
                escape_control(&keys.join(", "))
            ));
        }
        for w in &e.warnings {
            lines.push(format!("warning: {}", escape_control(w)));
        }
        notes.push(lines);
        let (value, threshold) = match e.status {
            Status::Pass | Status::Fail => (
                e.value.map_or("-".into(), |v| format!("{v:.5}")),
                format!("{}", e.threshold),
            ),
            _ => ("-".into(), "-".into()),
        };
        cells.push([
            status_label(e.status).into(),
            escape_control(&e.name),
            e.buffer.as_ref().map_or_else(
                || metric_label(e.metric_used).to_string(),
                |b| format!("{} ({})", metric_label(b.metric), b.unit),
            ),
            value,
            threshold,
        ]);
    }
    let mut widths = [0usize; 5];
    for row in &cells {
        for (w, c) in widths.iter_mut().zip(row) {
            *w = (*w).max(c.chars().count());
        }
    }
    let mut out = String::new();
    for (row, note) in cells.iter().zip(&notes) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
        for note in note {
            out.push_str(&format!("  ↳ {note}\n"));
        }
    }
    if let Some(headline) = saccade_core::render::identity_headline(report) {
        out.insert_str(0, &format!("{}\n\n", escape_control(&headline)));
    }
    if let Some(v) = &report.combined_verdict {
        out.push_str(&format!("\n{}\n", escape_control(v)));
    }
    if let Some(d) = &report.perf_diff {
        out.push_str(&format!("{}\n", escape_control(&d.summary(3))));
        for w in &d.warnings {
            out.push_str(&format!("warning: {}\n", escape_control(w)));
        }
    }
    let t = &report.totals;
    out.push_str(&format!(
        "\n{} fail, {} error, {} missing, {} new, {} pass ({} total)\n",
        t.fail, t.error, t.missing, t.new, t.pass, t.total
    ));
    if report.is_empty_run() {
        out.push_str(
            "nothing compared: no image exists in both directories (--allow-empty accepts this)\n",
        );
    }
    out
}
