//! The whole comparison: pair two directories, compare, write the report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::compare::{CompareOptions, compare};
use crate::config::{RunConfig, compile_glob};
use crate::error::{Error, Result};
use crate::properties;
use crate::render;
use crate::report::{
    Entry, EntryPaths, Metric, Metrics, REPORT_FILE_NAME, REPORT_SCHEMA, Report, ReportConfig,
    Status, Totals,
};

fn io_err(context: String) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io { context, source }
}

/// Recursively lists image files under `root` as `/`-separated relative names.
fn collect_images(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    if !root.is_dir() {
        return Err(Error::Io {
            context: format!("{} is not a directory", root.display()),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    }
    let mut out = BTreeMap::new();
    for item in walkdir::WalkDir::new(root).follow_links(true) {
        let item = item.map_err(|e| Error::Io {
            context: format!("walking {}", root.display()),
            source: e.into(),
        })?;
        if !item.file_type().is_file() {
            continue;
        }
        let is_image = item
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                ["png", "jpg", "jpeg"]
                    .iter()
                    .any(|known| e.eq_ignore_ascii_case(known))
            });
        if !is_image {
            continue;
        }
        let Ok(rel) = item.path().strip_prefix(root) else {
            continue;
        };
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        out.insert(name, item.path().to_path_buf());
    }
    Ok(out)
}

fn decode(path: &Path) -> std::result::Result<image::RgbImage, Error> {
    image::open(path)
        .map(|i| i.to_rgb8())
        .map_err(|source| Error::Decode {
            path: path.to_path_buf(),
            source,
        })
}

/// Copies `src` to `<report>/images/<name>/<stem>.<ext>`, returning the path
/// relative to the report directory.
fn copy_into_report(src: &Path, report_dir: &Path, name: &str, stem: &str) -> Result<String> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let rel = format!("images/{name}/{stem}.{ext}");
    let dest = report_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(io_err(format!("creating {}", parent.display())))?;
    }
    std::fs::copy(src, &dest).map_err(io_err(format!(
        "copying {} to {}",
        src.display(),
        dest.display()
    )))?;
    Ok(rel)
}

fn status_of(value: f64, threshold: f64) -> Status {
    // `!(<=)` so a NaN value fails rather than passes.
    if value <= threshold {
        Status::Pass
    } else {
        Status::Fail
    }
}

fn metric_value(m: &Metrics, metric: Metric) -> f64 {
    match metric {
        Metric::Mean => m.mean,
        Metric::P95 => m.p95,
        Metric::Max => m.max,
    }
}

/// Compares every image in `capture_dir` with its namesake in `baseline_dir`,
/// writes `<report_dir>/flipdiff-report.v1.json` plus per-image copies and
/// heatmaps, renders the HTML report and returns the [`Report`].
///
/// Only directory, config and report-write failures return `Err`; per-image
/// problems become `error` entries.
pub fn run(
    baseline_dir: &Path,
    capture_dir: &Path,
    report_dir: &Path,
    config: &RunConfig,
) -> Result<Report> {
    config.validate()?;
    let ignore: Vec<_> = config
        .ignore
        .iter()
        .map(|g| compile_glob(g))
        .collect::<Result<_>>()?;

    let baselines = collect_images(baseline_dir)?;
    let captures = collect_images(capture_dir)?;
    std::fs::create_dir_all(report_dir)
        .map_err(io_err(format!("creating {}", report_dir.display())))?;

    let mut names: BTreeMap<&str, (Option<&PathBuf>, Option<&PathBuf>)> = BTreeMap::new();
    for (n, p) in &baselines {
        names.entry(n).or_default().0 = Some(p);
    }
    for (n, p) in &captures {
        names.entry(n).or_default().1 = Some(p);
    }

    let opts = CompareOptions {
        pixels_per_degree: config.pixels_per_degree,
    };
    let mut entries = Vec::new();
    for (name, (base, cap)) in names {
        if ignore.iter().any(|m| m.is_match(name)) {
            continue;
        }
        let (metric_used, threshold) = config.effective_for(name);
        entries.push(build_entry(
            name,
            base.map(PathBuf::as_path),
            cap.map(PathBuf::as_path),
            report_dir,
            &opts,
            metric_used,
            threshold,
        )?);
    }

    let mut totals = Totals {
        total: entries.len(),
        ..Totals::default()
    };
    for e in &entries {
        match e.status {
            Status::Pass => totals.pass += 1,
            Status::Fail => totals.fail += 1,
            Status::New => totals.new += 1,
            Status::Missing => totals.missing += 1,
            Status::Error => totals.error += 1,
        }
    }
    let report = Report {
        schema: REPORT_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        config: ReportConfig {
            default_threshold: config.default_threshold,
            default_metric: config.default_metric,
            pixels_per_degree: config.pixels_per_degree,
            fail_on_new: config.fail_on_new,
        },
        totals,
        entries,
    };

    let json_path = report_dir.join(REPORT_FILE_NAME);
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, json).map_err(io_err(format!("writing {}", json_path.display())))?;
    render::render_html(&report, report_dir)?;
    Ok(report)
}

fn build_entry(
    name: &str,
    base: Option<&Path>,
    cap: Option<&Path>,
    report_dir: &Path,
    opts: &CompareOptions,
    metric_used: Metric,
    threshold: f64,
) -> Result<Entry> {
    let mut entry = Entry {
        name: name.to_string(),
        status: Status::Error,
        metric_used,
        threshold,
        value: None,
        metrics: None,
        properties: None,
        paths: EntryPaths::default(),
        error: None,
    };
    if let Some(p) = base {
        entry.paths.baseline = Some(copy_into_report(p, report_dir, name, "baseline")?);
    }
    if let Some(p) = cap {
        entry.paths.capture = Some(copy_into_report(p, report_dir, name, "capture")?);
    }
    let (base, cap) = match (base, cap) {
        (Some(b), Some(c)) => (b, c),
        (None, Some(c)) => {
            entry.status = Status::New;
            entry.properties = decode(c).ok().map(|i| properties::validate(&i));
            return Ok(entry);
        }
        (Some(_), None) => {
            entry.status = Status::Missing;
            return Ok(entry);
        }
        (None, None) => return Ok(entry),
    };

    let cap_img = match decode(cap) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(entry);
        }
    };
    entry.properties = Some(properties::validate(&cap_img));
    let base_img = match decode(base) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(entry);
        }
    };
    match compare(&cap_img, &base_img, opts) {
        Ok(cmp) => {
            let value = metric_value(&cmp.metrics, metric_used);
            entry.status = status_of(value, threshold);
            entry.value = Some(value);
            entry.metrics = Some(cmp.metrics);
            let rel = format!("images/{name}/heatmap.png");
            let dest = report_dir.join(&rel);
            cmp.heatmap_rgb()
                .save(&dest)
                .map_err(|source| Error::Encode { path: dest, source })?;
            entry.paths.heatmap = Some(rel);
        }
        Err(e) => entry.error = Some(e.to_string()),
    }
    Ok(entry)
}
