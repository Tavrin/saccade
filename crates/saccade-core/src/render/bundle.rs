//! Portable measured cases and the opening human review summary.
use crate::evidence::canonical::Digest;
use crate::evidence::case::{
    ArtifactRef, Availability, EvidenceCase, Input, Measurement, Provenance, Scope, Validity,
    ValidityStatus,
};
use crate::evidence::{Artifact, Document};
use crate::{Error, Report, Result};
use std::collections::BTreeMap;
use std::path::Path;

fn contract(e: impl std::fmt::Display) -> Error {
    Error::Config(e.to_string())
}
fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(crate::paths::native(path), bytes)
        .map_err(crate::run::io_err(format!("writing {}", path.display())))
}

/// Builds a relocatable case from the immutable originals already in the bundle.
/// Display transforms never substitute for native input content.
pub fn measured_case(report: &Report, dir: &Path) -> Result<Option<EvidenceCase>> {
    let document = dir.join("evidence.json");
    let report_path = dir.join(crate::report::REPORT_FILE_NAME);
    if report.entries.is_empty() || !report_path.is_file() {
        return Ok(None);
    }
    let mut inputs = Vec::new();
    for entry in &report.entries {
        for (role, shown, hash) in [
            ("baseline", &entry.paths.baseline, &entry.baseline_sha256),
            ("capture", &entry.paths.capture, &entry.capture_sha256),
        ] {
            let Some(hash) = hash else {
                continue;
            };
            let mut candidates = Vec::new();
            if let Some(shown) = shown {
                candidates.push(dir.join(shown));
            }
            let originals = dir.join(format!("images/{}.d", entry.name));
            if originals.is_dir() {
                for file in std::fs::read_dir(&originals)
                    .map_err(crate::run::io_err("reading bundled originals".into()))?
                {
                    let file =
                        file.map_err(crate::run::io_err("reading bundled original".into()))?;
                    let stem = if role == "baseline" {
                        "baseline"
                    } else {
                        "capture"
                    };
                    if file
                        .file_name()
                        .to_string_lossy()
                        .starts_with(&format!("{stem}.orig."))
                    {
                        candidates.push(file.path());
                    }
                }
            }
            let expected = Digest::parse(format!("sha256:{hash}")).map_err(contract)?;
            let mut content = None;
            for path in candidates {
                if path.is_file() {
                    let reference =
                        ArtifactRef::from_file(&path, &document, false).map_err(contract)?;
                    if reference.sha256 == expected {
                        content = Some(reference);
                        break;
                    }
                }
            }
            let Some(content) = content else {
                return Ok(None);
            };
            let mut sidecars = Vec::new();
            if role == "capture" {
                for attribution in &entry.object_attribution {
                    for (path, hash) in [
                        (&attribution.image_path, &attribution.image_sha256),
                        (&attribution.legend_path, &attribution.legend_sha256),
                    ] {
                        let source = dir.join(path);
                        if !source.is_file() {
                            return Ok(None);
                        }
                        let reference =
                            ArtifactRef::from_file(&source, &document, false).map_err(contract)?;
                        if reference.sha256
                            != Digest::parse(format!("sha256:{hash}")).map_err(contract)?
                        {
                            return Ok(None);
                        }
                        sidecars.push(reference);
                    }
                }
            }
            inputs.push(Input {
                id: format!("{role}:{}", entry.name),
                content,
                sidecars,
                native_samples: Availability::missing(
                    "native samples are described by the measured report",
                ),
                capture: Availability::missing("capture context was not independently verified"),
                build: Availability::missing("build identity was not independently verified"),
                provenance: Provenance::default(),
            });
        }
    }
    if inputs.is_empty() {
        return Ok(None);
    }
    let scope = Scope {
        entries: report.entries.iter().map(|e| e.name.clone()).collect(),
        exclusions: report.config.ignore.clone(),
    };
    let validity = report.capture_validity();
    let mut case = EvidenceCase {
        case_id: Digest::of_bytes(b""),
        inputs,
        measurement: Measurement::from_report(
            &report_path,
            &document,
            scope.entries.clone(),
            "saccade-report.v1".into(),
            "saccade-config.v1".into(),
        )
        .map_err(contract)?,
        scope,
        effective_config: BTreeMap::from([(
            "report_config".into(),
            serde_json::to_value(&report.config)?,
        )]),
        calibration: Availability::missing("no decision calibration supplied"),
        validity: Validity {
            status: match validity.status {
                crate::meta::Validity::Valid => ValidityStatus::Valid,
                crate::meta::Validity::Invalid => ValidityStatus::Invalid,
                crate::meta::Validity::Unknown => ValidityStatus::Unknown,
            },
            reasons: validity.reasons,
        },
        facts: Vec::new(),
        intent: Availability::missing("no structured intent supplied with this measured report"),
        requests: Vec::new(),
        proposals: Vec::new(),
        human_decisions: Vec::new(),
        next_actions: Vec::new(),
        limits: vec![
            "Native sample equality does not establish capture validity or qualified performance."
                .into(),
            "Threshold passes do not establish perceptual invisibility or renderer correctness."
                .into(),
        ],
        provenance: Provenance::default(),
    };
    case.refresh_id().map_err(contract)?;
    case.validate().map_err(contract)?;
    Ok(Some(case))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn summary(report: &Report, case: Option<&EvidenceCase>) -> Result<String> {
    let identity = report.config.mode == crate::report::Mode::Identity;
    let validity = report.capture_validity();
    let claim = if identity {
        "Exact native decoded-sample equality across the selected scope"
    } else {
        "Image change against the declared numerical thresholds"
    };
    let changes = if identity {
        super::identity_headline(report).unwrap_or_else(|| "Native equality is unresolved.".into())
    } else {
        format!(
            "{} passing; {} failing; {} errors; {} missing; {} new",
            report.totals.pass,
            report.totals.fail,
            report.totals.error,
            report.totals.missing,
            report.totals.new
        )
    };
    let configuration = if identity {
        format!(
            "{} vs {}; exact native decoded-sample equality for supplied captures: {}. Capture comparability and performance are separate findings.",
            report.config.labels.baseline,
            report.config.labels.capture,
            report
                .entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "{} vs {}; mode {:?}; metric {:?}; threshold {}; {} pixels per degree",
            report.config.labels.baseline,
            report.config.labels.capture,
            report.config.mode,
            report.config.default_metric,
            report.config.default_threshold,
            report.config.pixels_per_degree
        )
    };
    let criteria = if identity {
        "All selected pairs must exist and decode with equal native samples, dimensions, channel interpretation and sample type. No tolerance or mask establishes identity."
    } else {
        "Per-entry numerical criteria appear below. Missing/error entries are unresolved evidence."
    };
    let mut rows = vec![
        ("Claim", claim.to_owned()),
        ("Captures and configuration", configuration),
        (
            "Comparison validity",
            format!(
                "{:?}: {} recorded validity findings. {}",
                validity.status,
                validity.reasons.len(),
                validity
                    .reasons
                    .first()
                    .map_or("No missing capture checks recorded.", String::as_str)
            ),
        ),
        ("What changed", changes),
        ("Acceptance criteria", criteria.into()),
        (
            "Next action",
            if report.totals.fail + report.totals.error + report.totals.missing + report.totals.new
                > 0
            {
                "Inspect the failing entries and numbered hotspots; resolve missing evidence before approval.".into()
            } else {
                "Review capture validity and performance qualification before making a broader claim.".into()
            },
        ),
    ];
    if let Some(case) = case {
        if let Some(intent) = case.intent.value() {
            rows.push((
                "Declared intent",
                format!(
                    "{}; criteria: {}",
                    intent.objective,
                    serde_json::to_string(&intent.criteria)?
                ),
            ));
        }
        rows.push((
            "Scope",
            format!(
                "{}; exclusions: {}",
                case.scope.entries.join(", "),
                case.scope.exclusions.join(", ")
            ),
        ));
        rows.push((
            "Model proposals",
            if case.proposals.is_empty() {
                "None recorded.".into()
            } else {
                serde_json::to_string(&case.proposals)?
            },
        ));
        rows.push(("Human disposition", if case.human_decisions.is_empty() { "No human decision recorded. Offline controls export proposals; workbench attestation requires a live scoped session.".into() } else { serde_json::to_string(&case.human_decisions)? }));
        rows.push((
            "Exact input hashes",
            case.inputs
                .iter()
                .map(|i| format!("{}: {}", i.id, i.content.sha256.as_str()))
                .collect::<Vec<_>>()
                .join("; "),
        ));
        rows.push(("What remains unproven", case.limits.join(" ")));
    } else {
        rows.push(("Model proposals", "None recorded.".into()));
        rows.push((
            "Human disposition",
            "No attested human decision recorded.".into(),
        ));
        rows.push(("What remains unproven", "Missing repeat noise stays unknown. Image equality does not qualify performance. No model or threshold result authenticates a human or proves renderer correctness.".into()));
    }
    let next = rows.remove(5);
    rows.insert(1, next);
    let configuration = rows.remove(2);
    rows.insert(4, configuration);
    rows.push(("Validity findings", validity.reasons.join("; ")));
    let mut fields = rows
        .into_iter()
        .map(|(k, v)| format!("<div><dt>{}</dt><dd>{}</dd></div>", escape(k), escape(&v)))
        .collect::<Vec<_>>();
    let detail = fields.split_off(4).join("");
    let fields = fields.join("");
    let (verdict_class, verdict) = verdict_line(report);
    Ok(format!(
        "<section class=\"review-summary\" aria-label=\"Evidence and review\"><p class=\"verdict {verdict_class}\" role=\"status\">{}</p><h2>Evidence and review</h2><dl>{fields}</dl><details><summary>Configuration, criteria, scope and review records</summary><dl>{detail}</dl></details><p><a href=\"saccade-report.v1.json\">Measurement</a> · <a href=\"evidence.json\">Evidence case</a> · <a href=\"decisions.json\">Recorded decisions</a></p></section>",
        escape(&verdict)
    ))
}

/// The report's one-line result and its status class, shown first on the page.
fn verdict_line(report: &Report) -> (&'static str, String) {
    let t = &report.totals;
    let class = if report.is_empty_run() {
        "v-empty"
    } else if report.is_regression() {
        "v-fail"
    } else {
        "v-pass"
    };
    if let Some(headline) = super::identity_headline(report) {
        return (class, headline);
    }
    let images = if t.total == 1 { "image" } else { "images" };
    let problems: Vec<String> = [
        (t.fail, "fail"),
        (t.error, "error"),
        (t.missing, "missing"),
        (t.new, "new"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    let text = if report.is_empty_run() {
        format!(
            "Nothing compared: no image exists in both directories ({} {images} found)",
            t.total
        )
    } else if report.is_regression() {
        format!(
            "Regression: {} of {} {images}",
            problems.join(", "),
            t.total
        )
    } else if problems.is_empty() {
        format!("No regression: {} of {} {images} passed", t.pass, t.total)
    } else {
        format!(
            "No regression: {} passed; {} not gated",
            t.pass,
            problems.join(", ")
        )
    };
    (class, text)
}

pub(crate) fn prepare(report: &Report, dir: &Path) -> Result<Option<EvidenceCase>> {
    std::fs::create_dir_all(dir.join("assets"))
        .map_err(crate::run::io_err("creating report assets".into()))?;
    write(&dir.join(".saccade-run"), b"saccade report bundle\n")?;
    let path = dir.join("evidence.json");
    let case = if path.is_file() {
        let doc = Document::read(&path).map_err(contract)?;
        let Artifact::Case(case) = doc.artifact else {
            return Err(contract("bundle evidence must be a case"));
        };
        let report_bytes = std::fs::read(dir.join(crate::report::REPORT_FILE_NAME))
            .map_err(crate::run::io_err("reading measured report".into()))?;
        if case.measurement.semantic_sha256
            != Measurement::report_identity(report).map_err(contract)?
            || case.measurement.report.sha256 != Digest::of_bytes(&report_bytes)
        {
            write(&dir.join("decisions.json"), b"[]\n")?;
            measured_case(report, dir)?
        } else {
            Some(*case)
        }
    } else {
        measured_case(report, dir)?
    };
    if let Some(case) = &case {
        write(
            &path,
            serde_json::to_vec_pretty(&Document::new(Artifact::Case(Box::new(case.clone()))))?,
        )?;
    }
    if !dir.join("decisions.json").exists() {
        write(&dir.join("decisions.json"), b"[]\n")?;
    }
    Ok(case)
}
