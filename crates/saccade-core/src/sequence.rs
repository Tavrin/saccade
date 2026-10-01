//! Numbered image sequences and added temporal instability.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::compare::CompareOptions;
use crate::config::{RunConfig, compile_glob};
use crate::error::{Error, Result};
use crate::report::{Entry, REPORT_FILE_NAME, REPORT_SCHEMA, Report, ReportConfig, Status, Totals};
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

struct Frame {
    name: String,
    number: u64,
    path: Option<PathBuf>,
    problem: Option<String>,
}

impl Frame {
    fn source(&self) -> Option<Source<'_>> {
        match (&self.path, &self.problem) {
            (Some(p), _) => Some(Source::File(p)),
            (_, Some(why)) => Some(Source::Problem(why)),
            _ => None,
        }
    }
}

fn collect(root: &Path, pattern: &str, cfg: &RunConfig) -> Result<Vec<Frame>> {
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
pub fn run_sequence(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    pattern: &str,
    cfg: &RunConfig,
) -> Result<SequenceReport> {
    cfg.validate()?;
    let (base, cap) = (
        collect(baseline, pattern, cfg)?,
        collect(capture, pattern, cfg)?,
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
            let checked = (|| {
                let (bm, cm) = (meta.load(baseline, &b.name)?, meta.load(capture, &c.name)?);
                let (diff, ignored) = meta.diff_split(bm.as_ref(), cm.as_ref());
                let bad = meta.violations(&diff);
                let error = (!bad.is_empty()).then(|| {
                    format!(
                        "configuration differs on undeclared keys: {}",
                        bad.join(", ")
                    )
                });
                Ok::<_, String>((diff, ignored, error))
            })();
            let failure = match checked {
                Ok((diff, ignored, failure)) => {
                    entry.meta_diff = diff;
                    entry.meta_ignored_diff = ignored;
                    failure
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
    let report = Report {
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
        },
        totals,
        entries: frames.iter().map(|f| f.entry.clone()).collect(),
    };
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
    let full = SequenceReport {
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
