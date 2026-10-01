//! The whole comparison: pair two directories, compare, write the report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::compare::{CompareOptions, compare_rgba, flatten_over};
use crate::config::{RunConfig, compile_glob};
use crate::error::{Error, Result};
use crate::properties;
use crate::render;
use crate::report::{
    Entry, EntryPaths, Metric, Metrics, REPORT_FILE_NAME, REPORT_SCHEMA, Report, ReportConfig,
    Status, Totals,
};

pub(crate) fn io_err(context: String) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io { context, source }
}

/// Image files found under a root, plus entries that could not be listed.
#[derive(Debug, Default)]
pub(crate) struct Collected {
    /// `/`-separated relative name to file path.
    pub files: BTreeMap<String, PathBuf>,
    /// `/`-separated relative name to the reason it cannot be used.
    pub problems: BTreeMap<String, String>,
}

fn relative_name(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// Recursively lists image files under `root`. Symbolic links are not
/// followed: a link becomes a problem entry. Walk errors that name a path
/// below `root` become problem entries too; only an unusable `root` is `Err`.
pub(crate) fn collect_images(root: &Path) -> Result<Collected> {
    if !root.is_dir() {
        return Err(Error::Io {
            context: format!("{} is not a directory", root.display()),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        });
    }
    let mut out = Collected::default();
    for item in walkdir::WalkDir::new(root).follow_links(false) {
        let item = match item {
            Ok(item) => item,
            Err(e) => {
                let name = e.path().and_then(|p| relative_name(root, p));
                match name {
                    Some(name) => {
                        out.problems.insert(name, format!("cannot read: {e}"));
                        continue;
                    }
                    None => {
                        return Err(Error::Io {
                            context: format!("walking {}", root.display()),
                            source: e.into(),
                        });
                    }
                }
            }
        };
        let Some(name) = relative_name(root, item.path()) else {
            continue;
        };
        if item.path_is_symlink() {
            out.problems
                .insert(name, "symlinks are not followed".to_string());
            continue;
        }
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
        if is_image {
            out.files.insert(name, item.path().to_path_buf());
        }
    }
    Ok(out)
}

/// Decodes an image to 8-bit RGBA (16-bit and other formats are converted).
pub(crate) fn decode(path: &Path) -> std::result::Result<image::RgbaImage, Error> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|source| Error::Decode {
            path: path.to_path_buf(),
            source,
        })
}

/// Whether `path` decodes as an image.
pub fn is_decodable(path: &Path) -> bool {
    decode(path).is_ok()
}

/// Removes what a previous run left in `report_dir` (the report JSON, the
/// HTML page and `images/`), and nothing else, so a failed run cannot leave a
/// stale report behind.
fn clear_previous_report(report_dir: &Path) -> Result<()> {
    for leaf in [REPORT_FILE_NAME, "index.html", "images"] {
        let path = report_dir.join(leaf);
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let removed = if meta.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(io_err(format!("removing stale {}", path.display())))?;
    }
    Ok(())
}

/// Errors when the report directory is one of the input directories, because
/// clearing it would delete inputs.
fn check_report_dir(report_dir: &Path, baseline_dir: &Path, capture_dir: &Path) -> Result<()> {
    let Ok(report) = report_dir.canonicalize() else {
        return Ok(());
    };
    for (what, dir) in [("baseline", baseline_dir), ("capture", capture_dir)] {
        if dir.canonicalize().is_ok_and(|d| d == report) {
            return Err(Error::Config(format!(
                "the report directory {} is the {what} directory",
                report_dir.display()
            )));
        }
    }
    Ok(())
}

/// Copies `src` to `<report>/images/<name>/<stem>.<ext>`, returning the path
/// relative to the report directory.
pub(crate) fn copy_into_report(
    src: &Path,
    report_dir: &Path,
    name: &str,
    stem: &str,
) -> Result<String> {
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
/// Previous report files in `report_dir` (`flipdiff-report.v1.json`,
/// `index.html`, `images/`) are removed first, so a failed run leaves no stale
/// report. Only directory, config and report-write failures return `Err`;
/// per-image problems (decode, copy, walk errors, symlinks) become `error`
/// entries.
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

    check_report_dir(report_dir, baseline_dir, capture_dir)?;
    std::fs::create_dir_all(report_dir)
        .map_err(io_err(format!("creating {}", report_dir.display())))?;
    clear_previous_report(report_dir)?;
    let baselines = collect_images(baseline_dir)?;
    let captures = collect_images(capture_dir)?;

    let mut names: BTreeMap<&str, (Option<Source<'_>>, Option<Source<'_>>)> = BTreeMap::new();
    for (n, p) in &baselines.files {
        names.entry(n).or_default().0 = Some(Source::File(p));
    }
    for (n, why) in &baselines.problems {
        names.entry(n).or_default().0 = Some(Source::Problem(why));
    }
    for (n, p) in &captures.files {
        names.entry(n).or_default().1 = Some(Source::File(p));
    }
    for (n, why) in &captures.problems {
        names.entry(n).or_default().1 = Some(Source::Problem(why));
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
            base,
            cap,
            report_dir,
            &opts,
            metric_used,
            threshold,
        ));
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

/// One side of a pair: a usable file, or the reason there is none.
#[derive(Clone, Copy)]
enum Source<'a> {
    File(&'a Path),
    Problem(&'a str),
}

fn build_entry(
    name: &str,
    base: Option<Source<'_>>,
    cap: Option<Source<'_>>,
    report_dir: &Path,
    opts: &CompareOptions,
    metric_used: Metric,
    threshold: f64,
) -> Entry {
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
    if let Err(e) = fill_entry(&mut entry, base, cap, report_dir, opts) {
        entry.status = Status::Error;
        entry.value = None;
        entry.metrics = None;
        entry.paths.heatmap = None;
        entry.error = Some(e.to_string());
    }
    entry
}

fn fill_entry(
    entry: &mut Entry,
    base: Option<Source<'_>>,
    cap: Option<Source<'_>>,
    report_dir: &Path,
    opts: &CompareOptions,
) -> Result<()> {
    let name = entry.name.clone();
    let mut problems = Vec::new();
    for (side, src) in [("baseline", base), ("capture", cap)] {
        if let Some(Source::Problem(why)) = src {
            problems.push(format!("{side}: {why}"));
        }
    }
    if let Some(Source::File(p)) = base {
        entry.paths.baseline = Some(copy_into_report(p, report_dir, &name, "baseline")?);
    }
    if let Some(Source::File(p)) = cap {
        entry.paths.capture = Some(copy_into_report(p, report_dir, &name, "capture")?);
    }
    if !problems.is_empty() {
        entry.error = Some(problems.join("; "));
        return Ok(());
    }
    let (base, cap) = match (base, cap) {
        (Some(Source::File(b)), Some(Source::File(c))) => (b, c),
        (None, Some(Source::File(c))) => {
            match decode(c) {
                Ok(img) => {
                    entry.status = Status::New;
                    entry.properties = Some(properties::validate(&flatten_over(&img, 0)));
                }
                Err(e) => entry.error = Some(e.to_string()),
            }
            return Ok(());
        }
        (Some(Source::File(_)), None) => {
            entry.status = Status::Missing;
            return Ok(());
        }
        _ => return Ok(()),
    };

    let cap_img = match decode(cap) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.properties = Some(properties::validate(&flatten_over(&cap_img, 0)));
    let base_img = match decode(base) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    match compare_rgba(&cap_img, &base_img, opts) {
        Ok(cmp) => {
            let value = metric_value(&cmp.metrics, entry.metric_used);
            let rel = format!("images/{name}/heatmap.png");
            let dest = report_dir.join(&rel);
            cmp.heatmap_rgb()
                .save(&dest)
                .map_err(|source| Error::Encode { path: dest, source })?;
            entry.status = status_of(value, entry.threshold);
            entry.value = Some(value);
            entry.metrics = Some(cmp.metrics);
            entry.paths.heatmap = Some(rel);
        }
        Err(e) => entry.error = Some(e.to_string()),
    }
    Ok(())
}
