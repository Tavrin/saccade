//! The whole comparison: pair two directories, compare, write the report.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

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
                ["png", "jpg", "jpeg", "exr", "hdr"]
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

/// Hex SHA-256 of a file's bytes.
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let ctx = || format!("hashing {}", path.display());
    let mut file = std::fs::File::open(path).map_err(io_err(ctx()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(io_err(ctx()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Sentinel a run writes into its output directory while it is incomplete, so
/// a failed run's leftovers are still recognised as flipdiff's own.
pub const RUN_SENTINEL: &str = ".flipdiff-run";

/// `path` with `..` and `.` folded away and the longest existing prefix
/// canonicalised (symlinks resolved), so a not-yet-created output directory
/// can be compared with an input.
pub fn normalise_path(path: &Path) -> PathBuf {
    let mut folded = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                folded.pop();
            }
            std::path::Component::CurDir => {}
            other => folded.push(other.as_os_str()),
        }
    }
    let mut existing = folded.clone();
    let mut tail = Vec::new();
    while !existing.exists() {
        match existing.file_name().map(std::ffi::OsStr::to_owned) {
            Some(name) => tail.push(name),
            None => break,
        }
        if !existing.pop() {
            break;
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for name in tail.into_iter().rev() {
        out.push(name);
    }
    out
}

/// The one check every command that writes an output directory runs first.
///
/// Refuses (as [`Error::Config`], exit code 2) when `out` is, or is inside,
/// one of `inputs`, or when an input lives under `out/images` (which a run
/// clears), and when `out` exists, is not empty and holds none of `markers`:
/// a directory that is not a previous flipdiff output is never cleared or
/// overwritten.
pub fn guard_output_dir(out: &Path, inputs: &[&Path], markers: &[&str]) -> Result<()> {
    let norm = normalise_path(out);
    for input in inputs {
        let input = normalise_path(input);
        if norm.starts_with(&input) || input.starts_with(norm.join("images")) {
            return Err(Error::Config(format!(
                "the output directory {} must not be inside an input directory ({})",
                out.display(),
                input.display()
            )));
        }
    }
    let mut entries = match std::fs::read_dir(out) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(Error::Io {
                context: format!("reading output directory {}", out.display()),
                source: e,
            });
        }
    };
    if entries.next().is_none() || markers.iter().any(|m| out.join(m).is_file()) {
        return Ok(());
    }
    Err(Error::NotEmptyOutDir(format!(
        "the output directory {} is not empty and is not a previous flipdiff output (no {}); \
         refusing to clear or overwrite it. Use an empty or new directory",
        out.display(),
        markers.join(" or ")
    )))
}

/// Removes what a previous run left in `report_dir` (the report JSON, the
/// HTML page and `images/`), and nothing else, so a failed run cannot leave a
/// stale report behind.
pub(crate) fn clear_previous_report(report_dir: &Path) -> Result<()> {
    for leaf in [
        REPORT_FILE_NAME,
        "index.html",
        "images",
        // Answers recorded against the previous report do not carry over.
        crate::decision::DECISIONS_FILE_NAME,
        crate::decision::DECISIONS_SIDECAR_NAME,
    ] {
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

/// Prints a stderr warning for each `[[region]]` or `[[mask]]` glob that
/// matches no image of the run (usually a typo that silently disables it).
fn warn_unmatched_globs(config: &RunConfig, entries: &[Entry]) {
    let globs = config
        .regions
        .iter()
        .map(|r| ("region", r.glob.as_deref()))
        .chain(config.masks.iter().map(|m| ("mask", m.glob.as_deref())));
    for (kind, glob) in globs {
        let Some(glob) = glob else { continue };
        let Ok(matcher) = compile_glob(glob) else {
            continue;
        };
        if !entries.iter().any(|e| matcher.is_match(&e.name)) {
            eprintln!("flipdiff: warning: {kind} glob {glob:?} matches no image in this run");
        }
    }
}

pub(crate) fn status_of(value: f64, threshold: f64) -> Status {
    // `!(<=)` so a NaN value fails rather than passes.
    if value <= threshold {
        Status::Pass
    } else {
        Status::Fail
    }
}

pub(crate) fn metric_value(m: &Metrics, metric: Metric) -> f64 {
    match metric {
        Metric::Mean => m.mean,
        Metric::P95 => m.p95,
        Metric::P99 => m.p99,
        Metric::Max => m.max,
    }
}

/// Compares every image in `capture_dir` with its namesake in `baseline_dir`,
/// writes `<report_dir>/flipdiff-report.v1.json` plus per-image copies and
/// heatmaps, renders the HTML report and returns the [`Report`].
///
/// `report_dir` must not be inside either input and must be empty, absent or a
/// previous flipdiff report ([`guard_output_dir`]); anything else is refused
/// before a file is touched. Previous report files in it
/// (`flipdiff-report.v1.json`, `index.html`, `images/`) are removed first, so
/// a failed run leaves no stale report. Pairs are compared in parallel; the
/// entries stay sorted by name. Only directory, config and report-write
/// failures return `Err`; per-image problems (decode, copy, walk errors,
/// symlinks) become `error` entries.
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

    guard_output_dir(
        report_dir,
        &[baseline_dir, capture_dir],
        &[REPORT_FILE_NAME, RUN_SENTINEL],
    )?;
    std::fs::create_dir_all(report_dir)
        .map_err(io_err(format!("creating {}", report_dir.display())))?;
    clear_previous_report(report_dir)?;
    let sentinel = report_dir.join(RUN_SENTINEL);
    std::fs::write(&sentinel, b"incomplete flipdiff run\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;
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
        hdr: config.hdr,
    };
    // Timing keys are performance data, not capture configuration: they are
    // read for the perf pairing and never fail `--require-matching-meta`.
    let mut meta_options = config.meta.clone();
    if config.diagnostics.enabled {
        for k in &config.diagnostics.perf_keys {
            if !meta_options.ignore.contains(k)
                && !crate::meta::DEFAULT_IGNORE.contains(&k.as_str())
            {
                meta_options.ignore.push(k.clone());
            }
        }
    }
    let meta = meta_options.checker()?;
    let work: Vec<_> = names
        .into_iter()
        .filter(|(name, _)| !ignore.iter().any(|m| m.is_match(name)))
        .collect();
    // Pairs are independent; an indexed parallel collect keeps the name order.
    let entries: Vec<Entry> = work
        .par_iter()
        .map(|&(name, (base, cap))| {
            let both_files = matches!((base, cap), (Some(Source::File(_)), Some(Source::File(_))));
            let mut entry = build_entry(name, base, cap, report_dir, &opts, config);
            if both_files {
                apply_meta(&mut entry, &meta, baseline_dir, capture_dir);
                apply_perf(&mut entry, &meta, baseline_dir, capture_dir, config);
            }
            apply_image_warnings(&mut entry, config);
            entry
        })
        .collect();
    warn_unmatched_globs(config, &entries);

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
        baseline_dir: Some(normalise_path(baseline_dir).display().to_string()),
        capture_dir: Some(normalise_path(capture_dir).display().to_string()),
        config: ReportConfig {
            default_threshold: config.default_threshold,
            default_metric: config.default_metric,
            pixels_per_degree: config.pixels_per_degree,
            fail_on_new: config.fail_on_new,
            mode: config.mode,
            labels: config.labels.clone(),
            meta: meta.settings(),
            allow_empty: config.allow_empty,
            fail_on_nonfinite: config.fail_on_nonfinite,
            hotspot_fail: config.hotspot_fail,
        },
        totals,
        entries,
    };
    if report.is_empty_run() {
        let why = if baselines.files.is_empty() && report.totals.new > 0 {
            format!(
                "the baseline directory {} has no images, so every capture is new",
                baseline_dir.display()
            )
        } else {
            "no image exists in both directories".to_string()
        };
        eprintln!(
            "flipdiff: warning: nothing compared: {why}; this is a failure unless --allow-empty is given"
        );
    }

    let json_path = report_dir.join(REPORT_FILE_NAME);
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, json).map_err(io_err(format!("writing {}", json_path.display())))?;
    render::render_html(&report, report_dir)?;
    // The report is complete: the JSON is now the ownership marker.
    std::fs::remove_file(&sentinel).map_err(io_err(format!("removing {}", sentinel.display())))?;
    Ok(report)
}

/// Fills `entry.warnings` from the structural checks and applies
/// `fail_on_nonfinite`: an all-black or all-white side, or non-finite samples,
/// is reported; non-finite samples in the capture also make the entry an
/// `error` when the config says so.
pub(crate) fn apply_image_warnings(entry: &mut Entry, config: &RunConfig) {
    let sides = [
        ("capture", entry.properties),
        ("baseline", entry.baseline_properties),
    ];
    for (side, props) in sides {
        let Some(p) = props else { continue };
        if p.is_all_black {
            entry.warnings.push(format!("{side} is all black"));
        }
        if p.is_all_white {
            entry.warnings.push(format!("{side} is all white"));
        }
        // A failing capture is reported by the entry's error instead.
        let reported_as_error = side == "capture" && config.fail_on_nonfinite;
        if (p.nan_count > 0 || p.inf_count > 0) && !reported_as_error {
            entry.warnings.push(format!(
                "{side} has non-finite samples ({} NaN, {} infinite)",
                p.nan_count, p.inf_count
            ));
        }
    }
    let Some(p) = entry.properties else { return };
    if config.fail_on_nonfinite && (p.nan_count > 0 || p.inf_count > 0) {
        let msg = format!(
            "capture has non-finite samples ({} NaN, {} infinite); \
             set fail_on_nonfinite = false to accept",
            p.nan_count, p.inf_count
        );
        entry.status = Status::Error;
        entry.error = Some(match entry.error.take() {
            Some(prev) => format!("{prev}; {msg}"),
            None => msg,
        });
    }
}

/// One side of a pair: a usable file, or the reason there is none.
#[derive(Clone, Copy)]
pub(crate) enum Source<'a> {
    File(&'a Path),
    Problem(&'a str),
}

/// Reads both sidecars of a compared pair into `entry.meta_diff`. An unreadable
/// or nested sidecar, or (when required) an undeclared differing key, turns
/// the entry into an `error` that names the cause; the metrics stay.
fn apply_meta(
    entry: &mut Entry,
    meta: &crate::meta::MetaChecker,
    baseline_dir: &Path,
    capture_dir: &Path,
) {
    let failure = match meta.compare_split(baseline_dir, capture_dir, &entry.name) {
        Ok((diff, ignored, failure)) => {
            entry.meta_diff = diff;
            entry.meta_ignored_diff = ignored;
            failure
        }
        Err(e) => Some(e),
    };
    if let Some(msg) = failure {
        entry.status = Status::Error;
        entry.error = Some(match entry.error.take() {
            Some(prev) => format!("{prev}; {msg}"),
            None => msg,
        });
    }
}

/// Pairs the timing keys of both sidecars next to the image verdict.
fn apply_perf(
    entry: &mut Entry,
    meta: &crate::meta::MetaChecker,
    baseline_dir: &Path,
    capture_dir: &Path,
    config: &RunConfig,
) {
    let Some(d) = entry.diagnostics.as_mut() else {
        return;
    };
    d.perf = crate::diagnostics::perf_pairs(
        meta,
        &config.diagnostics,
        baseline_dir,
        capture_dir,
        &entry.name,
    );
}

/// Both sides of a pair as the diagnostics engine reads them.
struct PairPixels<'a> {
    baseline: crate::diagnostics::Pixels<'a>,
    capture: crate::diagnostics::Pixels<'a>,
    capture_path: &'a Path,
    flip: &'a CompareOptions,
}

/// Runs the diagnostics engine and records its findings on `entry`; a failure
/// to write one of its images is a warning, not an error of the pair.
fn run_diagnostics(
    entry: &mut Entry,
    cmp: &crate::compare::Comparison,
    pair: &PairPixels<'_>,
    report_dir: &Path,
    config: &RunConfig,
) {
    use crate::diagnostics::{DiagOut, DiagnoseRequest, diagnose};
    if !config.diagnostics.enabled {
        return;
    }
    let req = DiagnoseRequest {
        baseline: pair.baseline,
        capture: pair.capture,
        comparison: cmp,
        flip: pair.flip,
        bit_identical: entry.bit_identical,
        baseline_properties: entry.baseline_properties,
        capture_properties: entry.properties,
        hotspots: &entry.hotspots,
        hotspot_options: crate::hotspots::HotspotOptions {
            threshold: config.hotspot_threshold,
            top_k: config.hotspots,
            min_share: config.hotspot_min_share,
        },
        config: &config.diagnostics,
        capture_path: Some(pair.capture_path),
        out: Some(DiagOut {
            report_dir,
            name: &entry.name,
        }),
    };
    match diagnose(&req) {
        Ok(out) => {
            entry.paths.signed_diff = out.signed_diff;
            entry.paths.nonfinite_mask = out.nonfinite_mask;
            entry.diagnostics = Some(out.diagnostics);
        }
        Err(e) => entry.warnings.push(format!("diagnostics failed: {e}")),
    }
}

pub(crate) fn build_entry(
    name: &str,
    base: Option<Source<'_>>,
    cap: Option<Source<'_>>,
    report_dir: &Path,
    opts: &CompareOptions,
    config: &RunConfig,
) -> Entry {
    let (metric_used, threshold) = config.effective_for(name);
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
        regions: Vec::new(),
        masked_fraction: None,
        bit_identical: None,
        hdr: None,
        meta_diff: Vec::new(),
        meta_ignored_diff: Vec::new(),
        baseline_properties: None,
        warnings: Vec::new(),
        baseline_sha256: None,
        capture_sha256: None,
        hotspots: Vec::new(),
        buffer: None,
        diagnostics: None,
    };
    if let Err(e) = fill_entry(&mut entry, base, cap, report_dir, opts, config) {
        entry.status = Status::Error;
        entry.value = None;
        entry.metrics = None;
        entry.paths.heatmap = None;
        entry.regions.clear();
        entry.masked_fraction = None;
        entry.hotspots.clear();
        entry.diagnostics = None;
        entry.paths.signed_diff = None;
        entry.paths.nonfinite_mask = None;
        entry.buffer = None;
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
    config: &RunConfig,
) -> Result<()> {
    let name = entry.name.clone();
    let mut problems = Vec::new();
    for (side, src) in [("baseline", base), ("capture", cap)] {
        if let Some(Source::Problem(why)) = src {
            problems.push(format!("{side}: {why}"));
        }
    }
    if let Some(Source::File(p)) = base {
        entry.baseline_sha256 = sha256_file(p).ok();
        entry.paths.baseline = copy_entry_side(p, report_dir, &name, "baseline", config)?;
    }
    if let Some(Source::File(p)) = cap {
        entry.capture_sha256 = sha256_file(p).ok();
        entry.paths.capture = copy_entry_side(p, report_dir, &name, "capture", config)?;
    }
    if !problems.is_empty() {
        entry.error = Some(problems.join("; "));
        return Ok(());
    }
    if let Some(spec) = config.buffer_for(&name) {
        entry.metric_used = spec.metric;
        entry.threshold = spec.threshold();
        match (base, cap) {
            (Some(Source::File(b)), Some(Source::File(c))) => {
                crate::buffer::fill_pair(entry, b, c, report_dir, spec)?;
            }
            (None, Some(Source::File(c))) => {
                crate::buffer::decode(c, spec)?;
                entry.status = Status::New;
            }
            (Some(Source::File(_)), None) => entry.status = Status::Missing,
            _ => {}
        }
        return Ok(());
    }
    let (base, cap) = match (base, cap) {
        (Some(Source::File(b)), Some(Source::File(c))) => {
            crate::hdr::check_same_kind(b, c)?;
            if crate::hdr::is_hdr_path(b) {
                return fill_hdr_pair(entry, b, c, report_dir, opts, config);
            }
            (b, c)
        }
        (None, Some(Source::File(c))) if crate::hdr::is_hdr_path(c) => {
            match crate::hdr::decode_hdr(c) {
                Ok(img) => {
                    entry.status = Status::New;
                    entry.properties = Some(crate::hdr::validate_hdr(&img));
                }
                Err(e) => entry.error = Some(e.to_string()),
            }
            return Ok(());
        }
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
    entry.baseline_properties = Some(properties::validate(&flatten_over(&base_img, 0)));
    entry.bit_identical = Some(crate::compare::native_samples_identical(base, cap));
    match compare_rgba(&cap_img, &base_img, opts) {
        Ok(cmp) => {
            let pair = PairPixels {
                baseline: crate::diagnostics::Pixels::Ldr(&base_img),
                capture: crate::diagnostics::Pixels::Ldr(&cap_img),
                capture_path: cap,
                flip: opts,
            };
            finish_entry(entry, cmp, report_dir, config, &pair)?;
        }
        Err(e) => entry.error = Some(e.to_string()),
    }
    Ok(())
}

/// Turns a finished comparison into the entry's status, value, metrics and
/// heatmap (regions and masks included).
fn finish_entry(
    entry: &mut Entry,
    cmp: crate::compare::Comparison,
    report_dir: &Path,
    config: &RunConfig,
    pair: &PairPixels<'_>,
) -> Result<()> {
    let name = entry.name.clone();
    let scene = crate::regions::evaluate(entry, &cmp, config)?;
    let value = metric_value(&scene.metrics, entry.metric_used);
    let rel = format!("images/{name}/heatmap.png");
    let dest = report_dir.join(&rel);
    let mut heatmap = cmp.heatmap_rgb();
    if let Some(mask) = &scene.mask {
        crate::compare::hatch_masked(&mut heatmap, mask);
    }
    heatmap
        .save(&dest)
        .map_err(|source| Error::Encode { path: dest, source })?;
    entry.hotspots = crate::hotspots::find_hotspots(
        &cmp.error_map,
        scene.mask.as_deref(),
        cmp.metrics.width,
        cmp.metrics.height,
        &crate::hotspots::HotspotOptions {
            threshold: config.hotspot_threshold,
            top_k: config.hotspots,
            min_share: config.hotspot_min_share,
        },
    );
    entry.status = crate::regions::combine(status_of(value, entry.threshold), &entry.regions);
    // A small severe defect barely moves the mean: `hotspot_fail` fails the
    // entry when the unmasked peak error reaches it (validated to be at least
    // `hotspot_threshold`, so some hotspot contains that pixel).
    let hotspot_limit = config
        .hotspot_fail
        .filter(|&limit| entry.status == Status::Pass && scene.metrics.max >= limit);
    if let Some(limit) = hotspot_limit {
        entry.status = Status::Fail;
        entry.warnings.push(format!(
            "hotspot_fail: local peak error {:.4} reaches {limit}",
            scene.metrics.max
        ));
    }
    entry.value = Some(value);
    entry.metrics = Some(scene.metrics);
    entry.paths.heatmap = Some(rel);
    run_diagnostics(entry, &cmp, pair, report_dir, config);
    Ok(())
}

/// Copies `src` into the report and returns the path the report should show
/// for this side. LDR files are copied as `<stem>.<ext>`. An HDR file is
/// copied as `<stem>.orig.<ext>` and a tone-mapped display PNG `<stem>.png`
/// is written next to it and returned; `None` when the HDR file cannot be
/// decoded (the comparison then reports the decode error).
fn copy_entry_side(
    src: &Path,
    report_dir: &Path,
    name: &str,
    stem: &str,
    config: &RunConfig,
) -> Result<Option<String>> {
    if let Some(spec) = config
        .buffer_for(name)
        .filter(|_| crate::hdr::is_hdr_path(src))
    {
        copy_into_report(src, report_dir, name, &format!("{stem}.orig"))?;
        let Ok(img) = crate::buffer::decode(src, spec) else {
            return Ok(None);
        };
        let rel = format!("images/{name}/{stem}.png");
        let path = report_dir.join(&rel);
        img.to_rgb8()
            .save(&path)
            .map_err(|source| Error::Encode { path, source })?;
        return Ok(Some(rel));
    }
    copy_side(src, report_dir, name, stem, config)
}

fn copy_side(
    src: &Path,
    report_dir: &Path,
    name: &str,
    stem: &str,
    config: &RunConfig,
) -> Result<Option<String>> {
    if !crate::hdr::is_hdr_path(src) {
        return copy_into_report(src, report_dir, name, stem).map(Some);
    }
    copy_into_report(src, report_dir, name, &format!("{stem}.orig"))?;
    let Ok(img) = crate::hdr::decode_hdr(src) else {
        return Ok(None);
    };
    let rel = format!("images/{name}/{stem}.png");
    let dest = report_dir.join(&rel);
    crate::hdr::display_image(&img, config.hdr.tonemapper)
        .save(&dest)
        .map_err(|source| Error::Encode { path: dest, source })?;
    Ok(Some(rel))
}

/// The HDR branch of [`fill_entry`]: HDR-FLIP over the baseline's exposure
/// range, with linear-luminance properties and a bit-exact `f32` identity flag.
fn fill_hdr_pair(
    entry: &mut Entry,
    base: &Path,
    cap: &Path,
    report_dir: &Path,
    opts: &CompareOptions,
    config: &RunConfig,
) -> Result<()> {
    let cap_img = match crate::hdr::decode_hdr(cap) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.properties = Some(crate::hdr::validate_hdr(&cap_img));
    let base_img = match crate::hdr::decode_hdr(base) {
        Ok(i) => i,
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.baseline_properties = Some(crate::hdr::validate_hdr(&base_img));
    entry.bit_identical = Some(crate::compare::native_samples_identical(base, cap));
    match crate::hdr::compare_hdr(&cap_img, &base_img, opts) {
        Ok((cmp, info)) => {
            entry.hdr = Some(info);
            let pair = PairPixels {
                baseline: crate::diagnostics::Pixels::Hdr(&base_img),
                capture: crate::diagnostics::Pixels::Hdr(&cap_img),
                capture_path: cap,
                flip: opts,
            };
            finish_entry(entry, cmp, report_dir, config, &pair)?;
        }
        Err(e) => entry.error = Some(e.to_string()),
    }
    Ok(())
}
