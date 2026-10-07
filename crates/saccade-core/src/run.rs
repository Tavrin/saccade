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
    Entry, EntryPaths, Metric, Metrics, Mode, REPORT_FILE_NAME, REPORT_SCHEMA, Report,
    ReportConfig, Status, Totals,
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
        let source = if root.exists() {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "this is a file; pass the directory that contains the images",
            )
        } else {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no such directory")
        };
        return Err(Error::Io {
            context: root.display().to_string(),
            source,
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
        if crate::object_ids::is_sidecar_image_name(&name) {
            continue;
        }
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
                ["png", "jpg", "jpeg", "exr", "hdr", "svg", "pdf"]
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
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("svg") || s.eq_ignore_ascii_case("pdf"))
    {
        return crate::general::input::load(path);
    }
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|source| Error::Decode {
            path: path.to_path_buf(),
            source,
        })
}

// Record actual native decoding losses beside the authoritative comparison.
fn decode_audited(
    path: &Path,
    hdr: bool,
    side: &str,
    losses: &mut Vec<String>,
) -> Result<image::DynamicImage> {
    let img = if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("svg") || s.eq_ignore_ascii_case("pdf"))
    {
        image::DynamicImage::ImageRgba8(crate::general::input::load(path)?)
    } else {
        image::open(path).map_err(|source| Error::Decode {
            path: path.to_path_buf(),
            source,
        })?
    };
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("svg") || s.eq_ignore_ascii_case("pdf"))
    {
        losses.push(format!("{side}: document rasterized at 96 DPI using the optional bounded document adapter; native sample identity covers this raster only."));
    }
    let colour = img.color();
    let bits = colour.bits_per_pixel() / u16::from(colour.channel_count());
    if hdr {
        if colour.has_alpha() {
            losses.push(format!("{side}: alpha dropped by HDR RGB decoding; alpha differences do not affect HDR-FLIP."));
        }
        losses.push(format!("{side}: HDR to SDR tone mapping and 8-bit quantization affect display previews only; HDR-FLIP uses linear f32 RGB."));
    } else if bits > 8 {
        losses.push(format!("{side}: {bits}-bit to 8-bit sample reduction for SDR-FLIP; native precision differences may be omitted."));
        if matches!(colour, image::ColorType::Rgb32F | image::ColorType::Rgba32F) {
            losses.push(format!(
                "{side}: HDR to SDR conversion clips float samples outside [0,1]."
            ));
        }
    }
    Ok(img)
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
/// a failed run's leftovers are still recognised as saccade's own.
pub const RUN_SENTINEL: &str = ".saccade-run";

/// `path` with the longest existing prefix canonicalised (symlinks resolved)
/// before folding the missing suffix's `..` and `.`, so a not-yet-created
/// output directory can be compared with an input. Resolving before folding
/// preserves the filesystem meaning of `symlink/..`.
pub fn normalise_path(path: &Path) -> PathBuf {
    let path = crate::paths::native(path);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path.as_ref()))
    };
    let mut existing = path.clone();
    let mut tail = Vec::new();
    let mut out = loop {
        if let Ok(canonical) = crate::paths::canonicalize(&existing) {
            break canonical;
        }
        match existing.components().next_back() {
            Some(c @ (std::path::Component::Normal(_) | std::path::Component::ParentDir)) => {
                tail.push(c.as_os_str().to_owned());
            }
            _ => break existing,
        }
        if !existing.pop() {
            break existing;
        }
    };
    for name in tail.into_iter().rev() {
        if name == ".." {
            out.pop();
        } else {
            out.push(name);
        }
    }
    out
}

/// The one check every command that writes an output directory runs first.
///
/// Refuses (as [`Error::Config`], exit code 2) when `out` is, or is inside,
/// one of `inputs`, or when an input lives under `out/images` (which a run
/// clears), and when `out` exists, is not empty and holds none of `markers`:
/// a directory that is not a previous saccade output is never cleared or
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
        "the output directory {} is not empty and is not a previous saccade output (no {}); \
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
        // Clear stale explicit-question provenance with its measured report.
        "saccade-pipeline-choice.v1.json",
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

/// Copies `src` to `<report>/images/<name>.d/<stem>.<ext>`, returning the path
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
    let rel = format!("images/{name}.d/{stem}.{ext}");
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
            eprintln!("saccade: warning: {kind} glob {glob:?} matches no image in this run");
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
/// writes `<report_dir>/saccade-report.v1.json` plus per-image copies and
/// heatmaps, renders the HTML report and returns the [`Report`].
///
/// `report_dir` must not be inside either input and must be empty, absent or a
/// previous saccade report ([`guard_output_dir`]); anything else is refused
/// before a file is touched. Previous report files in it
/// (`saccade-report.v1.json`, `index.html`, `images/`) are removed first, so
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
    if let Some(guard) = APPROVAL_GUARD.get() {
        let expected = guard(baseline_dir)?;
        return run_approved(baseline_dir, capture_dir, report_dir, config, &expected);
    }
    run_inner(baseline_dir, capture_dir, report_dir, config, None)
}

/// Authentication callback installed by a trusted embedding process.
/// The default library behavior is unauthenticated. Once installed, every stock
/// measurement (including rank/ablation helpers) verifies and snapshots its baseline.
pub type ApprovalGuard = fn(&Path) -> Result<BTreeMap<String, String>>;
static APPROVAL_GUARD: std::sync::OnceLock<ApprovalGuard> = std::sync::OnceLock::new();

/// Installs the process-wide approval guard once; it cannot be disabled or replaced.
pub fn set_approval_guard(guard: ApprovalGuard) -> Result<()> {
    APPROVAL_GUARD
        .set(guard)
        .map_err(|_| Error::Config("approval guard already installed".into()))
}
#[cfg(feature = "graphics")]
pub(crate) fn guard_nonstock(baseline: &Path) -> Result<()> {
    if let Some(guard) = APPROVAL_GUARD.get() {
        guard(baseline)?;
        return Err(Error::ApprovalRefused {
            code: "approval_consumer_unsupported",
            message: "signed policy requires a snapshot-aware measurement engine".into(),
        });
    }
    Ok(())
}
fn run_inner(
    baseline_dir: &Path,
    capture_dir: &Path,
    report_dir: &Path,
    config: &RunConfig,
    source_baseline: Option<&Path>,
) -> Result<Report> {
    config.validate()?;
    for (present, missing) in [(baseline_dir, capture_dir), (capture_dir, baseline_dir)] {
        if present.is_file() && !missing.exists() {
            return Err(Error::Config(format!(
                "input does not exist: {}",
                missing.display()
            )));
        }
    }
    let arm_validation = if config.meta.require_valid_arms {
        let check = crate::arms::validate_paths(baseline_dir, capture_dir, config)?;
        if check.exit_code != 0 {
            return Err(Error::InvalidComparison(Box::new(check)));
        }
        Some(check)
    } else {
        None
    };
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
    std::fs::write(&sentinel, b"incomplete saccade run\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;
    let file_pair = baseline_dir.is_file() && capture_dir.is_file();
    if baseline_dir.is_file() != capture_dir.is_file() {
        return Err(Error::Config(
            "inputs must both be files or both be directories".into(),
        ));
    }
    fn input_root(p: &Path, file_pair: bool) -> &Path {
        if file_pair {
            p.parent().unwrap_or(Path::new("."))
        } else {
            p
        }
    }
    let (baselines, captures) = if file_pair {
        let name = capture_dir
            .file_name()
            .ok_or_else(|| Error::Config("capture file needs a name".into()))?
            .to_string_lossy()
            .into_owned();
        (
            Collected {
                files: BTreeMap::from([(name.clone(), baseline_dir.to_path_buf())]),
                ..Default::default()
            },
            Collected {
                files: BTreeMap::from([(name, capture_dir.to_path_buf())]),
                ..Default::default()
            },
        )
    } else {
        (collect_images(baseline_dir)?, collect_images(capture_dir)?)
    };

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
    if config.diagnostics.enabled
        && config.meta.required_keys.is_empty()
        && !config.meta.require_valid_arms
    {
        for k in &config.diagnostics.perf_keys {
            if !meta_options.ignore.contains(k)
                && !crate::meta::DEFAULT_IGNORE.contains(&k.as_str())
            {
                meta_options.ignore.push(k.clone());
            }
        }
    }
    let meta = meta_options.checker()?;
    let excluded_captures = names
        .keys()
        .filter(|name| {
            ignore.iter().any(|m| m.is_match(name))
                || !crate::paths::matches_entries(&config.entries, name)
        })
        .map(|name| (*name).to_string())
        .collect();
    let work: Vec<_> = names
        .into_iter()
        .filter(|(name, _)| !ignore.iter().any(|m| m.is_match(name)))
        .filter(|(name, _)| crate::paths::matches_entries(&config.entries, name))
        .collect();
    // Pairs are independent; an indexed parallel collect keeps the name order.
    let mut entries: Vec<Entry> = work
        .par_iter()
        .map(|&(name, (base, cap))| {
            let both_files = matches!((base, cap), (Some(Source::File(_)), Some(Source::File(_))));
            let mut entry = build_entry(name, base, cap, report_dir, &opts, config);
            if both_files {
                apply_meta(
                    &mut entry,
                    &meta,
                    config.mode,
                    input_root(baseline_dir, file_pair),
                    input_root(capture_dir, file_pair),
                    if file_pair { baseline_dir.file_name().and_then(|n| n.to_str()).unwrap_or(name) } else { name },
                );
                if file_pair && baseline_dir.file_name() != capture_dir.file_name() {
                    entry.warnings.push("per-image timing pairing is unavailable for differently named file inputs; use same-named directory entries for timing comparisons".into());
                } else {
                #[cfg(feature = "graphics")]
                apply_perf(
                    &mut entry,
                    &meta,
                    input_root(baseline_dir, file_pair),
                    input_root(capture_dir, file_pair),
                    config,
                );
                }
            }
            apply_image_warnings(&mut entry, config);
            if !config.record_absolute_paths {
                crate::paths::redact_entry(&mut entry, report_dir, &[baseline_dir, capture_dir]);
            }
            entry
        })
        .collect();
    #[cfg(feature = "graphics")]
    let (perf_diff, mut perf_errors) = crate::perf::pair(
        input_root(baseline_dir, file_pair),
        input_root(capture_dir, file_pair),
        &config.perf,
    )?;
    #[cfg(not(feature = "graphics"))]
    let (perf_diff, mut perf_errors): (
        Option<crate::perf::PerfDiff>,
        Vec<crate::perf::PerfError>,
    ) = (None, Vec::new());
    for error in &mut perf_errors {
        if !config.record_absolute_paths {
            error.path = crate::paths::record(Path::new(&error.path), report_dir, false);
        }
        let why = error.to_string();
        let mut entry = build_entry(
            &format!("{}: {}", config.perf.name, error.path),
            Some(Source::Problem(&why)),
            None,
            report_dir,
            &opts,
            config,
        );
        entry.error = Some(error.to_string());
        entries.push(entry);
    }
    let combined_verdict = perf_diff
        .as_ref()
        .map(|diff| crate::ablate::combined(&entries, diff));
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
    let mut report = Report {
        report_id: None,
        source_refs: Vec::new(),
        schema: REPORT_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        baseline_dir: Some(crate::paths::record(
            source_baseline.unwrap_or(baseline_dir),
            report_dir,
            config.record_absolute_paths,
        )),
        capture_dir: Some(crate::paths::record(
            capture_dir,
            report_dir,
            config.record_absolute_paths,
        )),
        config: ReportConfig {
            mask_mode: config.mask_mode,
            entries: config.entries.clone(),
            ignore: config.ignore.clone(),
            default_threshold: config.default_threshold,
            default_metric: config.default_metric,
            pixels_per_degree: config.pixels_per_degree,
            fail_on_new: config.fail_on_new || config.mode == Mode::Identity,
            mode: config.mode,
            labels: config.labels.clone(),
            meta: {
                let mut settings = meta.settings();
                settings.arm_validation = arm_validation;
                settings
            },
            allow_empty: false,
            fail_on_nonfinite: config.fail_on_nonfinite || config.mode == Mode::Identity,
            hotspot_fail: config.hotspot_fail,
            hotspot_local_max: config.hotspot_local_max,
            hotspot_local_min_pixels: config.hotspot_local_min_pixels,
        },
        totals,
        entries,
        perf_diff,
        perf_errors,
        combined_verdict,
        exclusion_audit: None,
    };
    report.exclusion_audit = Some(crate::exclusions::audit(&report, excluded_captures));
    if report.is_empty_run() {
        let why = if baselines.files.is_empty() && report.totals.new > 0 {
            format!(
                "the baseline directory {} has no images, so every capture is new",
                baseline_dir.display()
            )
        } else {
            "no image exists in both directories".to_string()
        };
        eprintln!("saccade: warning: nothing compared: {why}; empty comparisons are not evidence");
    }

    let json_path = report_dir.join(REPORT_FILE_NAME);
    report = serde_json::from_value(crate::report_links::decorate(&serde_json::to_value(
        &report,
    )?)?)?;
    crate::report_links::write(&json_path, &report)?;
    render::render_html(&report, report_dir)?;
    std::fs::write(&sentinel, b"complete saccade run\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;
    Ok(report)
}

/// Measures a private snapshot whose exact copied bytes match an authenticated
/// inventory. The caller verifies the signature and canonical destination first.
/// Metadata and ignored files are bound too; no original baseline is read by the
/// measurement after snapshot creation. Source references retain the real baseline.
pub fn run_approved(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    config: &RunConfig,
    expected: &BTreeMap<String, String>,
) -> Result<Report> {
    use sha2::{Digest as _, Sha256};
    use std::io::Write;
    fn snapshot(
        root: &Path,
        dir: &Path,
        target: &Path,
        depth: usize,
        files: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        if depth > 64 {
            return Err(Error::ApprovalContentMismatch);
        }
        for entry in std::fs::read_dir(dir).map_err(|_| Error::ApprovalContentMismatch)? {
            let entry = entry.map_err(|_| Error::ApprovalContentMismatch)?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|_| Error::ApprovalContentMismatch)?;
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::ApprovalContentMismatch)?;
            if kind.is_symlink() {
                return Err(Error::ApprovalContentMismatch);
            }
            if relative == Path::new(".saccade-approval.json") && kind.is_file() {
                continue;
            }
            let destination = target.join(relative);
            if kind.is_dir() {
                std::fs::create_dir(&destination).map_err(|_| Error::ApprovalContentMismatch)?;
                snapshot(root, &path, target, depth + 1, files)?;
            } else if kind.is_file() {
                if files.len() >= 10000 {
                    return Err(Error::ApprovalContentMismatch);
                }
                // Stream once into the snapshot and hash precisely the bytes written.
                let mut options = std::fs::OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                }
                let mut source = options
                    .open(&path)
                    .map_err(|_| Error::ApprovalContentMismatch)?;
                if !source
                    .metadata()
                    .map_err(|_| Error::ApprovalContentMismatch)?
                    .is_file()
                {
                    return Err(Error::ApprovalContentMismatch);
                }
                let mut dest = std::fs::File::create(destination)
                    .map_err(|_| Error::ApprovalContentMismatch)?;
                let mut hash = Sha256::new();
                let mut buffer = [0u8; 65536];
                let mut size = 0u64;
                loop {
                    let n = std::io::Read::read(&mut source, &mut buffer)
                        .map_err(|_| Error::ApprovalContentMismatch)?;
                    if n == 0 {
                        break;
                    }
                    size += n as u64;
                    if size > 1024 * 1024 * 1024 {
                        return Err(Error::ApprovalContentMismatch);
                    }
                    dest.write_all(&buffer[..n])
                        .map_err(|_| Error::ApprovalContentMismatch)?;
                    hash.update(&buffer[..n]);
                }
                let name = relative
                    .components()
                    .map(|c| c.as_os_str().to_str().ok_or(Error::ApprovalContentMismatch))
                    .collect::<Result<Vec<_>>>()?
                    .join("/");
                files.insert(name, format!("sha256:{:x}", hash.finalize()));
            } else {
                return Err(Error::ApprovalContentMismatch);
            }
        }
        Ok(())
    }
    guard_output_dir(out, &[baseline, capture], &[REPORT_FILE_NAME, RUN_SENTINEL])?;
    let temp = tempfile::tempdir().map_err(io_err("creating verified baseline snapshot".into()))?;
    let mut actual = BTreeMap::new();
    snapshot(baseline, baseline, temp.path(), 0, &mut actual)?;
    if &actual != expected {
        return Err(Error::ApprovalContentMismatch);
    }
    run_inner(temp.path(), capture, out, config, Some(baseline))
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
        let reported_as_error = config.mode == Mode::Identity || config.fail_on_nonfinite;
        if (p.nan_count > 0 || p.inf_count > 0) && !reported_as_error {
            entry.warnings.push(format!(
                "{side} has non-finite samples ({} NaN, {} infinite)",
                p.nan_count, p.inf_count
            ));
        }
    }
    for (side, props) in sides {
        if let Some(p) = props
            && (p.nan_count > 0 || p.inf_count > 0)
        {
            let msg = format!(
                "{side} has non-finite samples ({} NaN, {} infinite)",
                p.nan_count, p.inf_count
            );
            entry.capture_validity.status = crate::meta::Validity::Invalid;
            entry.capture_validity.reasons.push(msg.clone());
            if config.fail_on_nonfinite || config.mode == Mode::Identity {
                entry.status = Status::Error;
                entry.error = Some(match entry.error.take() {
                    Some(prev) => format!("{prev}; {msg}"),
                    None => msg,
                });
            }
        }
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
    mode: crate::report::Mode,
    baseline_dir: &Path,
    capture_dir: &Path,
    baseline_name: &str,
) {
    let baseline = crate::meta::capture_evidence(baseline_dir, baseline_name, meta.name());
    let capture = crate::meta::capture_evidence(capture_dir, &entry.name, meta.name());
    for (side, evidence) in [("baseline", &baseline), ("capture", &capture)] {
        for field in ["binary_sha256", "source_head"] {
            if let Some(value) = evidence.get(field) {
                entry
                    .capture_provenance
                    .insert(format!("{side}.{field}"), value.clone());
            } else {
                let reason = format!(
                    "{side} {field} provenance is absent; supply {field} in --meta-name {name} (cost-card.json with binary.sha and build.commit)",
                    name = meta.name()
                );
                entry.warnings.push(reason.clone());
                entry.capture_validity.reasons.push(reason);
            }
        }
        for field in ["capture_id", "content_hash", "timestamp"] {
            if let Some(value) = evidence.get(field) {
                entry
                    .capture_provenance
                    .insert(format!("{side}.{field}"), value.clone());
            }
        }
    }
    let repeated = crate::meta::same_capture(&baseline, &capture);
    let provenance_reasons: Vec<_> = entry
        .capture_validity
        .reasons
        .iter()
        .filter(|reason| reason.as_str() != "capture context was not checked")
        .cloned()
        .collect();
    let failure = match meta.check_named(baseline_dir, baseline_name, capture_dir, &entry.name) {
        Ok(checked) => {
            entry.intended_variables = checked.intended;
            entry.covered_by_derivation = checked.covered_by_derivation;
            entry.meta_diff = checked.diff;
            entry.meta_ignored_diff = checked.ignored;
            entry.meta_declared_unchanged = checked.unchanged;
            if entry.capture_validity.status != crate::meta::Validity::Invalid {
                entry.capture_validity = checked.validity;
            } else {
                entry
                    .capture_validity
                    .reasons
                    .extend(checked.validity.reasons);
            }
            entry.capture_validity.reasons.extend(provenance_reasons);
            if ["binary_sha256", "source_head"]
                .into_iter()
                .any(|field| !baseline.contains_key(field) || !capture.contains_key(field))
                && entry.capture_validity.status == crate::meta::Validity::Valid
            {
                entry.capture_validity.status = crate::meta::Validity::Unknown;
            }
            checked.failure
        }
        Err(e) => Some(e),
    };
    if repeated {
        let reason = "same capture, not a repeat".to_string();
        entry.warnings.push(reason.clone());
        entry.capture_validity.reasons.push(reason.clone());
        if entry.capture_validity.status == crate::meta::Validity::Valid {
            entry.capture_validity.status = crate::meta::Validity::Unknown;
        }
        if mode == crate::report::Mode::Identity {
            entry.capture_validity.status = crate::meta::Validity::Invalid;
            entry.status = Status::Error;
            entry.error = Some(reason);
        }
    }
    if let Some(msg) = failure {
        entry.capture_validity.status = crate::meta::Validity::Invalid;
        if !entry.capture_validity.reasons.contains(&msg) {
            entry.capture_validity.reasons.push(msg.clone());
        }
        entry.status = Status::Error;
        entry.error = Some(match entry.error.take() {
            Some(prev) => format!("{prev}; {msg}"),
            None => msg,
        });
    }
}

/// Pairs the timing keys of both sidecars next to the image verdict.
#[cfg(feature = "graphics")]
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
    (d.perf, d.perf_not_comparable) = crate::diagnostics::perf_pairs(
        meta,
        &config.diagnostics,
        baseline_dir,
        capture_dir,
        &entry.name,
    );
}

/// Both sides of a pair as the diagnostics engine reads them.
struct PairPixels<'a> {
    baseline_path: &'a Path,
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
    let image_dir = format!("{}.d", entry.name);
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
            name: &image_dir,
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
        intended_variables: Vec::new(),
        covered_by_derivation: Vec::new(),
        spatial: None,
        gallery: Vec::new(),
        field_evidence: None,
        layers: None,
        required_effects: Vec::new(),
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
        pixel_exclusions: None,
        sample_exclusions: None,
        bit_identical: None,
        file_bytes_identical: None,
        capture_validity: Default::default(),
        capture_provenance: Default::default(),
        meta_declared_unchanged: Vec::new(),
        hdr: None,
        meta_diff: Vec::new(),
        meta_ignored_diff: Vec::new(),
        baseline_properties: None,
        warnings: Vec::new(),
        baseline_sha256: None,
        capture_sha256: None,
        hotspots: Vec::new(),
        changed_pixel_runs: Vec::new(),
        object_attribution: Vec::new(),
        pass_with_local_change: false,
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
        entry.object_attribution.clear();
        entry.diagnostics = None;
        entry.paths.signed_diff = None;
        entry.paths.nonfinite_mask = None;
        entry.buffer = None;
        entry.error = Some(e.to_string());
    }
    entry.file_bytes_identical = entry
        .baseline_sha256
        .as_ref()
        .zip(entry.capture_sha256.as_ref())
        .map(|(b, c)| b == c);
    if matches!(entry.status, Status::Missing | Status::New | Status::Error) {
        entry.capture_validity = crate::meta::CaptureValidity {
            status: crate::meta::Validity::Invalid,
            reasons: vec![
                entry
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("incomplete pairing: {:?}", entry.status)),
            ],
        };
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
    if config.mode == Mode::Identity
        && let (Some(Source::File(b)), Some(Source::File(c))) = (base, cap)
    {
        return fill_identity_pair(entry, b, c, report_dir, opts, config);
    }
    if let Some(spec) = config.buffer_for(&name) {
        if config.spatial.is_some() || !config.required_effect.is_empty() {
            return Err(Error::Config("spatial FLIP/effect-difference policies require a colour-image comparison; native buffers use capture_layers scope".into()));
        }
        entry.metric_used = spec.metric;
        entry.threshold = spec.threshold();
        match (base, cap) {
            (Some(Source::File(b)), Some(Source::File(c))) => {
                let mut spec = spec.clone();
                if spec.capture_layers.is_none() {
                    spec.capture_layers = config.layers.clone();
                }
                crate::buffer::fill_pair(entry, b, c, report_dir, &spec, &config.field)?;
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

    let mut losses = Vec::new();
    let cap_img = match decode_audited(cap, false, "capture", &mut losses) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.properties = Some(properties::validate(&flatten_over(&cap_img, 0)));
    let base_img = match decode_audited(base, false, "baseline", &mut losses) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.sample_exclusions = Some(losses);
    entry.baseline_properties = Some(properties::validate(&flatten_over(&base_img, 0)));
    entry.bit_identical = Some(crate::compare::native_samples_identical(base, cap));
    let local = layered_config(
        entry,
        base,
        cap,
        &image::DynamicImage::ImageRgba8(base_img.clone()),
        &image::DynamicImage::ImageRgba8(cap_img.clone()),
        config,
        report_dir,
    )?;
    let config = local.as_ref();
    let comparison = if config.mask_mode == crate::compare::MaskMode::Neutralize
        && cap_img.dimensions() == base_img.dimensions()
    {
        let mask =
            crate::regions::effective_mask(&entry.name, cap_img.width(), cap_img.height(), config)?;
        if mask.as_ref().is_some_and(|m| m.iter().all(|v| *v)) {
            return Err(Error::Config("every pixel is masked".into()));
        }
        crate::compare::compare_rgba_masked(
            &cap_img,
            &base_img,
            opts,
            mask.as_deref(),
            config.mask_mode,
        )
    } else {
        compare_rgba(&cap_img, &base_img, opts)
    };
    match comparison {
        Ok(cmp) => {
            let pair = PairPixels {
                baseline_path: base,
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
    // Keep original excluded error and the no-mask diagnostic independent of
    // neutralized scoring. Raw pixels remain authoritative for this audit.
    let raw_comparison =
        if config.mask_mode == crate::compare::MaskMode::Neutralize && scene.mask.is_some() {
            Some(match (&pair.capture, &pair.baseline) {
                (crate::diagnostics::Pixels::Ldr(c), crate::diagnostics::Pixels::Ldr(b)) => {
                    compare_rgba(c, b, pair.flip)?
                }
                (crate::diagnostics::Pixels::Hdr(c), crate::diagnostics::Pixels::Hdr(b)) => {
                    crate::hdr::compare_hdr(c, b, pair.flip)?.0
                }
                _ => return Err(Error::Config("comparison pixel kinds differ".into())),
            })
        } else {
            None
        };
    entry.pixel_exclusions = Some(crate::exclusions::pixels(
        raw_comparison.as_ref().unwrap_or(&cmp),
        scene.mask.as_deref(),
        entry,
        config,
    )?);
    let value = metric_value(&scene.metrics, entry.metric_used);
    let rel = format!("images/{name}.d/heatmap.png");
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
    entry.changed_pixel_runs = crate::hotspots::threshold_runs(
        &cmp.error_map,
        scene.mask.as_deref(),
        config.hotspot_threshold,
    );
    entry.object_attribution = crate::object_ids::attribute(
        crate::object_ids::Sidecars {
            capture: pair.capture_path,
            report_dir,
            entry_name: &entry.name,
        },
        crate::object_ids::HotspotPixels {
            errors: &cmp.error_map,
            mask: scene.mask.as_deref(),
            width: cmp.metrics.width,
            height: cmp.metrics.height,
            hotspots: &entry.hotspots,
            cutoff: config.hotspot_threshold,
        },
    )?;
    entry.status = if config.mode == Mode::Identity {
        if entry.bit_identical == Some(true) {
            Status::Pass
        } else {
            Status::Fail
        }
    } else {
        crate::regions::combine(status_of(value, entry.threshold), &entry.regions)
    };
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
    entry.pass_with_local_change = entry.status == Status::Pass
        && entry.bit_identical == Some(false)
        && crate::hotspots::has_severe_component(
            &cmp.error_map,
            scene.mask.as_deref(),
            cmp.metrics.width,
            cmp.metrics.height,
            config.hotspot_local_max,
            config.hotspot_local_min_pixels,
        );
    entry.value = Some(value);
    entry.metrics = Some(scene.metrics);
    entry.paths.heatmap = Some(rel);
    run_diagnostics(entry, &cmp, pair, report_dir, config);
    apply_quality(entry, &cmp, pair, report_dir, config)?;
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
        let rel = format!("images/{name}.d/{stem}.png");
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
    let rel = format!("images/{name}.d/{stem}.png");
    let dest = report_dir.join(&rel);
    crate::hdr::display_image(&img, config.hdr.tonemapper)
        .save(&dest)
        .map_err(|source| Error::Encode { path: dest, source })?;
    Ok(Some(rel))
}

/// Identity decodes native samples before any display conversion or metric.
fn fill_identity_pair(
    entry: &mut Entry,
    base: &Path,
    cap: &Path,
    report_dir: &Path,
    opts: &CompareOptions,
    config: &RunConfig,
) -> Result<()> {
    let open = |p: &Path| {
        image::open(p).map_err(|source| Error::Decode {
            path: p.to_path_buf(),
            source,
        })
    };
    let b = open(base)?;
    let c = open(cap)?;
    let local = layered_config(entry, base, cap, &b, &c, config, report_dir)?;
    let config = local.as_ref();
    entry.bit_identical = Some(crate::compare::native_images_identical(&b, &c));
    let props = |i: &image::DynamicImage| {
        let mut p = properties::validate(&flatten_over(&i.to_rgba8(), 0));
        (p.nan_count, p.inf_count) = crate::compare::nonfinite_samples(i);
        p
    };
    entry.baseline_properties = Some(props(&b));
    entry.properties = Some(props(&c));
    entry.status = if entry.bit_identical == Some(true) {
        Status::Pass
    } else {
        Status::Fail
    };
    if b.width() == 0 || b.height() == 0 || c.width() == 0 || c.height() == 0 {
        return Err(Error::EmptyImage);
    }
    if b.width() != c.width() || b.height() != c.height() {
        entry.capture_validity = crate::meta::CaptureValidity {
            status: crate::meta::Validity::Invalid,
            reasons: vec!["native dimensions differ".into()],
        };
        return Ok(());
    }
    if [entry.properties, entry.baseline_properties]
        .into_iter()
        .flatten()
        .any(|p| p.nan_count > 0 || p.inf_count > 0)
    {
        // The equality finding survives; invalid samples never enter FLIP.
        return Ok(());
    }
    if crate::hdr::is_hdr_path(base) && crate::hdr::is_hdr_path(cap) {
        let b = crate::hdr::decode_hdr(base)?;
        let c = crate::hdr::decode_hdr(cap)?;
        let (cmp, info) = crate::hdr::compare_hdr(&c, &b, opts)?;
        entry.hdr = Some(info);
        let pair = PairPixels {
            baseline_path: base,
            baseline: crate::diagnostics::Pixels::Hdr(&b),
            capture: crate::diagnostics::Pixels::Hdr(&c),
            capture_path: cap,
            flip: opts,
        };
        finish_entry(entry, cmp, report_dir, config, &pair)
    } else {
        let (b, c) = (b.to_rgba8(), c.to_rgba8());
        let cmp = compare_rgba(&c, &b, opts)?;
        let pair = PairPixels {
            baseline_path: base,
            baseline: crate::diagnostics::Pixels::Ldr(&b),
            capture: crate::diagnostics::Pixels::Ldr(&c),
            capture_path: cap,
            flip: opts,
        };
        finish_entry(entry, cmp, report_dir, config, &pair)
    }
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
    let mut losses = Vec::new();
    let cap_img = match decode_audited(cap, true, "capture", &mut losses) {
        Ok(i) => crate::hdr::from_decoded(i),
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.properties = Some(crate::hdr::validate_hdr(&cap_img));
    let base_img = match decode_audited(base, true, "baseline", &mut losses) {
        Ok(i) => crate::hdr::from_decoded(i),
        Err(e) => {
            entry.error = Some(e.to_string());
            return Ok(());
        }
    };
    entry.sample_exclusions = Some(losses);
    entry.baseline_properties = Some(crate::hdr::validate_hdr(&base_img));
    entry.bit_identical = Some(crate::compare::native_samples_identical(base, cap));
    let local = layered_config(
        entry,
        base,
        cap,
        &crate::evidence_quality::image(base)?,
        &crate::evidence_quality::image(cap)?,
        config,
        report_dir,
    )?;
    let config = local.as_ref();
    let mut filtered_capture = cap_img.clone();
    if config.mask_mode == crate::compare::MaskMode::Neutralize
        && cap_img.width == base_img.width
        && cap_img.height == base_img.height
        && let Some(mask) =
            crate::regions::effective_mask(&entry.name, cap_img.width, cap_img.height, config)?
    {
        for ((test, reference), excluded) in filtered_capture
            .data
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(base_img.data.as_chunks::<3>().0.iter())
            .zip(mask)
        {
            if excluded {
                test.copy_from_slice(reference);
            }
        }
    }
    match crate::hdr::compare_hdr(&filtered_capture, &base_img, opts) {
        Ok((cmp, info)) => {
            entry.hdr = Some(info);
            let pair = PairPixels {
                baseline_path: base,
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

fn apply_quality(
    entry: &mut Entry,
    cmp: &crate::compare::Comparison,
    pair: &PairPixels<'_>,
    report_dir: &Path,
    config: &RunConfig,
) -> Result<()> {
    let base_display;
    let cap_display;
    let base_img = match &pair.baseline {
        crate::diagnostics::Pixels::Ldr(i) => *i,
        crate::diagnostics::Pixels::Hdr(i) => {
            base_display =
                image::DynamicImage::ImageRgb8(crate::hdr::display_image(i, config.hdr.tonemapper))
                    .to_rgba8();
            &base_display
        }
    };
    let cap_img = match &pair.capture {
        crate::diagnostics::Pixels::Ldr(i) => *i,
        crate::diagnostics::Pixels::Hdr(i) => {
            cap_display =
                image::DynamicImage::ImageRgb8(crate::hdr::display_image(i, config.hdr.tonemapper))
                    .to_rgba8();
            &cap_display
        }
    };
    let errors = &cmp.error_map;
    if let Some(policy) = &config.spatial {
        use crate::evidence_quality::spatial;
        let excluded = crate::regions::effective_mask(
            &entry.name,
            base_img.width(),
            base_img.height(),
            config,
        )?;
        let root = config.config_dir.as_deref().unwrap_or(Path::new("."));
        let gaps = policy
            .background
            .as_ref()
            .map(|criterion| {
                Ok::<_, Error>((
                    spatial::background(criterion, base_img, pair.baseline_path, root)?,
                    spatial::background(criterion, cap_img, pair.capture_path, root)?,
                ))
            })
            .transpose()?;
        let fallback = entry
            .diagnostics
            .as_ref()
            .map_or(crate::diagnostics::ChangeClass::LocalStructure, |d| d.class);
        let result = spatial::analyze(
            base_img,
            cap_img,
            errors,
            excluded.as_deref(),
            gaps.as_ref().map(|(b, c)| (b.as_slice(), c.as_slice())),
            policy,
            pair.flip,
            fallback,
        )?;
        if policy.decide && config.mode != Mode::Identity {
            entry.status = if matches!(
                result.class,
                crate::diagnostics::ChangeClass::Identical
                    | crate::diagnostics::ChangeClass::TextureNoiseOnly
            ) && entry.regions.iter().all(|r| r.status != Some(Status::Fail))
            {
                Status::Pass
            } else {
                Status::Fail
            };
        }
        if let Some(d) = &mut entry.diagnostics {
            d.class = result.class;
            d.description = format!(
                "{} under {}; {} systematic regions",
                result.class.as_str(),
                policy.version,
                result.regions.len()
            );
        }
        entry.gallery = crate::evidence_quality::gallery::generate(
            &entry.name,
            base_img,
            cap_img,
            errors,
            excluded.as_deref(),
            &entry.hotspots,
            &result,
            report_dir,
        )?;
        entry.spatial = Some(result);
    }
    for effect in &config.required_effect {
        if !crate::config::compile_glob(&effect.glob)?.is_match(&entry.name) {
            continue;
        }
        let root = config
            .effect_roots
            .get(&effect.name)
            .map(|p| p.as_path())
            .or(config.config_dir.as_deref())
            .unwrap_or(Path::new("."));
        let b = crate::evidence_quality::effect::select(
            &effect.selection,
            pair.baseline_path,
            root,
            base_img.dimensions(),
        )?;
        let c = crate::evidence_quality::effect::select(
            &effect.selection,
            pair.capture_path,
            root,
            cap_img.dimensions(),
        )?;
        let result =
            crate::evidence_quality::effect::measure(effect, &b, &c, base_img, cap_img, errors)?;
        if !result.failures.is_empty() {
            entry.status = Status::Fail;
        }
        entry.required_effects.push(result);
    }
    use crate::evidence_quality::{field, maps, repeat_noise};
    let excluded =
        crate::regions::effective_mask(&entry.name, base_img.width(), base_img.height(), config)?;
    let mut ids = config.field_ids.clone();
    if ids.is_none()
        && let Some(source) = entry.object_attribution.first()
    {
        let image = crate::evidence_quality::image(&report_dir.join(&source.image_path))?;
        let values = field::ids(&image)?;
        let layer = field::IdLayer {
            values,
            names: Default::default(),
            provenance: format!(
                "candidate {} ID buffer, sha256 {}; candidate footprint used on both arms",
                source.kind, source.image_sha256
            ),
        };
        ids = Some((layer.clone(), layer));
    }
    let masked = excluded.as_ref().is_some_and(|m| m.iter().any(|v| *v));
    let scope = field::scope(ids.is_some(), masked);
    if config.field.require_scope && scope == "whole_frame_unmasked" {
        return Err(Error::Config(
            "render evidence requires scope: supply an ID layer, mask or generic instance dump"
                .into(),
        ));
    }
    let mut result = field::Report {
        class: entry
            .spatial
            .as_ref()
            .map_or_else(
                || {
                    entry
                        .diagnostics
                        .as_ref()
                        .map_or("unclassified", |d| d.class.as_str())
                },
                |s| s.class.as_str(),
            )
            .into(),
        scope: scope.into(),
        per_id: Vec::new(),
        ids_measured: 0,
        provenance: Vec::new(),
        maps: None,
        noise: None,
    };
    if let Some((b, c)) = &ids {
        let lb = crate::evidence_quality::spatial::rgb(base_img);
        let lc = crate::evidence_quality::spatial::rgb(cap_img);
        let signed: Vec<_> = lb
            .iter()
            .zip(lc)
            .map(|(b, c)| {
                crate::evidence_quality::spatial::luminance(c)
                    - crate::evidence_quality::spatial::luminance(*b)
            })
            .collect();
        let (rows, count, _) = field::attribute(
            b,
            c,
            base_img.dimensions(),
            &signed,
            None,
            excluded.as_deref(),
            "normalized_srgb_luminance",
            config.field.threshold,
            config.field.top,
        )?;
        result.per_id = rows;
        result.ids_measured = count;
        result.provenance = vec![b.provenance.clone(), c.provenance.clone()];
        field::crops(
            &mut result.per_id,
            base_img,
            cap_img,
            errors,
            report_dir,
            &entry.name,
        )?;
    }
    if config.field.export_maps {
        result.maps = Some(maps::write(
            &maps::collect(
                errors,
                base_img.dimensions(),
                excluded.as_deref(),
                entry.spatial.as_ref(),
            )?,
            report_dir,
            &entry.name,
        )?);
    }
    if !config.field.noise_from.is_empty() {
        if entry.hdr.is_some() {
            return Err(Error::Config("repeat noise currently requires SDR captures; HDR noise must retain native radiance".into()));
        }
        let paths = repeat_noise::paths(&config.field.noise_from, &entry.name)?;
        for path in &paths {
            crate::arms::enforce(pair.baseline_path, path, config)?;
        }
        let floor = repeat_noise::build(
            &paths,
            config.spatial.as_ref().map_or(32, |s| s.tile_size),
            excluded.as_deref(),
        )?;
        let decision = repeat_noise::decide(base_img, cap_img, floor, excluded.as_deref())?;
        result.class = decision.class.clone();
        if config.mode != Mode::Identity && config.spatial.as_ref().is_some_and(|p| p.decide) {
            entry.status = if decision.class == "texture_noise_only"
                && !config
                    .hotspot_fail
                    .is_some_and(|limit| entry.metrics.as_ref().is_some_and(|m| m.max >= limit))
                && entry.required_effects.iter().all(|e| e.failures.is_empty())
                && entry.regions.iter().all(|r| r.status != Some(Status::Fail))
            {
                Status::Pass
            } else {
                Status::Fail
            };
            if let Some(s) = &mut entry.spatial {
                s.class = if decision.class == "texture_noise_only" {
                    crate::diagnostics::ChangeClass::TextureNoiseOnly
                } else {
                    crate::diagnostics::ChangeClass::SystematicShift
                };
            }
            if let Some(d) = &mut entry.diagnostics {
                d.class = if decision.class == "texture_noise_only" {
                    crate::diagnostics::ChangeClass::TextureNoiseOnly
                } else {
                    crate::diagnostics::ChangeClass::SystematicShift
                };
            }
        }
        result.noise = Some(decision);
    }
    entry.field_evidence = Some(result);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn layered_config<'a>(
    entry: &mut Entry,
    base: &Path,
    cap: &Path,
    b: &image::DynamicImage,
    c: &image::DynamicImage,
    config: &'a RunConfig,
    report_dir: &Path,
) -> Result<std::borrow::Cow<'a, RunConfig>> {
    let Some(policy) = &config.layers else {
        return Ok(std::borrow::Cow::Borrowed(config));
    };
    let mut bl = crate::evidence_quality::layers::load_for_image(base, policy, b)?;
    let mut cl = crate::evidence_quality::layers::load_for_image(cap, policy, c)?;
    crate::evidence_quality::layers::bundle(&mut bl, report_dir, &entry.name, "baseline")?;
    crate::evidence_quality::layers::bundle(&mut cl, report_dir, &entry.name, "candidate")?;
    let (selected, nb, nc) = crate::evidence_quality::layers::scope_pair(&bl, &cl, policy)?;
    entry.layers = Some(crate::evidence_quality::layers::evidence(
        &bl, &cl, policy, &selected, nb, nc, b, c,
    )?);
    let mut local = config.clone();
    local.field_ids = match (
        crate::evidence_quality::layers::id_layer(&bl)?,
        crate::evidence_quality::layers::id_layer(&cl)?,
    ) {
        (Some(b), Some(c)) => Some((b, c)),
        (None, None) => None,
        _ => {
            return Err(Error::Config(
                "ID layer missing on one comparison arm".into(),
            ));
        }
    };
    local.layer_mask = Some(selected.into_iter().map(|v| !v).collect());
    Ok(std::borrow::Cow::Owned(local))
}
