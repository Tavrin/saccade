//! `flipdiff` command-line interface: `compare`, `identity`, `approve`, `view`, `unblind`, `summary`.
//!
//! Exit codes: `0` no regression, `1` regression, `2` usage/config/IO error.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use flipdiff_core::config::RunConfig;
use flipdiff_core::render::{MarkdownOptions, is_valid_comment_key, render_markdown};
use flipdiff_core::report::{Labels, Metric, Mode, Report, Status};
use flipdiff_core::view::{
    ViewOptions, build_view, is_safe_name, read_blind_key, read_decisions, unblind,
};

mod agent;
mod mcp;

#[derive(Parser)]
#[command(
    name = "flipdiff",
    version,
    about = "Perceptual (FLIP) visual-regression diffing"
)]
struct Cli {
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
    fn apply(&self, hdr: &mut flipdiff_core::hdr::HdrConfig) -> Result<(), String> {
        if let Some(t) = &self.hdr_tonemapper {
            hdr.tonemapper = flipdiff_core::hdr::Tonemapper::parse(t).map_err(|e| e.to_string())?;
        }
        if let Some(e) = &self.hdr_exposures {
            hdr.parse_exposures(e).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Metadata-sidecar flags shared by `compare`, `identity` and `view`.
#[derive(clap::Args, Clone, Default)]
struct MetaArgs {
    /// Sidecar file name (default `flipdiff-meta.json`); the per-image sidecar is
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
    fn apply(&self, meta: &mut flipdiff_core::meta::MetaOptions) {
        if let Some(n) = &self.meta_name {
            meta.name.clone_from(n);
        }
        meta.ignore.extend(self.meta_ignore.iter().cloned());
    }
}

impl MetaRequireArgs {
    fn apply(&self, meta: &mut flipdiff_core::meta::MetaOptions) {
        meta.required |= self.require_matching_meta;
        meta.declared.extend(self.declare.iter().cloned());
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum MetricArg {
    Mean,
    P95,
    Max,
}

impl From<MetricArg> for Metric {
    fn from(m: MetricArg) -> Self {
        match m {
            MetricArg::Mean => Metric::Mean,
            MetricArg::P95 => Metric::P95,
            MetricArg::Max => Metric::Max,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Markdown,
    Text,
    Json,
}

#[derive(Subcommand)]
enum Command {
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
        /// Config file; defaults to ./flipdiff.toml when it exists.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Treat new images (no baseline) as a regression.
        #[arg(long)]
        fail_on_new: bool,
        /// Print the report JSON instead of the table.
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
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
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
        /// Pass threshold for images that are not bit-identical.
        #[arg(long)]
        threshold: Option<f64>,
        /// Deciding metric (default: max).
        #[arg(long, value_enum)]
        metric: Option<MetricArg>,
        /// Config file; defaults to ./flipdiff.toml when it exists.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print the report JSON instead of the table.
        #[arg(long)]
        json: bool,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Display names of the two sides, `parent,candidate`.
        #[arg(long, value_delimiter = ',', value_name = "A,B")]
        labels: Option<Vec<String>>,
        #[command(flatten)]
        meta: MetaArgs,
        #[command(flatten)]
        require: MetaRequireArgs,
    },
    /// Copy captures over baselines.
    Approve {
        /// Directory of fresh captures.
        capture_dir: PathBuf,
        /// Baseline directory to update.
        baseline_dir: PathBuf,
        /// Image names (relative paths) to approve.
        names: Vec<String>,
        /// Also approve every fail and new entry of this report JSON.
        #[arg(long, value_name = "REPORT_JSON")]
        all_failing: Option<PathBuf>,
        /// Also approve every "accept" entry of a decisions file exported by
        /// `flipdiff view`.
        #[arg(long, value_name = "DECISIONS_JSON")]
        decisions: Option<PathBuf>,
        /// With --all-failing: also approve `error` entries (for example a size
        /// change) whose capture exists and decodes.
        #[arg(long, requires = "all_failing")]
        include_errors: bool,
        /// With --all-failing: delete the baselines of every `missing` entry
        /// of the report (capture absent). Only files inside the baseline
        /// directory are removed; each removal is printed.
        #[arg(long, requires = "all_failing")]
        prune_missing: bool,
        /// Print `{"schema":"flipdiff-approve.v1","copied":[...],"pruned":[...]}`
        /// instead of one line per file.
        #[arg(long)]
        json: bool,
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
        /// Seed for the blind shuffle (default: from the clock; recorded in the data).
        #[arg(long)]
        seed: Option<u64>,
        /// Output directory.
        #[arg(long, default_value = "view")]
        out: PathBuf,
        /// FLIP pixels per degree.
        #[arg(long)]
        ppd: Option<f32>,
        /// Config file whose `[[region]]` tables become preset ROIs
        /// (default: `./flipdiff.toml` when present).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Print a JSON summary (`flipdiff-view-summary.v1`) instead of text.
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        hdr: HdrArgs,
        #[command(flatten)]
        meta: MetaArgs,
    },
    /// Serve a local web app for browsing a capture archive and comparing runs
    /// (127.0.0.1 only; the archive is never written to).
    Serve {
        /// Archive root to browse (read-only).
        root: PathBuf,
        /// Port on 127.0.0.1 (0 picks a free one).
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Cache directory for sessions, thumbnails and uploads
        /// (default: `$XDG_CACHE_HOME/flipdiff`).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Directory the viewer's decisions are written to
        /// (default: `$XDG_DATA_HOME/flipdiff/decisions`).
        #[arg(long)]
        decisions_dir: Option<PathBuf>,
        /// Config file for sidecar settings and preset regions
        /// (default: `./flipdiff.toml` when present).
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
        /// Path to `flipdiff-report.v1.json`.
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
        /// key goes to `blind-key.json`.
        #[arg(long)]
        blind: bool,
        /// Seed for the blind shuffle (default: from the clock; recorded in the key).
        #[arg(long, requires = "blind")]
        seed: Option<u64>,
        /// Explain only these entries (default: every failing entry).
        #[arg(long, value_delimiter = ',', value_name = "NAME,...")]
        entries: Vec<String>,
        /// Print the pack's `explain.json` instead of `explain.md`.
        #[arg(long)]
        json: bool,
    },
    /// Serve the Model Context Protocol over stdio, so an AI agent can run
    /// comparisons and read their hotspots as a tool.
    Mcp,
    /// Print a summary of a report JSON.
    Summary {
        /// Path to `flipdiff-report.v1.json`.
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("flipdiff: error: {}", escape_multiline(&msg));
            ExitCode::from(2)
        }
    }
}

fn dispatch(command: Command) -> Result<u8, String> {
    match command {
        Command::Compare {
            baseline_dir,
            capture_dir,
            out,
            threshold,
            metric,
            config,
            fail_on_new,
            json,
            ppd,
            labels,
            hdr,
            meta,
            require,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            hdr.apply(&mut cfg.hdr)?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
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
            let report = flipdiff_core::run::run(&baseline_dir, &capture_dir, &out, &cfg)
                .map_err(|e| e.to_string())?;
            if json {
                let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
                emit(&format!("{text}\n"))?;
            } else {
                emit(&text_table(&report))?;
            }
            Ok(u8::from(report.is_regression()))
        }
        Command::Identity {
            parent_dir,
            candidate_dir,
            out,
            threshold,
            metric,
            config,
            json,
            ppd,
            labels,
            meta,
            require,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            meta.apply(&mut cfg.meta);
            require.apply(&mut cfg.meta);
            if config.is_none() && !cfg.overrides.is_empty() {
                // `./flipdiff.toml` is auto-loaded; it must not silently relax identity.
                eprintln!(
                    "flipdiff: note: ignoring [[override]] entries ({}) from the auto-loaded flipdiff.toml; \
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
                    "flipdiff: note: applying [[override]] entries ({}) to identity because --config was given",
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
            let report = flipdiff_core::run::run(&parent_dir, &candidate_dir, &out, &cfg)
                .map_err(|e| e.to_string())?;
            if json {
                let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
                emit(&format!("{text}\n"))?;
            } else {
                emit(&text_table(&report))?;
            }
            Ok(u8::from(report.is_regression()))
        }
        Command::Approve {
            capture_dir,
            baseline_dir,
            names,
            all_failing,
            decisions,
            include_errors,
            prune_missing,
            json,
        } => approve(
            &capture_dir,
            &baseline_dir,
            names,
            all_failing.as_deref(),
            decisions.as_deref(),
            ApproveFlags {
                include_errors,
                prune_missing,
                json,
            },
        ),
        Command::Serve {
            root,
            port,
            cache_dir,
            decisions_dir,
            config,
            ppd,
            open,
            hdr,
            meta,
        } => {
            let loaded = load_config(config.as_deref())?;
            let mut opts = flipdiff_core::serve::ServeOptions::new(root);
            opts.port = port;
            if let Some(d) = cache_dir {
                opts.cache_dir = d;
            }
            if let Some(d) = decisions_dir {
                opts.decisions_dir = d;
            }
            opts.view.regions = loaded.regions;
            opts.view.meta = loaded.meta;
            hdr.apply(&mut opts.view.hdr)?;
            meta.apply(&mut opts.view.meta);
            if let Some(p) = ppd {
                opts.view.pixels_per_degree = p;
            }
            let (cache, decisions) = (opts.cache_dir.clone(), opts.decisions_dir.clone());
            let handle = flipdiff_core::serve::start(opts).map_err(|e| e.to_string())?;
            let url = format!("http://127.0.0.1:{}/", handle.port());
            emit(&format!(
                "flipdiff serve: {url}\n  cache:     {}\n  decisions: {}\n  (Ctrl-C to stop)\n",
                escape_control(&cache.display().to_string()),
                escape_control(&decisions.display().to_string())
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
            let decisions = read_decisions(&decisions_json).map_err(|e| e.to_string())?;
            let key = read_blind_key(&blind_key_json).map_err(|e| e.to_string())?;
            let text = serde_json::to_string_pretty(
                &unblind(&decisions, &key).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            match out {
                Some(path) => std::fs::write(&path, format!("{text}\n"))
                    .map_err(|e| format!("writing {}: {e}", path.display()))?,
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
            out,
            ppd,
            config,
            json,
            hdr,
            meta,
        } => {
            let loaded = load_config(config.as_deref())?;
            let mut opts = ViewOptions {
                regions: loaded.regions,
                meta: loaded.meta,
                labels,
                reference,
                blind,
                seed,
                ..ViewOptions::default()
            };
            hdr.apply(&mut opts.hdr)?;
            meta.apply(&mut opts.meta);
            if let Some(p) = ppd {
                opts.pixels_per_degree = p;
            }
            let model = build_view(&dirs, &out, &opts).map_err(|e| e.to_string())?;
            if json {
                let key = out.join(flipdiff_core::view::BLIND_KEY_FILE);
                let value = serde_json::json!({
                    "schema": "flipdiff-view-summary.v1",
                    "index_html": out.join("index.html").display().to_string(),
                    "out_dir": out.display().to_string(),
                    "sets": model.sets.len(),
                    "set_names": model.sets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                    "directories": model.labels.len(),
                    // A blind view's labels stay out of the output.
                    "labels": (!blind).then(|| model.labels.clone()),
                    "blind": blind,
                    "seed": model.seed,
                    "blind_key": blind.then(|| key.display().to_string()),
                });
                emit(&format!(
                    "{}\n",
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
                ))?;
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
                    escape_control(
                        &out.join(flipdiff_core::view::BLIND_KEY_FILE)
                            .display()
                            .to_string()
                    )
                ))?;
            }
            Ok(0)
        }
        Command::Mcp => {
            mcp::serve_stdio()?;
            Ok(0)
        }
        Command::Explain {
            report_json,
            out,
            top,
            pad,
            stretch,
            blind,
            seed,
            entries,
            json,
        } => {
            let out = out.unwrap_or_else(|| {
                report_json
                    .parent()
                    .map_or_else(|| PathBuf::from("explain"), |p| p.join("explain"))
            });
            let opts = flipdiff_core::explain::ExplainOptions {
                top,
                pad,
                stretch,
                blind,
                seed,
                entries,
            };
            let pack = flipdiff_core::explain::explain(&report_json, &out, &opts)
                .map_err(|e| e.to_string())?;
            if json {
                let text = serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())?;
                emit(&format!("{text}\n"))?;
            } else {
                let md = std::fs::read_to_string(out.join(flipdiff_core::explain::EXPLAIN_MD_FILE))
                    .map_err(|e| format!("reading explain.md: {e}"))?;
                emit(&md)?;
                emit(&format!(
                    "\npack: {}\n",
                    escape_control(&out.display().to_string())
                ))?;
                if blind {
                    emit(&format!(
                        "blind key (keep it away from the judge): {}\n",
                        escape_control(
                            &out.join(flipdiff_core::explain::EXPLAIN_BLIND_KEY_FILE)
                                .display()
                                .to_string()
                        )
                    ))?;
                }
            }
            Ok(0)
        }
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
                return Err("--comment-key must be 1-64 characters of A-Z a-z 0-9 . _ -".into());
            }
            let report = read_report(&report_json)?;
            match format {
                Format::Markdown => emit(&render_markdown(
                    &report,
                    &MarkdownOptions {
                        artifact_url,
                        comment_key,
                        max_bytes: None,
                    },
                ))?,
                Format::Text => emit(&text_table(&report))?,
                Format::Json => {
                    let value =
                        agent::summary_value(&report, &report_json, agent::DEFAULT_TOP_FAILING);
                    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
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

fn load_config(explicit: Option<&Path>) -> Result<RunConfig, String> {
    let path = match explicit {
        Some(p) => Some(p),
        None => Some(Path::new("flipdiff.toml")).filter(|p| p.is_file()),
    };
    match path {
        Some(p) => RunConfig::from_toml_file(p).map_err(|e| e.to_string()),
        None => Ok(RunConfig::default()),
    }
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

fn read_report(path: &Path) -> Result<Report, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading report {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parsing report {}: {e}", path.display()))
}

/// Writes `text` to stdout. A closed pipe (for example `| head`) is not an error.
fn emit(text: &str) -> Result<(), String> {
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(format!("writing to stdout: {e}")),
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
fn check_no_symlinks(baseline_dir: &Path, rel: &Path) -> Result<(), String> {
    let mut cur = baseline_dir.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "refusing to write through symlink {}",
                    cur.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(format!("inspecting {}: {e}", cur.display())),
        }
    }
    Ok(())
}

/// The switches of `approve`.
struct ApproveFlags {
    include_errors: bool,
    prune_missing: bool,
    json: bool,
}

fn approve(
    capture_dir: &Path,
    baseline_dir: &Path,
    mut names: Vec<String>,
    all_failing: Option<&Path>,
    decisions: Option<&Path>,
    flags: ApproveFlags,
) -> Result<u8, String> {
    let ApproveFlags {
        include_errors,
        prune_missing,
        json,
    } = flags;
    let mut copied: Vec<serde_json::Value> = Vec::new();
    let mut pruned: Vec<String> = Vec::new();
    let mut prune: Vec<String> = Vec::new();
    if let Some(path) = decisions {
        let d = read_decisions(path).map_err(|e| e.to_string())?;
        names.extend(d.accepted());
    }
    if let Some(path) = all_failing {
        let report = read_report(path)?;
        if prune_missing {
            prune = report
                .entries
                .iter()
                .filter(|e| e.status == Status::Missing)
                .map(|e| e.name.clone())
                .collect();
            prune.sort();
            prune.dedup();
        }
        names.extend(
            report
                .entries
                .into_iter()
                .filter(|e| match e.status {
                    Status::Fail | Status::New => true,
                    Status::Error => {
                        include_errors
                            && e.paths.capture.is_some()
                            && is_safe_name(&e.name)
                            && flipdiff_core::run::is_decodable(&capture_dir.join(&e.name))
                    }
                    _ => false,
                })
                .map(|e| e.name),
        );
    } else if names.is_empty() && decisions.is_none() {
        return Err(
            "name at least one image, or pass --all-failing <REPORT_JSON> or --decisions <FILE>"
                .into(),
        );
    }
    names.sort();
    names.dedup();
    // Validate every name before copying anything.
    for name in &names {
        if !is_safe_name(name) {
            return Err(format!("unsafe image name {name:?}"));
        }
        let src = capture_dir.join(name);
        if !src.is_file() {
            return Err(format!("capture {} does not exist", src.display()));
        }
        check_no_symlinks(baseline_dir, Path::new(name))?;
    }
    for name in &prune {
        if !is_safe_name(name) {
            return Err(format!("unsafe image name {name:?}"));
        }
        check_no_symlinks(baseline_dir, Path::new(name))?;
    }
    for name in &names {
        let rel = Path::new(name);
        let src = capture_dir.join(rel);
        let dest = baseline_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        std::fs::copy(&src, &dest)
            .map_err(|e| format!("copying {} to {}: {e}", src.display(), dest.display()))?;
        if json {
            copied.push(serde_json::json!({
                "name": name,
                "from": src.display().to_string(),
                "to": dest.display().to_string(),
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
        std::fs::remove_file(&target).map_err(|e| format!("removing {}: {e}", target.display()))?;
        if json {
            pruned.push(target.display().to_string());
        } else {
            emit(&format!(
                "removed {}\n",
                escape_control(&target.display().to_string())
            ))?;
        }
    }
    if json {
        let value = serde_json::json!({
            "schema": "flipdiff-approve.v1",
            "copied": copied,
            "pruned": pruned,
        });
        let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
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
        Metric::Max => "max",
    }
}

/// Aligned plain-text table, non-pass rows first (then by name).
fn text_table(report: &Report) -> String {
    let mut rows: Vec<&flipdiff_core::Entry> = report.entries.iter().collect();
    rows.sort_by_key(|e| (e.status == Status::Pass, e.name.clone()));
    let mut cells: Vec<[String; 5]> = vec![[
        "STATUS".into(),
        "NAME".into(),
        "METRIC".into(),
        "VALUE".into(),
        "THRESHOLD".into(),
    ]];
    let mut notes: Vec<Option<String>> = vec![None];
    for e in rows {
        notes.push(if e.status == Status::Fail {
            flipdiff_core::hotspots::summary_line(&e.hotspots)
        } else {
            None
        });
        let (value, threshold) = match e.status {
            Status::Pass | Status::Fail => (
                e.value.map_or("-".into(), |v| format!("{v:.5}")),
                format!("{}", e.threshold),
            ),
            _ => ("-".into(), "-".into()),
        };
        let name = match &e.error {
            Some(msg) => format!("{} ({})", escape_control(&e.name), escape_control(msg)),
            None => escape_control(&e.name),
        };
        let name = if e.meta_diff.is_empty() {
            name
        } else {
            let keys: Vec<&str> = e.meta_diff.iter().map(|d| d.key.as_str()).collect();
            format!(
                "{name} [config differs: {}]",
                escape_control(&keys.join(", "))
            )
        };
        cells.push([
            status_label(e.status).into(),
            name,
            metric_label(e.metric_used).into(),
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
        if let Some(note) = note {
            out.push_str(&format!("  ↳ {note}\n"));
        }
    }
    if let Some(headline) = flipdiff_core::render::identity_headline(report) {
        out.insert_str(0, &format!("{}\n\n", escape_control(&headline)));
    }
    let t = &report.totals;
    out.push_str(&format!(
        "\n{} fail, {} error, {} missing, {} new, {} pass ({} total)\n",
        t.fail, t.error, t.missing, t.new, t.pass, t.total
    ));
    out
}
