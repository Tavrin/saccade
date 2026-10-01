//! Core and CLI release ergonomics.

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use saccade_core::config::RunConfig;
use saccade_core::report::REPORT_FILE_NAME;

use crate::agent::CliError;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Template {
    Renderer,
    Ui,
    Identity,
    Ml,
}

#[derive(Args)]
pub(crate) struct InitArgs {
    #[arg(long, value_enum, default_value = "renderer")]
    template: Template,
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    #[arg(long)]
    force: bool,
}

pub(crate) fn template(t: Template) -> &'static str {
    match t {
        Template::Renderer => {
            "# Renderer captures: refuse comparisons from different settings.\nrequire_matching_meta = true\nmetric = \"p95\"\nthreshold = 0.01\n# Catch severe small defects even when p95 passes.\nhotspot_fail = 0.5\n"
        }
        Template::Ui => {
            "# UI screenshots: small text/control changes matter.\nmetric = \"p95\"\nthreshold = 0.002\n\n[[region]]\nname = \"text\"\nrect = [0.05, 0.1, 0.9, 0.5]\nmetric = \"max\"\nthreshold = 0.02\n\n[[region]]\nname = \"controls\"\nrect = [0.05, 0.6, 0.9, 0.35]\nthreshold = 0.002\n\n# Example timestamp mask: adjust to your clock's rectangle.\n# [[mask]]\n# rect = [0.85, 0.0, 0.15, 0.05]\n"
        }
        Template::Identity => {
            "# Pixel-preserving refactors and optimizations.\nmetric = \"max\"\nthreshold = 0\n"
        }
        Template::Ml => {
            "# Compare the same seeds/prompts across model checkpoints.\nmetric = \"p95\"\nthreshold = 0.01\n# Rank: saccade rank reference checkpoint-a checkpoint-b --out ranking\n# Judge: saccade judge ranking/saccade-rank.v1.json --panel examples/panel.toml --dry-run\n"
        }
    }
}

pub(crate) fn init(args: InitArgs) -> Result<u8, CliError> {
    std::fs::create_dir_all(&args.dir)
        .map_err(|e| CliError::io(format!("creating {}: {e}", args.dir.display())))?;
    let path = args.dir.join("saccade.toml");
    // create_new makes the refusal race-free; even --force never follows a symlink.
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::new(
            "unsafe_path",
            format!("refusing config symlink {}", path.display()),
        ));
    }
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(&args.dir)
        .map_err(|e| CliError::io(format!("creating config in {}: {e}", args.dir.display())))?;
    file.write_all(template(args.template).as_bytes())
        .map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))?;
    let result = if args.force {
        file.persist(&path)
    } else {
        file.persist_noclobber(&path)
    };
    result.map_err(|e| {
        CliError::io(format!(
            "writing {}: {e}; use --force to replace an existing config",
            path.display()
        ))
    })?;
    crate::emit(&format!(
        "wrote {}\nBootstrap baselines from {}:\n  mkdir -p baseline\n  saccade compare baseline capture --out report\n  saccade approve --report report/saccade-report.v1.json --all-failing\n  Commit the baseline directory with your project.\nFirst run exits 1 because no baseline pairs exist yet.\n",
        path.display(),
        args.dir.display()
    ))?;
    Ok(0)
}

#[derive(Args)]
pub(crate) struct ConfigArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, value_name = "PATH_OR_NAME")]
    explain: Option<String>,
    #[arg(long)]
    json: bool,
}

pub(crate) fn config(args: ConfigArgs) -> Result<u8, CliError> {
    let file = args.config.as_deref().or_else(|| {
        Path::new("saccade.toml")
            .is_file()
            .then_some(Path::new("saccade.toml"))
    });
    let cfg = crate::load_config(file)?;
    let reason = if args.config.is_some() {
        "explicit --config"
    } else if file.is_some() {
        "auto-loaded ./saccade.toml"
    } else {
        "no ./saccade.toml; built-in defaults"
    };
    let name = args.explain.as_deref().map(|n| {
        let p = Path::new(n);
        let base = file.and_then(Path::parent).unwrap_or(Path::new("."));
        if p.is_absolute() {
            saccade_core::paths::record(p, base, false)
        } else {
            n.replace('\\', "/")
        }
    });
    let value = cfg.explain_settings(file, reason, name.as_deref())?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    } else {
        crate::emit(&format!(
            "config: {} ({reason})\n",
            file.map_or_else(
                || "built-in defaults".into(),
                |p| saccade_core::paths::cwd(p, false)
            )
        ))?;
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    }
    Ok(0)
}

#[derive(Args)]
pub(crate) struct EntriesArgs {
    report_json: PathBuf,
    #[arg(long, value_delimiter = ',')]
    status: Vec<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long, default_value_t = 0)]
    offset: usize,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long)]
    json: bool,
}

pub(crate) fn entries(args: EntriesArgs) -> Result<u8, CliError> {
    let report = crate::read_report(&args.report_json)?;
    let page = saccade_core::ergonomics::entries(
        &report,
        &args.status,
        args.name.as_deref(),
        args.offset,
        args.limit,
    )?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&page)?))?;
    } else {
        for e in &page.entries {
            crate::emit(&format!(
                "{:?}\t{}\t{:?} {:?} / {}\n",
                e.status,
                crate::escape_control(&e.name),
                e.metric_used,
                e.value,
                e.threshold
            ))?;
        }
        crate::emit(&format!(
            "{} matching; offset {}; returned {}; next {}\n",
            page.total,
            page.offset,
            page.entries.len(),
            page.next_cursor.as_deref().unwrap_or("end")
        ))?;
    }
    Ok(0)
}

#[derive(Args)]
pub(crate) struct NoiseArgs {
    #[arg(required = true, num_args = 2..)]
    dirs: Vec<PathBuf>,
    #[arg(long, default_value_t = 1.5)]
    margin: f64,
    #[arg(long, value_enum, default_value = "p95")]
    metric: crate::MetricArg,
    #[arg(long, default_value = "saccade.noise.toml")]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}

pub(crate) fn noise(args: NoiseArgs, record_absolute_paths: bool) -> Result<u8, CliError> {
    let mut report =
        saccade_core::ergonomics::noise(&args.dirs, args.margin, args.metric.into(), &args.out)?;
    if record_absolute_paths {
        report.runs = args
            .dirs
            .iter()
            .map(|p| saccade_core::paths::cwd(p, true))
            .collect();
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    } else {
        for e in &report.entries {
            crate::emit(&format!(
                "{}: mean {:.5}, p95 {:.5}, max {:.5}; suggested {:.5} ({:?})\n",
                crate::escape_control(&e.name),
                e.mean,
                e.p95,
                e.max,
                e.suggested_threshold,
                report.metric
            ))?;
        }
        crate::emit(&format!("wrote {}\n", args.out.display()))?;
    }
    for w in &report.warnings {
        eprintln!("saccade noise: {}", crate::escape_control(w));
    }
    Ok(0)
}

#[derive(Args)]
pub(crate) struct DemoArgs {
    #[arg(long)]
    out: Option<PathBuf>,
}

const EXAMPLES: &[(&str, &[u8])] = &[
    (
        "baseline/sphere_identical.png",
        include_bytes!("../assets/demo/baseline/sphere_identical.png"),
    ),
    (
        "baseline/sphere_missing.png",
        include_bytes!("../assets/demo/baseline/sphere_missing.png"),
    ),
    (
        "baseline/sphere_shadow.png",
        include_bytes!("../assets/demo/baseline/sphere_shadow.png"),
    ),
    (
        "baseline/sphere_subtle.png",
        include_bytes!("../assets/demo/baseline/sphere_subtle.png"),
    ),
    (
        "capture/sphere_identical.png",
        include_bytes!("../assets/demo/capture/sphere_identical.png"),
    ),
    (
        "capture/sphere_new.png",
        include_bytes!("../assets/demo/capture/sphere_new.png"),
    ),
    (
        "capture/sphere_shadow.png",
        include_bytes!("../assets/demo/capture/sphere_shadow.png"),
    ),
    (
        "capture/sphere_subtle.png",
        include_bytes!("../assets/demo/capture/sphere_subtle.png"),
    ),
];

pub(crate) fn demo(args: DemoArgs, record_absolute_paths: bool) -> Result<u8, CliError> {
    let dir = match args.out {
        Some(p) => p,
        None => tempfile::Builder::new()
            .prefix("saccade-demo-")
            .tempdir()
            .map_err(|e| CliError::io(format!("creating demo directory: {e}")))?
            .keep(),
    };
    saccade_core::run::guard_output_dir(&dir, &[], &[".saccade-demo"])?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| CliError::io(format!("creating {}: {e}", dir.display())))?;
    std::fs::write(dir.join(".saccade-demo"), b"saccade demo\n")
        .map_err(|e| CliError::io(format!("writing demo marker: {e}")))?;
    for (name, bytes) in EXAMPLES {
        let path = dir.join(name);
        crate::check_no_symlinks(&dir, Path::new(name))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::io(format!("creating {}: {e}", parent.display())))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))?;
    }
    let out = dir.join("report");
    let report = saccade_core::run::run(
        &dir.join("baseline"),
        &dir.join("capture"),
        &out,
        &RunConfig {
            record_absolute_paths,
            ..Default::default()
        },
    )?;
    crate::emit(&crate::text_table(&report))?;
    crate::emit(&format!(
        "report: {}\nOpen {}: inspect sphere_shadow.png's moved light/shadow and numbered hotspots.\nExpected exit 1: the shadow fails and sphere_missing.png has no capture.\n",
        saccade_core::paths::cwd(&out.join(REPORT_FILE_NAME), record_absolute_paths),
        saccade_core::paths::cwd(&out.join("index.html"), record_absolute_paths)
    ))?;
    Ok(u8::from(report.is_regression()))
}

pub(crate) fn approve_dirs(
    capture: Option<PathBuf>,
    baseline: Option<PathBuf>,
    report: Option<&Path>,
    decisions: Option<&Path>,
) -> Result<(PathBuf, PathBuf), CliError> {
    if let (Some(c), Some(b)) = (&capture, &baseline) {
        if c.is_file() || b.is_file() {
            return Err(CliError::usage(
                "approve needs directories, not file inputs",
            ));
        }
        return Ok((c.clone(), b.clone()));
    }
    let derived = if let Some(p) = report {
        let r = crate::read_report(p)?;
        match (r.capture_dir, r.baseline_dir) {
            (Some(c), Some(b)) => Some((
                saccade_core::paths::resolve(&c, p),
                saccade_core::paths::resolve(&b, p),
            )),
            _ => None,
        }
    } else if let Some(p) = decisions {
        let d = saccade_core::view::read_decisions(p)?;
        if d.blind {
            return Err(CliError::new(
                "approve_mismatch",
                "blind --decisions: run saccade unblind first",
            ));
        }
        match d.dirs.as_slice() {
            [b, c] => Some((
                saccade_core::paths::resolve(c, p),
                saccade_core::paths::resolve(b, p),
            )),
            _ if capture.is_some() && baseline.is_some() => None,
            _ => {
                return Err(CliError::usage(
                    "--decisions needs exactly two recorded dirs to derive capture and baseline; supply positional directories for a multi-run view",
                ));
            }
        }
    } else {
        None
    };
    let (c, b) = match (capture, baseline, derived) {
        (Some(c), Some(b), _) => (c, b),
        (None, None, Some(pair)) => pair,
        _ => {
            return Err(CliError::usage(
                "approve needs --report REPORT_JSON, --decisions FILE, or both positional capture and baseline directories",
            ));
        }
    };
    if c.is_file() || b.is_file() {
        return Err(CliError::usage(
            "approve cannot update a file-pair report; compare directories before adopting baselines",
        ));
    }
    Ok((c, b))
}

pub(crate) fn junit_reports(out: &Path, path: &Path, rank: bool) -> Result<(), CliError> {
    let report = if rank {
        let document: saccade_core::rank::RankReport = serde_json::from_str(
            &std::fs::read_to_string(out.join("saccade-rank.v1.json"))
                .map_err(|e| CliError::io(e.to_string()))?,
        )?;
        let mut merged = None;
        for candidate in document.overall {
            let mut report =
                crate::read_report(&out.join(&candidate.label).join(REPORT_FILE_NAME))?;
            for e in &mut report.entries {
                e.name = format!("{}/{}", candidate.label, e.name);
            }
            match &mut merged {
                None => merged = Some(report),
                Some(r) => {
                    let r: &mut saccade_core::Report = r;
                    r.entries.extend(report.entries);
                }
            }
        }
        merged.ok_or_else(|| CliError::io("rank produced no reports"))?
    } else {
        crate::read_report(&out.join(REPORT_FILE_NAME))?
    };
    saccade_core::ergonomics::junit(&report, path)?;
    Ok(())
}

/// Refuse JUnit destinations that would overwrite inputs or generated report data.
pub(crate) fn guard_junit(
    path: Option<&Path>,
    inputs: &[&Path],
    out: &Path,
) -> Result<(), CliError> {
    let Some(path) = path else { return Ok(()) };
    let norm = saccade_core::run::normalise_path(path);
    if inputs
        .iter()
        .any(|p| norm.starts_with(saccade_core::run::normalise_path(p)))
    {
        return Err(CliError::usage(format!(
            "--junit {} is inside an input: choose a sibling results.xml",
            path.display()
        )));
    }
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::new(
            "unsafe_path",
            format!(
                "--junit {} is a symlink: choose a regular XML file",
                path.display()
            ),
        ));
    }
    let base = saccade_core::run::normalise_path(out);
    if norm.starts_with(base.join("images"))
        || [
            REPORT_FILE_NAME,
            "saccade-rank.v1.json",
            "saccade-sequence.v1.json",
            "index.html",
            "ranking.md",
            ".saccade-run",
        ]
        .iter()
        .any(|name| norm == base.join(name))
    {
        return Err(CliError::usage(format!(
            "--junit {} would replace report data: choose results.xml",
            path.display()
        )));
    }
    Ok(())
}
