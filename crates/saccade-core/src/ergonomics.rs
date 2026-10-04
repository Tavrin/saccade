//! Entry inspection, repeated-capture noise calibration and CI XML export.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{RunConfig, compile_glob};
use crate::report::{Entry, Metric, Report, Status};
use crate::{Error, Result};

/// A filtered, stable-order page of full report entries.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
pub struct EntriesPage {
    /// Always `saccade-entries.v1`.
    pub schema: String,
    /// Number of entries matching the filters, before pagination.
    pub total: usize,
    /// Offset into the filtered list.
    pub offset: usize,
    /// Requested page size.
    pub limit: usize,
    /// Cursor for the next page, or null at the end.
    pub next_cursor: Option<String>,
    /// Complete entry payloads, in report order.
    pub entries: Vec<Entry>,
}

/// Selects a page. Cursors encode the next offset in the filtered list.
pub fn entries(
    report: &Report,
    statuses: &[String],
    name: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<EntriesPage> {
    let statuses: Vec<Status> = statuses
        .iter()
        .map(|s| match s.as_str() {
            "pass" => Ok(Status::Pass),
            "fail" => Ok(Status::Fail),
            "error" => Ok(Status::Error),
            "new" => Ok(Status::New),
            "missing" => Ok(Status::Missing),
            _ => Err(Error::Config(format!(
                "--status: unknown status {s:?}; use pass,fail,error,new,missing"
            ))),
        })
        .collect::<Result<_>>()?;
    let matcher = name.map(compile_glob).transpose()?;
    let selected: Vec<_> = report
        .entries
        .iter()
        .filter(|e| {
            (statuses.is_empty() || statuses.contains(&e.status))
                && matcher.as_ref().is_none_or(|g| g.is_match(&e.name))
        })
        .collect();
    let shown: Vec<Entry> = selected
        .iter()
        .skip(offset)
        .take(limit)
        .map(|e| (*e).clone())
        .collect();
    let end = offset.saturating_add(shown.len());
    Ok(EntriesPage {
        schema: "saccade-entries.v1".into(),
        total: selected.len(),
        offset,
        limit,
        next_cursor: (limit > 0 && end < selected.len()).then(|| end.to_string()),
        entries: shown,
    })
}

/// Largest pairwise noise statistics for one image.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
pub struct NoiseEntry {
    /// Relative image name.
    pub name: String,
    /// Number of measured pairs.
    pub pairs: usize,
    /// Largest pairwise mean FLIP.
    pub mean: f64,
    /// Largest pairwise p95 FLIP.
    pub p95: f64,
    /// Largest pairwise max FLIP.
    pub max: f64,
    /// Largest observed value of the chosen metric multiplied by margin.
    pub suggested_threshold: f64,
}

/// Repeated-capture calibration output.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
pub struct NoiseReport {
    /// Always `saccade-noise.v1`.
    pub schema: String,
    /// Image-noise discriminant; historical records can omit it.
    #[serde(default = "image_noise_kind")]
    pub kind: String,
    /// Dimensionless FLIP error units; historical records can omit it.
    #[serde(default = "image_noise_unit")]
    pub unit: String,
    /// Metric used for suggestions.
    pub metric: Metric,
    /// Multiplier over the largest observed value.
    pub margin: f64,
    /// Capture directories, relative to the output configuration directory.
    pub runs: Vec<String>,
    /// Per-image noise estimates.
    pub entries: Vec<NoiseEntry>,
    /// Limitations and high-noise warnings.
    pub warnings: Vec<String>,
    /// Declared build identity for each input, indexed in the order of `runs`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capture_provenance: BTreeMap<String, BTreeMap<String, String>>,
    /// Raw repeat ranges, timer quantum and minimum meaningful delta settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perf_noise: Option<crate::perf::PerfNoise>,
}
fn image_noise_kind() -> String {
    "image_noise".into()
}
fn image_noise_unit() -> String {
    "FLIP".into()
}

/// Compares every distinct pair of unchanged-build captures and writes TOML overrides.
/// Missing images and comparison errors prevent misleading suggestions.
pub fn noise(dirs: &[PathBuf], margin: f64, metric: Metric, out: &Path) -> Result<NoiseReport> {
    noise_with_perf(dirs, margin, metric, out, crate::perf::DEFAULT_PERF_NAME)
}

/// Noise calibration with a configurable performance sidecar file name.
pub fn noise_with_perf(
    dirs: &[PathBuf],
    margin: f64,
    metric: Metric,
    out: &Path,
    perf_name: &str,
) -> Result<NoiseReport> {
    noise_with_perf_options(
        dirs,
        margin,
        metric,
        out,
        &crate::perf::PerfOptions {
            name: perf_name.into(),
            ..Default::default()
        },
    )
}

/// Noise calibration with timer resolution and minimum meaningful delta options.
pub fn noise_with_perf_options(
    dirs: &[PathBuf],
    margin: f64,
    metric: Metric,
    out: &Path,
    perf: &crate::perf::PerfOptions,
) -> Result<NoiseReport> {
    if dirs.len() < 2 || !margin.is_finite() || margin < 1.0 {
        return Err(Error::Config(
            "noise needs at least two RUN_DIR arguments and --margin >= 1".into(),
        ));
    }
    if dirs
        .iter()
        .any(|d| crate::run::normalise_path(out).starts_with(crate::run::normalise_path(d)))
    {
        return Err(Error::Config(
            "--out is inside a noise input directory; choose a sibling saccade.noise.toml".into(),
        ));
    }
    perf.validate()?;
    #[cfg(not(feature = "graphics"))]
    if perf != &crate::perf::PerfOptions::default() {
        return Err(Error::FeatureUnavailable {
            feature: "graphics",
        });
    }
    #[cfg(feature = "graphics")]
    let (perf_noise, perf_warnings) = crate::perf::noise_with_options(dirs, perf)?;
    #[cfg(not(feature = "graphics"))]
    let (perf_noise, perf_warnings): (Option<crate::perf::PerfNoise>, Vec<String>) =
        (None, Vec::new());
    let temp = tempfile::tempdir().map_err(crate::run::io_err(
        "creating noise scratch directory".into(),
    ))?;
    let mut values = BTreeMap::<String, NoiseEntry>::new();
    for i in 0..dirs.len() {
        for j in i + 1..dirs.len() {
            let report = crate::run::run(
                &dirs[i],
                &dirs[j],
                &temp.path().join(format!("{i}-{j}")),
                &RunConfig {
                    default_metric: metric,
                    ..Default::default()
                },
            )?;
            if report.entries.is_empty() || report.entries.iter().any(|e| e.metrics.is_none()) {
                return Err(Error::Config(format!(
                    "noise inputs {} and {} need identical image names and decodable pairs",
                    dirs[i].display(),
                    dirs[j].display()
                )));
            }
            if report
                .entries
                .iter()
                .any(|e| e.warnings.iter().any(|w| w == "same capture, not a repeat"))
            {
                return Err(Error::Config(format!(
                    "same capture, not a repeat: {} and {}",
                    dirs[i].display(),
                    dirs[j].display()
                )));
            }
            for e in report.entries {
                let Some(m) = e.metrics else { continue };
                let n = values.entry(e.name.clone()).or_insert(NoiseEntry {
                    name: e.name,
                    pairs: 0,
                    mean: 0.0,
                    p95: 0.0,
                    max: 0.0,
                    suggested_threshold: 0.0,
                });
                n.pairs += 1;
                n.mean = n.mean.max(m.mean);
                n.p95 = n.p95.max(m.p95);
                n.max = n.max.max(m.max);
                n.suggested_threshold = n
                    .suggested_threshold
                    .max(crate::run::metric_value(&m, metric) * margin);
                if !n.suggested_threshold.is_finite() {
                    return Err(Error::Config(
                        "--margin produces a non-finite threshold; reduce it".into(),
                    ));
                }
            }
        }
    }
    let entries: Vec<_> = values.into_values().collect();
    let mut warnings = vec!["Noise alone cannot prove separation from real changes; validate against a known changed build.".into()];
    let mut capture_provenance = BTreeMap::new();
    for (index, dir) in dirs.iter().enumerate() {
        let evidence =
            crate::meta::capture_evidence(dir, "capture.png", crate::meta::DEFAULT_META_NAME);
        for field in ["binary_sha256", "source_head"] {
            if !evidence.contains_key(field) {
                warnings.push(format!("run {index} {field} provenance is absent"));
            }
        }
        capture_provenance.insert(index.to_string(), evidence);
    }
    let mut toml = String::from(
        "# Repeated captures of an unchanged build; thresholds = largest observed metric × margin.\n",
    );
    if perf_noise.is_some() {
        toml.push_str(&format!("perf_noise_k = {:.17}\n", perf.k));
    }
    for e in &entries {
        if e.suggested_threshold >= 0.1 {
            warnings.push(format!(
                "{}: high noise ({:.4}); no separation from changes at or below this level",
                e.name, e.suggested_threshold
            ));
        }
        let literal_glob = globset::escape(&e.name.replace('\\', "\\\\"));
        compile_glob(&literal_glob)?;
        toml.push_str(&format!(
            "\n[[override]]\nglob = {}\nmetric = {}\nthreshold = {:.17}\n",
            serde_json::to_string(&literal_glob)?,
            serde_json::to_string(&metric)?,
            e.suggested_threshold
        ));
    }
    warnings.extend(perf_warnings);
    if let Some(floor) = &perf_noise {
        toml.push_str(&format!(
            "\n[perf_noise]\nframe = {:.17}\nresolution_ticks = {}\nmin_delta_ms = {:.17}\nmin_delta_pct = {:.17}\n",
            floor.frame, floor.resolution_ticks, floor.min_delta_ms, floor.min_delta_pct
        ));
        if let Some(q) = floor.resolution_ms {
            toml.push_str(&format!("resolution_ms = {q:.17}\n"));
        }
        toml.push_str(&format!(
            "comparability = {}\n",
            serde_json::to_string(&floor.comparability)?
        ));
        if let Some(timer) = &floor.timer {
            toml.push_str(&format!("timer = {}\n", serde_json::to_string(timer)?));
        }
        if let Some(identity) = &floor.context_identity {
            toml.push_str(&format!(
                "context_identity = {}\n",
                serde_json::to_string(identity)?
            ));
        }
        toml.push_str("\n[perf_noise.terms]\n");
        for (id, value) in &floor.terms {
            toml.push_str(&format!("{} = {:.17}\n", serde_json::to_string(id)?, value));
        }
    }
    RunConfig::from_toml_str(&toml)?;
    std::fs::write(out, toml).map_err(crate::run::io_err(format!(
        "writing noise configuration {}",
        out.display()
    )))?;
    Ok(NoiseReport {
        schema: "saccade-noise.v1".into(),
        kind: image_noise_kind(),
        unit: image_noise_unit(),
        metric,
        margin,
        runs: dirs
            .iter()
            .map(|d| crate::paths::record(d, out.parent().unwrap_or(Path::new(".")), false))
            .collect(),
        entries,
        warnings,
        capture_provenance,
        perf_noise,
    })
}

fn xml(text: &str) -> String {
    text.chars()
        .filter(|c| {
            matches!(c, '\t' | '\n' | '\r') || *c >= ' ' && *c != '\u{fffe}' && *c != '\u{ffff}'
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Writes one JUnit testcase per report entry. Missing entries fail; new entries
/// fail when configured to, otherwise they are skipped. Errors use failure nodes.
pub fn junit(report: &Report, out: &Path) -> Result<()> {
    let mut body = String::new();
    let mut failures = 0;
    let mut skipped = 0;
    for e in &report.entries {
        let message = e
            .error
            .as_deref()
            .or_else(|| e.diagnostics.as_ref().map(|d| d.description.as_str()))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{:?}: {:?} value {:?}, threshold {}",
                    e.status, e.metric_used, e.value, e.threshold
                )
            });
        body.push_str(&format!(
            "  <testcase name=\"{}\" classname=\"saccade\">",
            xml(&e.name)
        ));
        match e.status {
            Status::Pass => {}
            Status::New if !report.config.fail_on_new => {
                skipped += 1;
                body.push_str(&format!("<skipped message=\"{}\"/>", xml(&message)));
            }
            _ => {
                failures += 1;
                body.push_str(&format!(
                    "<failure message=\"{}\">{}</failure>",
                    xml(&message),
                    xml(&message)
                ));
            }
        }
        body.push_str("</testcase>\n");
    }
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"saccade\" tests=\"{}\" failures=\"{failures}\" errors=\"0\" skipped=\"{skipped}\">\n{body}</testsuite>\n",
        report.entries.len()
    );
    std::fs::write(out, document).map_err(crate::run::io_err(format!(
        "writing JUnit {}",
        out.display()
    )))
}
