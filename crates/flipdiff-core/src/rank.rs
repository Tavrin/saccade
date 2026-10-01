//! Rank candidate image directories against one common reference.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::RunConfig;
use crate::error::{Error, Result};
use crate::report::{Metric, REPORT_FILE_NAME, Report, Status, Totals};
use crate::run::io_err;

/// Ranking JSON schema identifier.
pub const RANK_SCHEMA: &str = "flipdiff-rank.v1";
/// Ranking report filename.
pub const RANK_FILE: &str = "flipdiff-rank.v1.json";
/// Markdown ranking table filename.
pub const RANK_MD: &str = "ranking.md";

/// One candidate's result for an image.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageRank {
    /// Candidate label (also the report subdirectory).
    pub label: String,
    /// Competition rank (1, 1, 3 for a tie); null for an unavailable comparison.
    pub rank: Option<usize>,
    /// Metric value; null for missing, new or erroneous entries.
    pub value: Option<f64>,
    /// The normal report's entry status.
    pub status: Status,
}

/// Ranking of the candidates for one relative image name.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedImage {
    /// Relative image name.
    pub name: String,
    /// Candidates in increasing metric order; unavailable results last.
    pub candidates: Vec<ImageRank>,
}

/// Overall comparison of one candidate.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateRank {
    /// Candidate label.
    pub label: String,
    /// Competition rank by mean rank, with mean metric as a tie-breaker.
    /// Null unless every reference image has a valid comparison for every candidate.
    pub rank: Option<usize>,
    /// Mean competition rank across the common, successfully compared images.
    pub mean_rank: Option<f64>,
    /// Mean metric across the same common image set.
    pub mean_metric: Option<f64>,
    /// Number of images used by both overall means.
    pub ranked_images: usize,
    /// Number of bit-identical pairs (all pairs, including outside the common set).
    pub bit_identical: usize,
    /// Per-candidate status totals.
    pub totals: Totals,
    /// Whether every reference image was compared successfully.
    pub complete: bool,
    /// Absolute normal report JSON path.
    pub report_json: String,
    /// Relative HTML link from the ranking report.
    pub report_html: String,
}

/// Full ranking on disk; lean outputs omit `images`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankReport {
    /// Always `flipdiff-rank.v1`.
    pub schema: String,
    /// Compare's verdict across all candidate reports.
    pub verdict: String,
    /// Metric used for every candidate and image (per-path metric overrides ignored).
    pub metric: Metric,
    /// Absolute reference directory.
    pub reference_dir: String,
    /// Number of reference images.
    pub reference_images: usize,
    /// Number of reference images successfully compared by every candidate.
    pub common_images: usize,
    /// Overall ranking; incomplete runs publish means but no overall winner.
    pub overall: Vec<CandidateRank>,
    /// Per-image rankings; omitted from lean output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<RankedImage>,
    /// Absolute ranking JSON path.
    pub report_json: String,
    /// Absolute HTML path.
    pub index_html: String,
    /// Absolute Markdown table path.
    pub markdown: String,
}

impl RankReport {
    /// Whether any candidate has a regression.
    pub fn is_regression(&self) -> bool {
        self.verdict == "regression"
    }

    /// Bounded machine output; per-image rankings remain on disk.
    pub fn lean(&self) -> Self {
        let mut out = self.clone();
        out.images.clear();
        out
    }

    /// Text table of the overall ranking.
    pub fn text(&self) -> String {
        let mut out = format!(
            "rank: {} ({:?}; {} common / {} reference images)\nRANK  CANDIDATE  MEAN RANK  MEAN METRIC  IDENTICAL\n",
            self.verdict, self.metric, self.common_images, self.reference_images
        );
        for c in &self.overall {
            out.push_str(&format!(
                "{}  {}  {}  {}  {}\n",
                c.rank.map_or_else(|| "-".into(), |v| v.to_string()),
                c.label,
                number(c.mean_rank),
                number(c.mean_metric),
                c.bit_identical
            ));
        }
        out.push_str(&format!("report: {}\n", self.index_html));
        out
    }

    /// Markdown tables of the overall and per-image rankings.
    pub fn markdown(&self) -> String {
        let mut out = format!(
            "# flipdiff ranking\n\nMetric: `{:?}`. Verdict: **{}**. {} common / {} reference images.\n\n| Rank | Candidate | Mean rank | Mean metric | Bit-identical | Report |\n|---:|---|---:|---:|---:|---|\n",
            self.metric, self.verdict, self.common_images, self.reference_images
        );
        for c in &self.overall {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | [HTML]({}) |\n",
                c.rank.map_or_else(|| "-".into(), |v| v.to_string()),
                c.label,
                number(c.mean_rank),
                number(c.mean_metric),
                c.bit_identical,
                c.report_html
            ));
        }
        out.push_str("\n| Image | Candidate | Rank | Value | Status |\n|---|---|---:|---:|---|\n");
        for image in &self.images {
            let name = image
                .name
                .replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace(['\n', '\r'], " ")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            for c in &image.candidates {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {:?} |\n",
                    name,
                    c.label,
                    c.rank.map_or_else(|| "-".into(), |v| v.to_string()),
                    number(c.value),
                    c.status
                ));
            }
        }
        out
    }
}

fn number(v: Option<f64>) -> String {
    v.map_or_else(|| "-".into(), |v| format!("{v:.6}"))
}

fn labels(dirs: &[PathBuf], supplied: Option<&[String]>) -> Result<Vec<String>> {
    let names = supplied.map_or_else(
        || {
            dirs.iter()
                .map(|d| {
                    d.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("candidate")
                        .to_string()
                })
                .collect()
        },
        <[String]>::to_vec,
    );
    if names.len() != dirs.len() || names.is_empty() {
        return Err(Error::Config(
            "rank requires a label for each candidate directory".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for name in &names {
        if name.is_empty()
            || name == "."
            || name == ".."
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || [
                "images",
                "index.html",
                RANK_FILE,
                RANK_MD,
                crate::run::RUN_SENTINEL,
            ]
            .iter()
            .any(|s| name.eq_ignore_ascii_case(s))
            || !seen.insert(name.to_ascii_lowercase())
        {
            return Err(Error::Config(format!(
                "rank label {name:?} must be unique and a safe directory name (ASCII letters, digits, dot, underscore, hyphen)"
            )));
        }
    }
    Ok(names)
}

/// Compare each candidate against the reference and write rankings and child reports.
pub fn run_rank(
    reference: &Path,
    candidates: &[PathBuf],
    supplied_labels: Option<&[String]>,
    metric: Metric,
    out: &Path,
    cfg: &RunConfig,
) -> Result<RankReport> {
    cfg.validate()?;
    if !cfg.buffers.is_empty() {
        return Err(Error::Config(
            "rank uses a common FLIP metric; numerical buffers belong in compare".into(),
        ));
    }
    let labels = labels(candidates, supplied_labels)?;
    let inputs: Vec<&Path> = std::iter::once(reference)
        .chain(candidates.iter().map(PathBuf::as_path))
        .collect();
    // Preflight all destinations against all inputs before writing any candidate report.
    crate::run::guard_output_dir(out, &inputs, &[RANK_FILE, crate::run::RUN_SENTINEL])?;
    for (dir, label) in candidates.iter().zip(&labels) {
        crate::run::collect_images(dir)?;
        let child = out.join(label);
        if std::fs::symlink_metadata(&child).is_ok_and(|m| m.file_type().is_symlink())
            || !crate::explain::absolute(&child).starts_with(crate::explain::absolute(out))
        {
            return Err(Error::Config(format!(
                "rank report {} escapes the output directory through a symlink",
                child.display()
            )));
        }
        crate::run::guard_output_dir(
            &child,
            &inputs,
            &[REPORT_FILE_NAME, crate::run::RUN_SENTINEL],
        )?;
    }
    let reference_files = crate::run::collect_images(reference)?;
    let ignores = cfg
        .ignore
        .iter()
        .map(|g| crate::config::compile_glob(g))
        .collect::<Result<Vec<_>>>()?;
    let reference_names: BTreeSet<_> = reference_files
        .files
        .keys()
        .chain(reference_files.problems.keys())
        .filter(|name| !ignores.iter().any(|g| g.is_match(name)))
        .cloned()
        .collect();
    std::fs::create_dir_all(out).map_err(io_err(format!("creating {}", out.display())))?;
    for file in [RANK_FILE, RANK_MD, "index.html"] {
        let path = out.join(file);
        if std::fs::symlink_metadata(&path).is_ok() {
            std::fs::remove_file(&path).map_err(io_err(format!("removing {}", path.display())))?;
        }
    }
    let sentinel = out.join(crate::run::RUN_SENTINEL);
    if std::fs::symlink_metadata(&sentinel).is_ok() {
        std::fs::remove_file(&sentinel)
            .map_err(io_err(format!("removing {}", sentinel.display())))?;
    }
    std::fs::write(&sentinel, b"incomplete ranking\n")
        .map_err(io_err(format!("writing {}", sentinel.display())))?;
    let mut reports: Vec<Report> = Vec::new();
    for (dir, label) in candidates.iter().zip(&labels) {
        let mut local = cfg.clone();
        local.default_metric = metric;
        for override_ in &mut local.overrides {
            override_.metric = None;
        }
        local.labels.baseline = "reference".into();
        local.labels.capture.clone_from(label);
        reports.push(crate::run::run(reference, dir, &out.join(label), &local)?);
    }
    let names: BTreeSet<_> = reports
        .iter()
        .flat_map(|r| r.entries.iter().map(|e| e.name.clone()))
        .collect();
    let mut images = Vec::new();
    for name in names {
        let mut row: Vec<_> = reports
            .iter()
            .zip(&labels)
            .map(|(r, label)| {
                let entry = r.entries.iter().find(|e| e.name == name);
                ImageRank {
                    label: label.clone(),
                    rank: None,
                    value: entry
                        .filter(|e| matches!(e.status, Status::Pass | Status::Fail))
                        .and_then(|e| e.metrics.as_ref())
                        .map(|m| crate::run::metric_value(m, metric))
                        .filter(|v| v.is_finite()),
                    status: entry.map_or(Status::Missing, |e| e.status),
                }
            })
            .collect();
        row.sort_by(|a, b| {
            match (a.value, b.value) {
                (Some(a), Some(b)) => a.total_cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then(a.label.cmp(&b.label))
        });
        let mut previous = None;
        let mut rank = 0;
        for (i, candidate) in row.iter_mut().enumerate() {
            if let Some(value) = candidate.value {
                if previous != Some(value) {
                    rank = i + 1;
                }
                candidate.rank = Some(rank);
                previous = Some(value);
            }
        }
        images.push(RankedImage {
            name,
            candidates: row,
        });
    }
    let common: Vec<_> = images
        .iter()
        .filter(|image| {
            reference_names.contains(image.name.as_str())
                && image.candidates.iter().all(|c| c.value.is_some())
        })
        .collect();
    let complete = !reference_names.is_empty() && common.len() == reference_names.len();
    let mut overall: Vec<_> = labels
        .iter()
        .zip(&reports)
        .map(|(label, report)| {
            let rows: Vec<_> = common
                .iter()
                .flat_map(|image| image.candidates.iter().filter(|c| c.label == *label))
                .collect();
            CandidateRank {
                label: label.clone(),
                rank: None,
                mean_rank: (!rows.is_empty()).then(|| {
                    rows.iter().filter_map(|c| c.rank).sum::<usize>() as f64 / rows.len() as f64
                }),
                mean_metric: (!rows.is_empty())
                    .then(|| rows.iter().filter_map(|c| c.value).sum::<f64>() / rows.len() as f64),
                ranked_images: rows.len(),
                bit_identical: report
                    .entries
                    .iter()
                    .filter(|e| e.bit_identical == Some(true))
                    .count(),
                totals: report.totals,
                complete,
                report_json: crate::explain::absolute(&out.join(label).join(REPORT_FILE_NAME))
                    .display()
                    .to_string(),
                report_html: format!("{label}/index.html"),
            }
        })
        .collect();
    overall.sort_by(|a, b| {
        a.mean_rank
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b.mean_rank.unwrap_or(f64::INFINITY))
            .then(
                a.mean_metric
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&b.mean_metric.unwrap_or(f64::INFINITY)),
            )
            .then(a.label.cmp(&b.label))
    });
    let mut previous = None;
    let mut rank = 0;
    for (i, candidate) in overall.iter_mut().enumerate() {
        if complete {
            let key = (candidate.mean_rank, candidate.mean_metric);
            if previous != Some(key) {
                rank = i + 1;
            }
            candidate.rank = Some(rank);
            previous = Some(key);
        }
    }
    let result = RankReport {
        schema: RANK_SCHEMA.into(),
        verdict: if reports.iter().any(Report::is_regression) {
            "regression"
        } else {
            "pass"
        }
        .into(),
        metric,
        reference_dir: crate::explain::absolute(reference).display().to_string(),
        reference_images: reference_names.len(),
        common_images: common.len(),
        overall,
        images,
        report_json: crate::explain::absolute(&out.join(RANK_FILE))
            .display()
            .to_string(),
        index_html: crate::explain::absolute(&out.join("index.html"))
            .display()
            .to_string(),
        markdown: crate::explain::absolute(&out.join(RANK_MD))
            .display()
            .to_string(),
    };
    for (file, text) in [
        (RANK_FILE, serde_json::to_string_pretty(&result)?),
        (RANK_MD, result.markdown()),
    ] {
        let path = out.join(file);
        std::fs::write(&path, text).map_err(io_err(format!("writing {}", path.display())))?;
    }
    crate::render::render_rank_html(&result, out)?;
    std::fs::remove_file(&sentinel).map_err(io_err(format!("removing {}", sentinel.display())))?;
    Ok(result)
}
