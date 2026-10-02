//! Bounded binary search of ordered image runs. Capture execution belongs to the CLI.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{RunConfig, compile_glob};
use crate::explain::absolute;
use crate::report::{Metric, Mode, REPORT_FILE_NAME, Status, Totals};
use crate::run::{guard_output_dir, run};
use crate::{Error, Result};

/// Search options shared by existing runs and user-command capture probes.
#[derive(Debug, Clone, Default)]
pub struct BisectOptions {
    /// An explicit threshold switches from native sample identity to FLIP.
    pub threshold: Option<f64>,
    /// Deciding FLIP statistic; defaults to max.
    pub metric: Option<Metric>,
    /// Optional image-name glob.
    pub entries: Option<String>,
}

/// Result of one probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Probe {
    /// Zero-based position in the supplied order.
    pub index: usize,
    /// Run path or immutable git revision.
    pub target: String,
    /// good, bad, or skip.
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["good", "bad", "skip"])))]
    pub verdict: String,
    /// Absolute report directory, including for image errors.
    pub report_dir: Option<String>,
    /// Reason for a skipped probe.
    pub reason: Option<String>,
}

/// `saccade-bisect.v1` result. Skips can make the answer inconclusive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BisectResult {
    /// Schema identifier.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-bisect.v1")))]
    pub schema: String,
    /// found, pass, inconclusive, or non_monotonic.
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["found", "pass", "inconclusive", "non_monotonic"])))]
    pub status: String,
    /// Proven first bad target under the monotonic assumption; null if ambiguous.
    pub first_bad: Option<String>,
    /// Last observed good target, or the supplied known-good anchor.
    pub last_good: Option<String>,
    /// Possible first-bad targets when skips prevent an exact answer.
    pub candidates: Vec<String>,
    /// Probes in execution order.
    pub probes: Vec<Probe>,
    /// Number of probes performed.
    pub total_probes: usize,
    /// Observed bad-before-good pairs (target names).
    pub non_monotonic: Vec<[String; 2]>,
}

impl BisectResult {
    /// Exit 0 for all observed good, 1 for divergence, 2 for uncertainty.
    pub fn exit_code(&self) -> u8 {
        match self.status.as_str() {
            "pass" => 0,
            "found" => 1,
            _ => 2,
        }
    }

    /// A lean human-readable summary.
    pub fn text(&self) -> String {
        format!(
            "bisect: {}; first bad: {}; last good: {}; {} probes; {} non-monotonic observations{}",
            self.status,
            self.first_bad.as_deref().unwrap_or("unknown"),
            self.last_good.as_deref().unwrap_or("none"),
            self.total_probes,
            self.non_monotonic.len(),
            if self.candidates.is_empty() {
                String::new()
            } else {
                format!("; candidates: {}", self.candidates.join(", "))
            }
        )
    }
}

/// Validate and prepare a bisect report directory without touching inputs.
pub fn prepare(out: &Path, inputs: &[&Path]) -> Result<PathBuf> {
    let out = absolute(out);
    for input in inputs {
        let input = absolute(input);
        if input.starts_with(&out) || out.starts_with(&input) {
            return Err(Error::Config(
                "bisect output must be separate from every input".into(),
            ));
        }
    }
    guard_output_dir(&out, inputs, &["saccade-bisect.v1.json"])?;
    if std::fs::symlink_metadata(out.join("saccade-bisect.v1.json"))
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Config("bisect result must not be a symlink".into()));
    }
    std::fs::create_dir_all(&out).map_err(io("creating bisect output"))?;
    Ok(out)
}

/// Compare one probe, retaining the report. Errors decoding images are skips.
pub fn compare_probe(
    reference: &Path,
    capture: &Path,
    out: &Path,
    opts: &BisectOptions,
    index: usize,
    target: String,
) -> Result<Probe> {
    let mut probe = Probe {
        index,
        target,
        verdict: "skip".into(),
        report_dir: None,
        reason: None,
    };
    if !capture.is_dir() {
        probe.reason = Some("capture directory is missing".into());
        return Ok(probe);
    }
    let matcher = opts.entries.as_deref().map(compile_glob).transpose()?;
    let cfg = RunConfig {
        mode: if opts.threshold.is_some() {
            Mode::Regression
        } else {
            Mode::Identity
        },
        default_threshold: opts.threshold.unwrap_or(0.0),
        default_metric: opts.metric.unwrap_or(Metric::Max),
        fail_on_new: true,
        ..RunConfig::default()
    };
    let mut report = run(reference, capture, out, &cfg)?;
    report
        .entries
        .retain(|e| matcher.as_ref().is_none_or(|m| m.is_match(&e.name)));
    report.totals = Totals {
        total: report.entries.len(),
        ..Totals::default()
    };
    for entry in &mut report.entries {
        if opts.threshold.is_none()
            && entry.status == Status::Pass
            && entry.bit_identical == Some(false)
        {
            entry.status = Status::Fail;
        }
        match entry.status {
            Status::Pass => report.totals.pass += 1,
            Status::Fail => report.totals.fail += 1,
            Status::Missing => report.totals.missing += 1,
            Status::New => report.totals.new += 1,
            Status::Error => report.totals.error += 1,
        }
    }
    std::fs::write(
        out.join(REPORT_FILE_NAME),
        serde_json::to_vec_pretty(&report)?,
    )
    .map_err(io("writing filtered bisect report"))?;
    crate::render::render_html(&report, out)?;
    probe.report_dir = Some(crate::paths::portable(&absolute(out)));
    if report.totals.error > 0 || report.entries.is_empty() {
        probe.reason = Some("image errors or no selected images".into());
    } else {
        probe.verdict = if report.is_regression() {
            "bad"
        } else {
            "good"
        }
        .into();
    }
    Ok(probe)
}

/// Binary-search a monotonic series. A skipped midpoint tries another point in
/// the remaining interval; skipped boundary points never become known good.
pub fn search(
    targets: &[String],
    mut probe: impl FnMut(usize, &str) -> Result<Probe>,
) -> Result<BisectResult> {
    if targets.is_empty() {
        return Err(Error::Config("bisect needs at least one target".into()));
    }
    let mut seen: Vec<Option<String>> = vec![None; targets.len()];
    let mut probes = Vec::new();
    for index in [0, targets.len() - 1] {
        if seen[index].is_none() {
            let p = probe(index, &targets[index])?;
            seen[index] = Some(p.verdict.clone());
            probes.push(p);
        }
    }
    loop {
        let bad = seen.iter().position(|v| v.as_deref() == Some("bad"));
        let end = bad.unwrap_or(targets.len());
        let good = seen[..end]
            .iter()
            .rposition(|v| v.as_deref() == Some("good"));
        let start = good.map_or(0, |g| g + 1);
        let mid = start + (end - start) / 2;
        let index = (start..end)
            .filter(|&i| seen[i].is_none())
            .min_by_key(|&i| i.abs_diff(mid));
        let Some(index) = index else { break };
        let p = probe(index, &targets[index])?;
        seen[index] = Some(p.verdict.clone());
        probes.push(p);
    }
    let bad = seen.iter().position(|v| v.as_deref() == Some("bad"));
    let end = bad.unwrap_or(targets.len());
    let good = seen[..end]
        .iter()
        .rposition(|v| v.as_deref() == Some("good"));
    let start = good.map_or(0, |g| g + 1);
    let mut candidates: Vec<_> = (start..end)
        .filter(|&i| seen[i].as_deref() != Some("good"))
        .map(|i| targets[i].clone())
        .collect();
    if !candidates.is_empty()
        && let Some(i) = bad
    {
        candidates.push(targets[i].clone());
    }
    let mut non_monotonic = Vec::new();
    for (i, v) in seen.iter().enumerate() {
        if v.as_deref() == Some("bad") {
            for (j, v) in seen.iter().enumerate().skip(i + 1) {
                if v.as_deref() == Some("good") {
                    non_monotonic.push([targets[i].clone(), targets[j].clone()]);
                }
            }
        }
    }
    let status = if !non_monotonic.is_empty() {
        "non_monotonic"
    } else if !candidates.is_empty() {
        "inconclusive"
    } else if bad.is_some() {
        "found"
    } else {
        "pass"
    };
    Ok(BisectResult {
        schema: "saccade-bisect.v1".into(),
        status: status.into(),
        first_bad: (status == "found")
            .then(|| bad.map(|i| targets[i].clone()))
            .flatten(),
        last_good: good.map(|i| targets[i].clone()),
        candidates,
        total_probes: probes.len(),
        probes,
        non_monotonic,
    })
}

/// Mode A: search existing runs in supplied chronological order.
pub fn runs(
    runs: &[PathBuf],
    good: Option<&Path>,
    out: &Path,
    opts: &BisectOptions,
) -> Result<BisectResult> {
    let reference = good
        .or_else(|| runs.first().map(PathBuf::as_path))
        .ok_or_else(|| Error::Config("bisect needs runs".into()))?;
    if !reference.is_dir() {
        return Err(Error::Config(
            "reference must be an existing directory".into(),
        ));
    }
    opts.entries.as_deref().map(compile_glob).transpose()?;
    let inputs: Vec<_> = std::iter::once(reference)
        .chain(runs.iter().map(PathBuf::as_path))
        .collect();
    let out = prepare(out, &inputs)?;
    let targets: Vec<_> = runs
        .iter()
        .map(|p| crate::paths::portable(&absolute(p)))
        .collect();
    let mut result = search(&targets, |i, target| {
        compare_probe(
            reference,
            &runs[i],
            &probe_dir(&out, i)?,
            opts,
            i,
            target.into(),
        )
    })?;
    if result.last_good.is_none() && good.is_some() {
        result.last_good = Some(crate::paths::portable(&absolute(reference)));
    }
    write_result(&out, &result)?;
    Ok(result)
}

/// Save the result alongside per-probe reports.
pub fn write_result(out: &Path, result: &BisectResult) -> Result<()> {
    if std::fs::symlink_metadata(out.join("saccade-bisect.v1.json"))
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Config("bisect result must not be a symlink".into()));
    }
    std::fs::write(
        out.join("saccade-bisect.v1.json"),
        serde_json::to_vec_pretty(result)?,
    )
    .map_err(io("writing bisect result"))
}

/// Resolve a report child without letting preexisting symlinks redirect writes.
pub fn probe_dir(out: &Path, index: usize) -> Result<PathBuf> {
    let child = out.join(format!("probe-{index}"));
    if absolute(&child) != child {
        return Err(Error::Config(
            "bisect probe directory must not be a symlink".into(),
        ));
    }
    Ok(child)
}

fn io(context: &str) -> impl FnOnce(std::io::Error) -> Error {
    crate::run::io_err(context.into())
}
