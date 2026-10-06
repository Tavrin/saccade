//! Numbered image sequences and added temporal instability.

use std::path::Path;
#[cfg(any(feature = "graphics", feature = "prechecks"))]
use std::path::PathBuf;
#[cfg(feature = "graphics")]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[cfg(feature = "graphics")]
use crate::compare::CompareOptions;
use crate::config::RunConfig;
#[cfg(any(feature = "graphics", feature = "prechecks"))]
use crate::config::compile_glob;
use crate::error::{Error, Result};
use crate::report::{Entry, Totals};
#[cfg(feature = "graphics")]
use crate::report::{REPORT_FILE_NAME, REPORT_SCHEMA, Report, ReportConfig, Status};
#[cfg(feature = "graphics")]
use crate::run::{Source, io_err};

/// Sequence JSON schema identifier.
pub const SEQUENCE_SCHEMA: &str = "saccade-sequence.v1";
/// Sequence report filename.
pub const SEQUENCE_FILE: &str = "saccade-sequence.v1.json";

/// One sorted-index pair, including its actual source names and numbers.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceFrame {
    /// Zero-based sorted index, independent of either sequence's numbering origin.
    pub index: usize,
    /// Baseline relative name, if present.
    pub baseline_name: Option<String>,
    /// Capture relative name, if present.
    pub capture_name: Option<String>,
    /// Baseline trailing integer, if present.
    pub baseline_number: Option<u64>,
    /// Capture trailing integer, if present.
    pub capture_number: Option<u64>,
    /// Normal comparison entry; images are keyed by its safe synthetic name.
    pub entry: Entry,
}

/// Worst paired frame by mean FLIP.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorstFrame {
    /// Zero-based sorted index.
    pub index: usize,
    /// Synthetic entry name in the per-frame report.
    pub name: String,
    /// Mean FLIP against the baseline.
    pub mean_flip: f64,
}

/// Full sequence report on disk; lean outputs omit `frames` and `mean_flip_curve`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequenceReport {
    /// Fixed-camera per-tile flicker and global motion qualification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_stability: Option<crate::evidence_quality::temporal::Report>,
    /// Always `saccade-sequence.v1`.
    pub schema: String,
    /// `pass` or `regression`, with the same rules as compare plus temporal errors.
    pub verdict: String,
    /// Effective relative-name glob.
    pub pattern: String,
    /// Number of baseline frames.
    pub baseline_frames: usize,
    /// Number of capture frames.
    pub capture_frames: usize,
    /// Status counts of the indexed pairs.
    pub totals: Totals,
    /// Per-index mean FLIP, null for an unmeasured pair.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mean_flip_curve: Vec<Option<f64>>,
    /// Mean FLIP between consecutive baseline frames; null with fewer than two or errors.
    pub baseline_temporal_mean: Option<f64>,
    /// Mean FLIP between consecutive capture frames; null with fewer than two or errors.
    pub capture_temporal_mean: Option<f64>,
    /// Capture temporal mean minus baseline temporal mean; signed, not clamped.
    pub temporal_instability: Option<f64>,
    /// Reasons a temporal metric could not be measured. These fail the run.
    pub temporal_errors: Vec<String>,
    /// Worst paired frame by mean FLIP, earliest index wins a tie.
    pub worst_frame: Option<WorstFrame>,
    /// Paired entries whose deciding value exceeds their effective threshold.
    pub frames_over_threshold: usize,
    /// JSON report path relative to the working directory (absolute by opt-in).
    pub report_json: String,
    /// Per-frame report path relative to the working directory (absolute by opt-in).
    pub frames_report_json: String,
    /// HTML report path relative to the working directory (absolute by opt-in).
    pub index_html: String,
    /// Full comparison scope, including ignored and pattern-excluded source frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusion_audit:
        Option<crate::evidence::analysis::Analysis<crate::exclusions::ExclusionAudit>>,
    /// Per-frame details on disk; omitted from lean output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<SequenceFrame>,
}

impl SequenceReport {
    /// Whether the sequence exits with compare's regression code (1).
    pub fn is_regression(&self) -> bool {
        self.verdict == "regression"
    }

    /// Bounded machine output; the complete curve and entries remain on disk.
    pub fn lean(&self) -> Self {
        let mut out = self.clone();
        out.frames.clear();
        out.mean_flip_curve.clear();
        out
    }

    /// Human-readable sequence metrics.
    pub fn text(&self) -> String {
        let number = |v: Option<f64>| v.map_or_else(|| "unavailable".into(), |v| format!("{v:.6}"));
        format!(
            "sequence: {} ({} baseline, {} capture frames)\ntemporal instability: {} (capture {} - baseline {})\nframes over threshold: {}\nworst frame: {}\nreport: {}\n",
            self.verdict,
            self.baseline_frames,
            self.capture_frames,
            number(self.temporal_instability),
            number(self.capture_temporal_mean),
            number(self.baseline_temporal_mean),
            self.frames_over_threshold,
            self.worst_frame.as_ref().map_or_else(
                || "none".into(),
                |f| format!("{} (mean FLIP {:.6})", f.index, f.mean_flip)
            ),
            self.index_html
        )
    }
}

#[cfg(any(feature = "graphics", feature = "prechecks"))]
struct Frame {
    name: String,
    number: u64,
    path: Option<PathBuf>,
    problem: Option<String>,
}

#[cfg(feature = "graphics")]
impl Frame {
    fn source(&self) -> Option<Source<'_>> {
        match (&self.path, &self.problem) {
            (Some(p), _) => Some(Source::File(p)),
            (_, Some(why)) => Some(Source::Problem(why)),
            _ => None,
        }
    }
}

#[cfg(any(feature = "graphics", feature = "prechecks"))]
fn collect(
    root: &Path,
    pattern: &str,
    cfg: &RunConfig,
    excluded: &mut std::collections::BTreeSet<String>,
) -> Result<Vec<Frame>> {
    let matcher = compile_glob(pattern)?;
    let ignores = cfg
        .ignore
        .iter()
        .map(|g| compile_glob(g))
        .collect::<Result<Vec<_>>>()?;
    let files = crate::run::collect_images(root)?;
    let mut frames = Vec::new();
    for (name, path, problem) in files
        .files
        .into_iter()
        .map(|(n, p)| (n, Some(p), None))
        .chain(files.problems.into_iter().map(|(n, p)| (n, None, Some(p))))
    {
        if !matcher.is_match(&name) || ignores.iter().any(|g| g.is_match(&name)) {
            excluded.insert(name);
            continue;
        }
        if cfg.buffer_for(&name).is_some() {
            return Err(Error::Config(
                "sequence metrics require colour frames; a [[buffer]] rule matches a frame".into(),
            ));
        }
        let stem = Path::new(&name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let digits: String = stem
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let number = digits.parse::<u64>().map_err(|_| {
            Error::Config(format!(
                "sequence frame {name:?} must end in an integer before its extension"
            ))
        })?;
        frames.push(Frame {
            name,
            number,
            path,
            problem,
        });
    }
    frames.sort_by(|a, b| a.number.cmp(&b.number).then(a.name.cmp(&b.name)));
    if let Some(pair) = frames.windows(2).find(|p| p[0].number == p[1].number) {
        return Err(Error::Config(format!(
            "duplicate sequence frame number {}: {:?} and {:?}",
            pair[0].number, pair[0].name, pair[1].name
        )));
    }
    Ok(frames)
}

/// Shared numbered-frame collector for the safety pre-check, retaining the
/// existing sorting, duplicate-number and unreadable/symlink rules.
#[cfg(feature = "prechecks")]
pub(crate) fn numbered_frames(root: &Path) -> Result<Vec<(String, u64, PathBuf)>> {
    collect(root, "**", &RunConfig::default(), &mut Default::default())?
        .into_iter()
        .map(|f| match (f.path, f.problem) {
            (Some(path), None) => Ok((f.name, f.number, path)),
            (_, problem) => Err(Error::Config(format!(
                "unreadable sequence frame {:?}: {}",
                f.name,
                problem.as_deref().unwrap_or("no readable path")
            ))),
        })
        .collect()
}

#[cfg(feature = "graphics")]
fn temporal(
    frames: &[Frame],
    cfg: &RunConfig,
    side: &str,
    errors: &mut Vec<String>,
) -> Option<f64> {
    if frames.len() < 2 {
        return None;
    }
    let opts = CompareOptions {
        pixels_per_degree: cfg.pixels_per_degree,
        hdr: cfg.hdr,
    };
    let mut values = Vec::new();
    for pair in frames.windows(2) {
        let compared = match (&pair[0].path, &pair[1].path) {
            (Some(b), Some(c)) => (|| {
                crate::hdr::check_same_kind(b, c)?;
                if crate::hdr::is_hdr_path(b) {
                    let cmp = crate::hdr::compare_hdr(
                        &crate::hdr::decode_hdr(c)?,
                        &crate::hdr::decode_hdr(b)?,
                        &opts,
                    )?;
                    Ok(cmp.0.metrics.mean)
                } else {
                    Ok(crate::compare::compare_rgba(
                        &crate::run::decode(c)?,
                        &crate::run::decode(b)?,
                        &opts,
                    )?
                    .metrics
                    .mean)
                }
            })(),
            _ => Err(Error::Config("unreadable frame or symlink".into())),
        };
        match compared {
            Ok(v) if v.is_finite() => values.push(v),
            result => errors.push(format!(
                "{side} temporal pair {:?} -> {:?}: {}",
                pair[0].name,
                pair[1].name,
                result
                    .err()
                    .map_or_else(|| "non-finite FLIP".into(), |e| e.to_string())
            )),
        }
    }
    (values.len() == frames.len() - 1).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// Compare numbered colour frames by sorted index and write both report formats.
#[cfg(feature = "graphics")]
pub fn run_sequence(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    pattern: &str,
    cfg: &RunConfig,
) -> Result<SequenceReport> {
    cfg.validate()?;
    let mut excluded = std::collections::BTreeSet::new();
    let (base, cap) = (
        collect(baseline, pattern, cfg, &mut excluded)?,
        collect(capture, pattern, cfg, &mut excluded)?,
    );
    crate::run::guard_output_dir(
        out,
        &[baseline, capture],
        &[SEQUENCE_FILE, crate::run::RUN_SENTINEL],
    )?;
    std::fs::create_dir_all(out).map_err(io_err(format!("creating {}", out.display())))?;
    crate::run::clear_previous_report(out)?;
    let path = out.join(SEQUENCE_FILE);
    if std::fs::symlink_metadata(&path).is_ok() {
        std::fs::remove_file(&path).map_err(io_err(format!("removing {}", path.display())))?;
    }
    let sentinel = out.join(crate::run::RUN_SENTINEL);
    if std::fs::symlink_metadata(&sentinel).is_ok() {
        std::fs::remove_file(&sentinel)
            .map_err(io_err(format!("removing {}", sentinel.display())))?;
    }
    std::fs::write(&sentinel, b"incomplete sequence\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;
    let opts = CompareOptions {
        pixels_per_degree: cfg.pixels_per_degree,
        hdr: cfg.hdr,
    };
    let meta = cfg.meta.checker()?;
    let mut frames = Vec::new();
    for i in 0..base.len().max(cap.len()) {
        let (b, c) = (base.get(i), cap.get(i));
        let name = format!("frame_{i:08}.png");
        // Overrides/regions match the source baseline name (capture name for new frames).
        let source_name = b.or(c).map_or(name.as_str(), |f| f.name.as_str());
        let mut local = cfg.clone();
        let (metric, threshold) = cfg.effective_for(source_name);
        local.default_metric = metric;
        local.default_threshold = threshold;
        local.overrides.clear();
        local.buffers.clear();
        for region in &mut local.regions {
            if region
                .glob
                .as_deref()
                .is_some_and(|g| !compile_glob(g).is_ok_and(|m| m.is_match(source_name)))
            {
                region.glob = Some("__no_sequence_match__".into());
            } else {
                region.glob = None;
            }
        }
        for mask in &mut local.masks {
            if mask
                .glob
                .as_deref()
                .is_some_and(|g| !compile_glob(g).is_ok_and(|m| m.is_match(source_name)))
            {
                mask.glob = Some("__no_sequence_match__".into());
            } else {
                mask.glob = None;
            }
        }
        let mut entry = crate::run::build_entry(
            &name,
            b.and_then(Frame::source),
            c.and_then(Frame::source),
            out,
            &opts,
            &local,
        );
        if let (Some(b), Some(c)) = (b, c) {
            let checked = meta.check_named(baseline, &b.name, capture, &c.name);
            let failure = match checked {
                Ok(checked) => {
                    entry.meta_diff = checked.diff;
                    entry.meta_ignored_diff = checked.ignored;
                    entry.intended_variables = checked.intended;
                    entry.meta_declared_unchanged = checked.unchanged;
                    entry.capture_validity = checked.validity;
                    checked.failure
                }
                Err(e) => Some(e),
            };
            if let Some(error) = failure {
                entry.status = Status::Error;
                entry.error = Some(match entry.error.take() {
                    Some(e) => format!("{e}; {error}"),
                    None => error,
                });
            }
        }
        crate::run::apply_image_warnings(&mut entry, cfg);
        if !cfg.record_absolute_paths {
            crate::paths::redact_entry(&mut entry, out, &[baseline, capture]);
        }
        frames.push(SequenceFrame {
            index: i,
            baseline_name: b.map(|f| f.name.clone()),
            capture_name: c.map(|f| f.name.clone()),
            baseline_number: b.map(|f| f.number),
            capture_number: c.map(|f| f.number),
            entry,
        });
    }
    let mut totals = Totals {
        total: frames.len(),
        ..Totals::default()
    };
    for f in &frames {
        match f.entry.status {
            Status::Pass => totals.pass += 1,
            Status::Fail => totals.fail += 1,
            Status::Error => totals.error += 1,
            Status::Missing => totals.missing += 1,
            Status::New => totals.new += 1,
        }
    }
    let mut report = Report {
        perf_diff: None,
        perf_errors: Vec::new(),
        combined_verdict: None,
        exclusion_audit: None,
        schema: REPORT_SCHEMA.into(),
        tool_version: env!("CARGO_PKG_VERSION").into(),
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        // Synthetic names cannot be used by approve; actual names live in frames.
        baseline_dir: Some(crate::paths::record(
            baseline,
            out,
            cfg.record_absolute_paths,
        )),
        capture_dir: Some(crate::paths::record(
            capture,
            out,
            cfg.record_absolute_paths,
        )),
        config: ReportConfig {
            mask_mode: crate::compare::MaskMode::default(),
            entries: vec![pattern.into()],
            ignore: cfg.ignore.clone(),
            default_threshold: cfg.default_threshold,
            default_metric: cfg.default_metric,
            pixels_per_degree: cfg.pixels_per_degree,
            fail_on_new: cfg.fail_on_new,
            mode: cfg.mode,
            labels: cfg.labels.clone(),
            meta: meta.settings(),
            allow_empty: cfg.allow_empty,
            fail_on_nonfinite: cfg.fail_on_nonfinite,
            hotspot_fail: cfg.hotspot_fail,
            hotspot_local_max: cfg.hotspot_local_max,
            hotspot_local_min_pixels: cfg.hotspot_local_min_pixels,
        },
        totals,
        entries: frames.iter().map(|f| f.entry.clone()).collect(),
    };
    report.exclusion_audit = Some(crate::exclusions::audit(
        &report,
        excluded.into_iter().collect(),
    ));
    let mut temporal_errors = Vec::new();
    let baseline_temporal_mean = temporal(&base, cfg, "baseline", &mut temporal_errors);
    let capture_temporal_mean = temporal(&cap, cfg, "capture", &mut temporal_errors);
    let mean_flip_curve: Vec<_> = frames
        .iter()
        .map(|f| f.entry.metrics.map(|m| m.mean))
        .collect();
    let worst_frame = frames
        .iter()
        .filter_map(|f| {
            f.entry.metrics.map(|m| WorstFrame {
                index: f.index,
                name: f.entry.name.clone(),
                mean_flip: m.mean,
            })
        })
        .max_by(|a, b| {
            a.mean_flip
                .total_cmp(&b.mean_flip)
                .then(b.index.cmp(&a.index))
        });
    let tile_stability = if let Some(policy) = &cfg.temporal_tiles {
        let mut allocated_pixels = 0_u64;
        let mut load = |frames: &[Frame]| -> Result<Vec<image::RgbaImage>> {
            if frames.len() > 256 {
                return Err(Error::Config("temporal frame limit exceeded".into()));
            }
            frames
                .iter()
                .map(|f| {
                    let path = f
                        .path
                        .as_ref()
                        .ok_or_else(|| Error::Config("unreadable temporal frame".into()))?;
                    if crate::hdr::is_hdr_path(path) {
                        return Err(Error::Config(
                            "tile stability currently requires SDR frames".into(),
                        ));
                    }
                    let image = crate::evidence_quality::image(path)?.to_rgba8();
                    allocated_pixels += u64::from(image.width()) * u64::from(image.height());
                    if allocated_pixels > 33_554_432 {
                        return Err(Error::Config(
                            "temporal sequence allocation limit exceeded".into(),
                        ));
                    }
                    Ok(image)
                })
                .collect()
        };
        match load(&base).and_then(|b| {
            load(&cap).and_then(|c| {
                if b.len() != c.len() {
                    return Err(Error::Config("temporal paired frame count differs".into()));
                }
                let mut persistent: Option<Vec<bool>> = None;
                for (i, (bi, ci)) in b.iter().zip(&c).enumerate() {
                    let mut local = cfg.clone();
                    if let Some(layers) = &cfg.layers {
                        let bp = base[i]
                            .path
                            .as_ref()
                            .ok_or_else(|| Error::Config("missing frame".into()))?;
                        let cp = cap[i]
                            .path
                            .as_ref()
                            .ok_or_else(|| Error::Config("missing frame".into()))?;
                        let bl = crate::evidence_quality::layers::load_for_image(
                            bp,
                            layers,
                            &image::DynamicImage::ImageRgba8(bi.clone()),
                        )?;
                        let cl = crate::evidence_quality::layers::load_for_image(
                            cp,
                            layers,
                            &image::DynamicImage::ImageRgba8(ci.clone()),
                        )?;
                        local.layer_mask = Some(
                            crate::evidence_quality::layers::scope_pair(&bl, &cl, layers)?
                                .0
                                .into_iter()
                                .map(|v| !v)
                                .collect(),
                        );
                    }
                    for name in [&base[i].name, &cap[i].name] {
                        if let Some(mask) =
                            crate::regions::effective_mask(name, bi.width(), bi.height(), &local)?
                        {
                            let all = persistent.get_or_insert_with(|| vec![false; mask.len()]);
                            if all.len() != mask.len() {
                                return Err(Error::Config(
                                    "temporal scope geometry differs".into(),
                                ));
                            }
                            for (a, v) in all.iter_mut().zip(mask) {
                                *a |= v;
                            }
                        }
                    }
                }
                crate::evidence_quality::temporal::analyze_scoped(
                    &b,
                    &c,
                    policy,
                    persistent.as_deref(),
                )
            })
        }) {
            Ok(evidence) => {
                if evidence.verdict != "stable" {
                    temporal_errors
                        .push(format!("fixed-camera tile stability: {}", evidence.verdict));
                }
                Some(evidence)
            }
            Err(e) => {
                temporal_errors.push(e.to_string());
                None
            }
        }
    } else {
        None
    };
    let full = SequenceReport {
        tile_stability,
        exclusion_audit: report.exclusion_audit.clone(),
        schema: SEQUENCE_SCHEMA.into(),
        verdict: if report.is_regression() || !temporal_errors.is_empty() {
            "regression"
        } else {
            "pass"
        }
        .into(),
        pattern: pattern.into(),
        baseline_frames: base.len(),
        capture_frames: cap.len(),
        totals,
        mean_flip_curve,
        baseline_temporal_mean,
        capture_temporal_mean,
        temporal_instability: capture_temporal_mean
            .zip(baseline_temporal_mean)
            .map(|(c, b)| c - b),
        temporal_errors,
        worst_frame,
        frames_over_threshold: frames
            .iter()
            .filter(|f| f.entry.value.is_some_and(|v| v > f.entry.threshold))
            .count(),
        report_json: crate::paths::cwd(&path, cfg.record_absolute_paths),
        frames_report_json: crate::paths::cwd(
            &out.join(REPORT_FILE_NAME),
            cfg.record_absolute_paths,
        ),
        index_html: crate::paths::cwd(&out.join("index.html"), cfg.record_absolute_paths),
        frames,
    };
    for (file, json) in [
        (REPORT_FILE_NAME, serde_json::to_string_pretty(&report)?),
        (SEQUENCE_FILE, serde_json::to_string_pretty(&full)?),
    ] {
        let path = out.join(file);
        std::fs::write(&path, json).map_err(io_err(format!("writing {}", path.display())))?;
    }
    crate::render::render_sequence_html(&report, &full, out)?;
    std::fs::remove_file(&sentinel).map_err(io_err(format!("removing {}", sentinel.display())))?;
    Ok(full)
}

/// Returns `feature_unavailable` when graphics computation is not compiled.
#[cfg(not(feature = "graphics"))]
pub fn run_sequence(
    _baseline: &Path,
    _capture: &Path,
    _out: &Path,
    _pattern: &str,
    _cfg: &RunConfig,
) -> Result<SequenceReport> {
    Err(Error::FeatureUnavailable {
        feature: "graphics",
    })
}
