//! Twelve top-level commands for local measurement and evidence workflows.
//!
//! Exit codes: `0` no regression, `1` regression, `2` usage/config/IO error.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use saccade_core::config::RunConfig;
use saccade_core::report::{Labels, Metric, Mode, Report, Status};
use saccade_core::view::{ViewOptions, build_view, is_safe_name};

mod agent;
mod agent_ui;
mod approval;
mod f1;
mod local_cmd;
#[cfg(feature = "ai")]
mod review_cmd;

#[cfg(feature = "mcp")]
mod mcp;
mod outdirs;
mod perf_cmd;
#[cfg(feature = "prechecks")]
mod precheck;
#[cfg(all(feature = "prechecks", feature = "mcp"))]
mod precheck_mcp;

#[cfg(feature = "graphics")]
mod s6;

use agent::CliError;

#[derive(Parser)]
#[command(
    name = "saccade",
    version,
    disable_help_subcommand = true,
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

/// Structured intent and predeclared changes bound into local evidence.
#[derive(clap::Args, Clone, Default)]
struct IntentArgs {
    #[arg(long, conflicts_with = "intent_file")]
    intent: Option<String>,
    #[arg(long)]
    intent_file: Option<PathBuf>,
    #[arg(long)]
    changes_file: Option<PathBuf>,
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

#[derive(Subcommand)]
enum Command {
    /// Bootstrap a commented configuration and print baseline adoption steps.
    Init(f1::InitArgs),
    /// Run the bundled example and explain its expected regression.
    Demo(f1::DemoArgs),
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
        /// Print a bounded machine-readable result.
        #[arg(long)]
        json: bool,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Display names of the two sides, `baseline,capture`.
        #[arg(long, value_delimiter = ',', value_name = "A,B")]
        labels: Option<Vec<String>>,
        #[command(flatten)]
        hdr: HdrArgs,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long = "entry", value_name = "GLOB")]
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
        #[command(flatten)]
        intent: IntentArgs,
    },
    /// Establish exact native decoded-sample equality in the selected scope.
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
        /// Rejected for identity; use compare for perceptual thresholds.
        #[arg(long)]
        threshold: Option<f64>,
        /// Rejected for identity; use compare for perceptual metrics.
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./saccade.toml when it exists.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print a bounded machine-readable result.
        #[arg(long)]
        json: bool,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Display names of the two sides, `parent,candidate`.
        #[arg(long, value_delimiter = ',', value_name = "A,B")]
        labels: Option<Vec<String>>,
        /// Include only matching names (repeatable; union of globs).
        #[arg(long = "entry", value_name = "GLOB")]
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
        #[command(flatten)]
        intent: IntentArgs,
    },
    /// Calibrate thresholds from repeated captures of an unchanged build.
    Noise(f1::NoiseArgs),
    /// Write a self-contained review viewer for 2 to 6 image directories.
    View {
        /// Directories to compare, paired by relative image path (2 to 6).
        #[arg(num_args = 1..=6, required_unless_present = "unblind")]
        dirs: Vec<PathBuf>,
        /// Resolve recorded anonymous choices after review.
        #[arg(long, requires = "key", conflicts_with = "dirs")]
        unblind: Option<PathBuf>,
        #[arg(long, requires = "unblind")]
        key: Option<PathBuf>,
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
        #[arg(long = "entry", value_name = "GLOB")]
        entries: Vec<String>,
        #[command(flatten)]
        meta: MetaArgs,
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
        /// Select a report entry without positional directories; repeatable.
        #[arg(long = "entry", value_name = "NAME")]
        entries: Vec<String>,
        /// Also approve every fail and new entry of this report JSON.
        #[arg(long, value_name = "REPORT_JSON", num_args = 0..=1, default_missing_value = "__report__")]
        all_failing: Option<PathBuf>,
        /// Explicit canonical CLI decision bound to this report, inputs and scope.
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
        /// Removed: stale reviewed content cannot be overridden.
        #[arg(long)]
        force: bool,
        /// Prepare a selected update manifest and unattested CLI decision draft.
        #[arg(long)]
        dry_run: bool,
        /// Empty directory for the plan, decision and applied receipt.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// (127.0.0.1 only; the archive is never written to).
    #[cfg(feature = "workbench")]
    Serve {
        /// Archive roots to browse (read-only). With several, each is a
        /// top-level entry named after its directory.
        #[arg(num_args = 1..)]
        roots: Vec<PathBuf>,
        /// Additional read-only archive roots (repeatable).
        #[arg(long = "root")]
        registered_roots: Vec<PathBuf>,
        /// Explicit generated-artifact root.
        #[arg(long)]
        out_root: Option<PathBuf>,
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
    /// passes must resolve under `--root`.
    #[cfg(feature = "mcp")]
    Mcp {
        /// Read-only roots (repeatable).
        #[arg(long = "root", required = true)]
        roots: Vec<PathBuf>,
        /// Generated artifacts require this separate root.
        #[arg(long)]
        out_root: Option<PathBuf>,
        #[arg(long)]
        follow_symlinks_within_roots: bool,
        #[arg(long = "symlink-target")]
        symlink_targets: Vec<PathBuf>,
        #[cfg(feature = "ai")]
        #[command(flatten)]
        providers: review_cmd::Startup,
    },
    /// Read, explain, prepare or export existing evidence.
    Inspect(local_cmd::InspectArgs),
    /// Preview a review plan or handle a local closed decision request.
    Review(local_cmd::ReviewArgs),
    /// Analyze existing graphics captures.
    Experiment {
        #[command(subcommand)]
        operation: ExperimentOperation,
    },
}

#[derive(Subcommand)]
enum ExperimentOperation {
    /// Compare ablation arms against a base with image and performance evidence.
    #[cfg(feature = "graphics")]
    Ablate(perf_cmd::AblateArgs),
    /// Compare numbered colour frames by sorted index and measure added flicker.
    #[cfg(feature = "graphics")]
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
        #[arg(long)]
        json: bool,
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
    #[cfg(feature = "graphics")]
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
        #[arg(long)]
        json: bool,
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
    /// Find the first diverging run or revision in an ordered series.
    #[cfg(feature = "graphics")]
    Bisect(s6::BisectArgs),
    /// Photosensitivity PRE-CHECK only; not certification or formal compliance.
    #[cfg(feature = "prechecks")]
    Safety(precheck::SafetyArgs),
    /// Accessibility PRE-CHECK only; not certification or formal compliance.
    #[cfg(feature = "prechecks")]
    A11y(precheck::A11yArgs),
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
    if let Some(message) = local_cmd::migration(&args) {
        let err = CliError::new("interface_removed", message);
        if args_want_json(&args) {
            emit_json_error(&err);
        } else {
            eprintln!("saccade: {err}");
        }
        return ExitCode::from(2);
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            if e.use_stderr()
                && let Some(feature) = requested_feature(&args)
            {
                let err = CliError::from(saccade_core::Error::FeatureUnavailable { feature });
                if args_want_json(&args) {
                    emit_json_error(&err);
                } else {
                    eprintln!("saccade: error: {err}\n  hint: {}", err.hint);
                }
                return ExitCode::from(2);
            }
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
fn emit_run(report: &Report, out: &Path, json: bool) -> Result<(), CliError> {
    if json {
        let mut value = agent::result_value(
            report,
            &out.join(saccade_core::report::REPORT_FILE_NAME),
            agent::DEFAULT_TOP_FAILING,
            false,
        );
        // SPEC-r5 explicitly freezes the identity discriminator parsed by Moss.
        // The payload is the bounded shared envelope; no old-format mode exists.
        if report.config.mode == Mode::Identity {
            value["schema"] = serde_json::json!("saccade-result.v1");
        }
        emit(&format!("{}\n", serde_json::to_string(&value)?))
    } else {
        emit(&text_table(report))?;
        if report.config.mode == Mode::Identity
            && (!report.config.entries.is_empty() || !report.config.ignore.is_empty())
        {
            emit(&format!(
                "scope: selected {:?}; excluded {:?}\n",
                report.config.entries, report.config.ignore
            ))?;
        }
        Ok(())
    }
}

fn dispatch(command: Command, record_absolute_paths: bool) -> Result<u8, CliError> {
    match command {
        Command::Inspect(args) => local_cmd::inspect(args, record_absolute_paths),
        Command::Review(args) => local_cmd::review(args, record_absolute_paths),
        #[cfg(feature = "prechecks")]
        Command::Experiment {
            operation: ExperimentOperation::Safety(args),
        } => precheck::safety(args),
        #[cfg(feature = "prechecks")]
        Command::Experiment {
            operation: ExperimentOperation::A11y(args),
        } => precheck::a11y(args),
        Command::Init(args) => f1::init(args),
        Command::Noise(args) => f1::noise(args, record_absolute_paths),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: ExperimentOperation::Ablate(args),
        } => perf_cmd::ablate(args, record_absolute_paths),
        Command::Demo(args) => f1::demo(args, record_absolute_paths),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: ExperimentOperation::Bisect(args),
        } => s6::bisect(args),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                ExperimentOperation::Sequence {
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
                },
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
            if json {
                let value = local_cmd::analysis_result(&serde_json::to_value(&report)?, &out)?;
                emit(&format!(
                    "{}
",
                    serde_json::to_string(&value)?
                ))?;
            } else {
                emit(&report.text())?;
            }
            Ok(u8::from(report.is_regression()))
        }
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                ExperimentOperation::Rank {
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
                },
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
            if json {
                let value = local_cmd::analysis_result(&serde_json::to_value(&report)?, &out)?;
                emit(&format!(
                    "{}
",
                    serde_json::to_string(&value)?
                ))?;
            } else {
                emit(&report.text())?;
            }
            Ok(
                if report.common_images != report.reference_images
                    || report
                        .overall
                        .iter()
                        .any(|c| c.totals.error > 0 || c.totals.missing > 0 || c.totals.new > 0)
                {
                    2
                } else {
                    0
                },
            )
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
            intent,
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
            local_cmd::persist_case(
                &report,
                &out.join(saccade_core::report::REPORT_FILE_NAME),
                &intent,
            )?;
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
            intent,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            cfg.record_absolute_paths = record_absolute_paths;
            perf.apply(&mut cfg.perf)?;
            cfg.entries = entries;
            cfg.allow_empty |= allow_empty;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            f1::guard_junit(junit.as_deref(), &[&parent_dir, &candidate_dir], &out)?;
            if threshold.is_some() || metric.is_some() {
                return Err(CliError::usage(
                    "identity rejects --threshold and --metric; use compare for perceptual thresholds",
                ));
            }
            cfg.mode = Mode::Identity;
            cfg.labels = Labels {
                baseline: "parent".into(),
                capture: "candidate".into(),
            };
            // Identity is always exact; metrics describe differences only.
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
            local_cmd::persist_case(
                &report,
                &out.join(saccade_core::report::REPORT_FILE_NAME),
                &intent,
            )?;
            emit_run(&report, &out, json)?;
            Ok(u8::from(report.is_regression()))
        }
        Command::Approve {
            capture_dir,
            baseline_dir,
            mut names,
            entries,
            report,
            all_failing,
            decisions,
            include_errors,
            prune_missing,
            json,
            force,
            dry_run,
            out,
        } => {
            names.extend(entries);
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
            for name in &names {
                if !is_safe_name(name) {
                    return Err(CliError::new(
                        "unsafe_path",
                        format!("unsafe image name {name:?}"),
                    ));
                }
            }
            if force {
                return Err(CliError::new(
                    "approve_mismatch",
                    "--force was removed; stale content must be reviewed again",
                ));
            }
            let report_path = report_path.ok_or_else(|| CliError::new("approve_mismatch", "approval requires --report and a canonical --decisions file; legacy finals must be reviewed again"))?;
            let (capture, baseline) =
                f1::approve_dirs(capture_dir, baseline_dir, Some(&report_path), None)?;
            approval::run(
                &report_path,
                &capture,
                &baseline,
                decisions.as_deref(),
                approval::Options {
                    names,
                    all_failing: all_failing.is_some(),
                    include_errors,
                    prune_missing,
                    dry_run,
                    out,
                    json,
                    absolute: record_absolute_paths,
                },
            )
        }
        #[cfg(feature = "workbench")]
        Command::Serve {
            mut roots,
            registered_roots,
            out_root,
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
            roots.extend(registered_roots);
            if roots.is_empty() {
                return Err(CliError::usage("serve requires ROOT or --root DIR"));
            }
            let loaded = load_config(config.as_deref())?;
            if !loaded.symlink_targets.is_empty() {
                return Err(CliError::usage(
                    "project config cannot authorize symlink targets; pass --symlink-target at startup",
                ));
            }
            let mut opts = saccade_core::serve::ServeOptions::new(roots.remove(0));
            opts.extra_roots = roots;
            opts.follow_symlinks_within_roots = follow_symlinks_within_roots;
            opts.symlink_targets.extend(symlink_targets);
            opts.fs_timeout_ms = fs_timeout_ms.unwrap_or(loaded.fs_timeout_ms);
            opts.port = port;
            if let Some(root) = out_root {
                let all_roots = std::iter::once(opts.root.clone())
                    .chain(opts.extra_roots.clone())
                    .collect::<Vec<_>>();
                let policy = saccade_core::root_policy::RootPolicy::new(
                    &all_roots,
                    Some(&root),
                    opts.follow_symlinks_within_roots,
                    &opts.symlink_targets,
                )?;
                opts.cache_dir = policy.write(Path::new("cache"))?;
                opts.decisions_dir = policy.write(Path::new("decisions"))?;
            }
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
        Command::View {
            dirs,
            unblind,
            key,
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
            if let Some(decisions) = unblind {
                return local_cmd::unblind(
                    &decisions,
                    key.as_deref()
                        .ok_or_else(|| CliError::usage("--unblind requires --key"))?,
                    &out,
                    record_absolute_paths,
                );
            }
            if dirs.len() == 1 {
                return local_cmd::view_artifact(&dirs[0], &out, json);
            }
            if blind && key_out.is_none() {
                return Err(CliError::usage(
                    "--blind requires --key-out outside the view bundle",
                ));
            }
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
            let key = key_out.unwrap_or_else(|| saccade_core::view::private_key_path(&out));
            if json {
                let mut value = local_cmd::base_result("view");
                value["artifact"] =
                    local_cmd::reference(&out.join(saccade_core::view::VIEW_MARKER_FILE))?;
                value["counts"] =
                    serde_json::json!({"sets":model.sets.len(),"directories":model.labels.len()});
                value["data"] = serde_json::json!({"blind":blind});
                emit(&format!("{}\n", serde_json::to_string(&value)?))?;
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
        #[cfg(feature = "mcp")]
        Command::Mcp {
            roots,
            out_root,
            follow_symlinks_within_roots,
            symlink_targets,
            #[cfg(feature = "ai")]
            providers,
        } => {
            mcp::serve_stdio(
                &roots,
                out_root.as_deref(),
                follow_symlinks_within_roots,
                &symlink_targets,
                #[cfg(feature = "ai")]
                providers,
            )?;
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

/// Opens `url` in the default browser; failures are ignored.
#[cfg(feature = "workbench")]
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

/// The feature required by an optional CLI operation or MCP tool.
fn required_feature(operation: &str) -> Option<&'static str> {
    match operation {
        "ablate" | "bisect" | "sequence" | "rank" | "saccade_ablate" | "saccade_bisect"
        | "saccade_sequence" | "saccade_rank" => Some("graphics"),
        "saccade_review" => Some("ai"),
        "calibrate"
        | "selftest"
        | "bench"
        | "collect-labels"
        | "saccade_judge_calibrate"
        | "saccade_judge_bench" => Some("evaluation"),
        "serve" | "ask" | "saccade_ask_human" | "saccade_inbox_get" => Some("workbench"),
        "safety" | "a11y" | "saccade_safety" | "saccade_a11y" => Some("prechecks"),
        "mcp" => Some("mcp"),
        _ => None,
    }
}

fn feature_enabled(feature: &str) -> bool {
    if feature == "mcp" {
        cfg!(feature = "mcp")
    } else {
        saccade_core::COMPILED_FEATURES.contains(&feature)
    }
}

fn unavailable_feature(operation: &str) -> Option<&'static str> {
    required_feature(operation).filter(|f| !feature_enabled(f))
}

fn requested_feature(args: &[std::ffi::OsString]) -> Option<&'static str> {
    let operation = args
        .iter()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with('-'))?
        .to_str()?;
    unavailable_feature(operation).or_else(|| {
        if operation == "experiment" {
            args.iter()
                .skip(2)
                .find_map(|a| a.to_str().and_then(unavailable_feature))
        } else if operation == "init" && !feature_enabled("ai") {
            args.windows(2)
                .any(|w| {
                    w[0] == "--template"
                        && matches!(w[1].to_str(), Some("ci" | "nightly" | "lookdev"))
                })
                .then_some("ai")
        } else {
            None
        }
    })
}

pub(crate) fn capabilities(json: bool) -> Result<u8, CliError> {
    use clap::CommandFactory;
    fn operations(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for child in command.get_subcommands() {
            let name = if prefix.is_empty() {
                child.get_name().to_owned()
            } else {
                format!("{prefix} {}", child.get_name())
            };
            out.push(name.clone());
            operations(child, &name, out);
        }
    }
    let mut names = Vec::new();
    operations(&Cli::command(), "", &mut names);
    let mut features = saccade_core::COMPILED_FEATURES.to_vec();
    if cfg!(feature = "mcp") {
        features.push("mcp");
    }
    features.sort_unstable();
    let value = serde_json::json!({
        "features": features,
        "operations": names,
        "contract_versions": ["saccade-report.v1", "saccade-result.v2", "saccade-evidence.v1", "saccade-noise.v1"],
    });
    if json {
        let mut result = local_cmd::base_result("capabilities");
        result["data"] = value.clone();
        emit(&format!("{}\n", serde_json::to_string_pretty(&result)?))?;
    } else {
        emit(&format!(
            "features: {}\noperations: {}\ncontracts: {}\n",
            features.join(", "),
            names.join(", "),
            value["contract_versions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ))?;
    }
    Ok(0)
}
