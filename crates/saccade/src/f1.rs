//! Core and CLI release ergonomics.

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use saccade_core::config::RunConfig;
use saccade_core::report::REPORT_FILE_NAME;

use crate::agent::CliError;

#[path = "demo_assets.rs"]
mod demo_assets;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Template {
    Renderer,
    Ui,
    Identity,
    Ml,
    #[cfg(feature = "ai")]
    Ci,
    #[cfg(feature = "ai")]
    Nightly,
    #[cfg(feature = "ai")]
    Lookdev,
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
        #[cfg(feature = "ai")]
        Template::Ci => saccade_core::review::Profile::template("ci").unwrap_or(""),
        #[cfg(feature = "ai")]
        Template::Nightly => saccade_core::review::Profile::template("nightly").unwrap_or(""),
        #[cfg(feature = "ai")]
        Template::Lookdev => saccade_core::review::Profile::template("lookdev").unwrap_or(""),
        Template::Ml => {
            "# Compare the same seeds/prompts across model checkpoints.\nmetric = \"p95\"\nthreshold = 0.01\n# Rank: saccade experiment rank reference checkpoint-a checkpoint-b --out ranking\n# Review: saccade review ranking/saccade-rank.v1.json\n"
        }
    }
}

pub(crate) fn init(args: InitArgs) -> Result<u8, CliError> {
    std::fs::create_dir_all(&args.dir)
        .map_err(|e| CliError::io(format!("creating {}: {e}", args.dir.display())))?;
    #[cfg(feature = "ai")]
    let review = match args.template {
        #[cfg(feature = "ai")]
        Template::Ci => Some("ci"),
        #[cfg(feature = "ai")]
        Template::Nightly => Some("nightly"),
        #[cfg(feature = "ai")]
        Template::Lookdev => Some("lookdev"),
        Template::Ui => Some("ui"),
        _ => None,
    };
    #[cfg(not(feature = "ai"))]
    let review: Option<&str> = None;
    let path = args.dir.join(
        if review.is_some() && !matches!(args.template, Template::Ui) {
            "saccade-review.toml"
        } else {
            "saccade.toml"
        },
    );
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
    #[cfg(feature = "ai")]
    if let Some(name) = review {
        if matches!(args.template, Template::Ui) {
            let profile = args.dir.join("saccade-review.toml");
            if std::fs::symlink_metadata(&profile).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(CliError::new(
                    "unsafe_path",
                    "refusing review profile symlink",
                ));
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(!args.force)
                .create(args.force)
                .truncate(args.force)
                .open(&profile)
                .map_err(|e| CliError::io(e.to_string()))?;
            file.write_all(
                saccade_core::review::Profile::template(name)
                    .unwrap_or("")
                    .as_bytes(),
            )
            .map_err(|e| CliError::io(e.to_string()))?;
        }
        crate::emit(&format!(
            "wrote review profile; use saccade review REPORT_JSON --profile {}\n",
            args.dir.join("saccade-review.toml").display()
        ))?;
        if !matches!(args.template, Template::Ui) {
            return Ok(0);
        }
    }
    crate::emit(&format!(
        "wrote {}\nBootstrap baselines from {}:\n  mkdir -p baseline\n  saccade compare baseline capture --out report\n  saccade approve --report report/saccade-report.v1.json --all-failing --dry-run --out plan\n  Review plan/manifest.json and plan/decision.json, then under explicit human authorization:\n  saccade approve --report report/saccade-report.v1.json --decisions plan/decision.json --out approval\n  Commit the baseline directory with your project.\nFirst run exits 1 because no baseline pairs exist yet.\n",
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
    #[arg(long = "entry")]
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
        let mut result = crate::local_cmd::base_result("config");
        result["data"] = value;
        if let Some(data) = result["data"].as_object_mut() {
            // Settings already contain every default together with its source.
            data.remove("defaults");
        }
        crate::local_cmd::print(&crate::local_cmd::bounded(result, 4096)?, true)?;
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

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum NoiseKind {
    Image,
    Performance,
}
#[derive(Args)]
pub(crate) struct NoiseArgs {
    /// Image calibration (default) or qualified performance noise in ms.
    #[arg(long, value_enum, default_value = "image")]
    kind: NoiseKind,
    #[command(flatten)]
    perf: crate::perf_cmd::PerfArgs,
    #[arg(long)]
    config: Option<PathBuf>,
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
    if matches!(args.kind, NoiseKind::Image)
        && let Some(path) = &args.config
    {
        crate::perf_cmd::check_image_noise_config(path)?;
    }
    let mut cfg = crate::load_config(args.config.as_deref())?;
    args.perf.apply(&mut cfg.perf)?;
    if matches!(args.kind, NoiseKind::Performance) {
        let value =
            crate::perf_cmd::noise(&args.dirs, &args.out, &cfg.perf, record_absolute_paths)?;
        if args.json {
            crate::local_cmd::print(&value, true)?;
        } else {
            crate::emit(&format!(
                "performance noise (ms): qualification {}\nwrote {}\n",
                value["data"]["comparability"].as_str().unwrap_or("unknown"),
                args.out.display()
            ))?;
        }
        return Ok(0);
    }
    let mut report = saccade_core::ergonomics::noise_with_perf_options(
        &args.dirs,
        args.margin,
        args.metric.into(),
        &args.out,
        &cfg.perf,
    )?;
    if record_absolute_paths {
        report.runs = args
            .dirs
            .iter()
            .map(|p| saccade_core::paths::cwd(p, true))
            .collect();
    }
    let artifact = args.out.with_extension("json");
    let full = serde_json::to_value(&report)?;
    crate::local_cmd::write_value(&artifact, &full)?;
    if args.json {
        let mut value = crate::local_cmd::base_result("noise.image");
        value["artifact"] = crate::local_cmd::reference(&artifact)?;
        value["counts"] =
            serde_json::json!({"entries":report.entries.len(),"runs":report.runs.len()});
        value["data"] = serde_json::json!({"kind":"image_noise","unit":"FLIP"});
        crate::local_cmd::print(&value, true)?;
    } else {
        crate::emit("image noise (FLIP)\n")?;
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
        if let Some(f) = &report.perf_noise {
            crate::emit(&format!(
                "perf noise (max-min): frame {:.6} ms; {} terms; quantum {} ms; {} ticks; min delta {:.6} ms / {:.3}% of baseline frame\n",
                f.frame,
                f.terms.len(),
                f.resolution_ms
                    .map_or_else(|| "unknown".into(), |q| format!("{q:.9}")),
                f.resolution_ticks,
                f.min_delta_ms,
                f.min_delta_pct
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
    /// Directory for the demo images and reports (default: a new temporary directory).
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
    for (name, bytes) in EXAMPLES.iter().chain(demo_assets::FILES) {
        let path = dir.join(name);
        crate::check_no_symlinks(&dir, Path::new(name))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError::io(format!("creating {}: {e}", parent.display())))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))?;
    }
    demo_assets::reports(&dir, record_absolute_paths)?;
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
    let shown =
        |p: &Path| crate::escape_control(&saccade_core::paths::cwd(p, record_absolute_paths));
    crate::emit(&format!(
        "\nThis exit 1 is expected: the demo contains a regression and a missing capture.\n\n\
Open these three reports in a browser:\n  \
regression  {}\n              the moved shadow, the changed UI label and their numbered hotspots\n  \
identity    {}\n              equal pixels saved with different PNG encodings: identity passes\n  \
review      {}\n              a declared change with illustrative offline responses and resolution\n\n\
Measurement: {}\n\
Next: compare your own images with `saccade compare BASELINE_DIR CAPTURE_DIR --out report`.\n",
        shown(&out.join("index.html")),
        shown(&dir.join("identity/report/index.html")),
        shown(&dir.join("review/report/index.html")),
        shown(&out.join(REPORT_FILE_NAME)),
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

#[cfg(feature = "graphics")]
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
