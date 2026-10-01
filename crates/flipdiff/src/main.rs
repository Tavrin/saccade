//! `flipdiff` command-line interface: `compare`, `approve`, `view`, `summary`.
//!
//! Exit codes: `0` no regression, `1` regression, `2` usage/config/IO error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use flipdiff_core::config::RunConfig;
use flipdiff_core::render::{MarkdownOptions, render_markdown};
use flipdiff_core::report::{Metric, Report, Status};
use flipdiff_core::view::{ViewOptions, build_view, is_safe_name, read_decisions};

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
    },
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
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("flipdiff: error: {msg}");
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
        } => {
            let mut cfg = load_config(config.as_deref())?;
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
            let report = flipdiff_core::run::run(&baseline_dir, &capture_dir, &out, &cfg)
                .map_err(|e| e.to_string())?;
            if json {
                let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
                println!("{text}");
            } else {
                print!("{}", text_table(&report));
            }
            Ok(u8::from(report.is_regression()))
        }
        Command::Approve {
            capture_dir,
            baseline_dir,
            names,
            all_failing,
            decisions,
        } => approve(
            &capture_dir,
            &baseline_dir,
            names,
            all_failing.as_deref(),
            decisions.as_deref(),
        ),
        Command::View {
            dirs,
            labels,
            reference,
            blind,
            seed,
            out,
            ppd,
        } => {
            let mut opts = ViewOptions {
                labels,
                reference,
                blind,
                seed,
                ..ViewOptions::default()
            };
            if let Some(p) = ppd {
                opts.pixels_per_degree = p;
            }
            let model = build_view(&dirs, &out, &opts).map_err(|e| e.to_string())?;
            println!(
                "wrote {} ({} image sets, {} directories)",
                out.join("index.html").display(),
                model.sets.len(),
                model.labels.len()
            );
            Ok(0)
        }
        Command::Summary {
            report_json,
            format,
            artifact_url,
        } => {
            let report = read_report(&report_json)?;
            match format {
                Format::Markdown => print!(
                    "{}",
                    render_markdown(
                        &report,
                        &MarkdownOptions {
                            artifact_url,
                            max_bytes: None,
                        },
                    )
                ),
                Format::Text => print!("{}", text_table(&report)),
            }
            Ok(0)
        }
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

fn read_report(path: &Path) -> Result<Report, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading report {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parsing report {}: {e}", path.display()))
}

fn approve(
    capture_dir: &Path,
    baseline_dir: &Path,
    mut names: Vec<String>,
    all_failing: Option<&Path>,
    decisions: Option<&Path>,
) -> Result<u8, String> {
    if let Some(path) = decisions {
        let d = read_decisions(path).map_err(|e| e.to_string())?;
        names.extend(d.accepted());
    }
    if let Some(path) = all_failing {
        let report = read_report(path)?;
        names.extend(
            report
                .entries
                .into_iter()
                .filter(|e| matches!(e.status, Status::Fail | Status::New))
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
    for name in &names {
        if !is_safe_name(name) {
            return Err(format!("unsafe image name {name:?}"));
        }
        let rel = Path::new(name);
        let src = capture_dir.join(rel);
        let dest = baseline_dir.join(rel);
        if !src.is_file() {
            return Err(format!("capture {} does not exist", src.display()));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        std::fs::copy(&src, &dest)
            .map_err(|e| format!("copying {} to {}: {e}", src.display(), dest.display()))?;
        println!("{} -> {}", src.display(), dest.display());
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
    for e in rows {
        let (value, threshold) = match e.status {
            Status::Pass | Status::Fail => (
                e.value.map_or("-".into(), |v| format!("{v:.5}")),
                format!("{}", e.threshold),
            ),
            _ => ("-".into(), "-".into()),
        };
        let name = match &e.error {
            Some(msg) => format!("{} ({msg})", e.name),
            None => e.name.clone(),
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
    for row in &cells {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    let t = &report.totals;
    out.push_str(&format!(
        "\n{} fail, {} error, {} missing, {} new, {} pass ({} total)\n",
        t.fail, t.error, t.missing, t.new, t.pass, t.total
    ));
    out
}
