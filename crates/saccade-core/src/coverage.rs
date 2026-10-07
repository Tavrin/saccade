//! Declared-case coverage, variant grouping and reference health over Lane C manifests.
use crate::{Error, Result, manifest, report_links};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

/// Successor of the output manifest, preserving every v1 field.
pub const MANIFEST_SCHEMA: &str = "saccade-manifest.v2";
/// Portable declaration input for `manifest build --cases`.
pub const CASES_SCHEMA: &str = "saccade-cases.v1";
/// Grouped coverage and health output.
pub const REPORT_SCHEMA: &str = "saccade-coverage.v1";
const MAX_BYTES: u64 = 64 << 20;
const MAX_CASES: usize = 20_000;

/// An encoded input recorded by path and content hash, as in Lane C anchors.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    /// Path relative to the output manifest (not the declaration input).
    pub path: String,
    /// SHA-256 of encoded bytes.
    pub sha256: String,
}
/// One measured entry resolved through the existing manifest report-ID index.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementRef {
    /// Content-addressed report identity.
    pub report_id: String,
    /// Exact comparison entry name; never guessed from case ID or filename.
    pub entry: String,
}
/// A declared variant; absent files and absent comparisons are valid declarations.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Stable identity independent of paths.
    pub case_id: String,
    /// One value for every declared axis.
    pub variants: BTreeMap<String, String>,
    /// Whether incomplete coverage fails the coverage gate.
    pub required: bool,
    /// Expected baseline, including a path/hash for an unavailable input if known.
    pub baseline: Option<FileRef>,
    /// Expected current capture. None means explicitly absent on this side.
    pub capture: Option<FileRef>,
    /// Human-approved reference declaration; passing never populates it.
    pub approved_anchor: Option<FileRef>,
    /// Most recent passing reference declaration; a separate role from approval.
    pub last_good: Option<FileRef>,
    /// Explicit approval time, if known; never inferred from a report timestamp.
    pub approved_at_unix: Option<u64>,
    /// Optional producer refusal reason (no semantic inference).
    pub refusal: Option<String>,
    /// Latest baseline-relative comparison.
    pub latest: Option<MeasurementRef>,
    /// Current capture relative to the approved anchor.
    pub anchor_comparison: Option<MeasurementRef>,
    /// Current capture relative to last-good.
    pub last_good_comparison: Option<MeasurementRef>,
}
/// Explicit variant space and cases. Only cases, not a Cartesian product, are expected.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// Must be [`CASES_SCHEMA`].
    pub schema: String,
    /// Allowed values of generic axes such as viewport, language, theme or tier.
    pub axes: BTreeMap<String, Vec<String>>,
    /// All expected cases, including variants absent on both sides.
    pub cases: Vec<Case>,
}
/// Verified pair evidence, independent of coverage and reference health.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// measured, missing, stale, refused, unbound or unavailable.
    pub state: String,
    /// pass/fail only for verified, input-bound comparisons.
    pub verdict: Option<String>,
    /// Report metric name when measured.
    pub metric: Option<String>,
    /// Report metric value when measured.
    pub value: Option<f64>,
    /// Declared report-ID link, even when unavailable.
    pub report_id: Option<String>,
    /// Report path relative to the input manifest, for audit/navigation.
    pub report_path: Option<String>,
    /// Recorded run time, if known (not approval).
    pub run_at_unix: Option<u64>,
}
/// One explicit row in every grouped view.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// Original declaration, retaining all roles and absences.
    pub case: Case,
    /// present, missing, stale or unavailable for baseline bytes.
    pub baseline_state: String,
    /// present, missing, stale or unavailable for capture bytes.
    pub capture_state: String,
    /// measured, missing, stale, refused, unbound or unavailable.
    pub coverage: String,
    /// Latest ordinary baseline-relative evidence.
    pub latest: Measurement,
    /// Cumulative drift evidence, independent of last-good.
    pub approved_anchor: Measurement,
    /// Operational continuity evidence, independent of approved anchor.
    pub last_good: Measurement,
    /// Explicit health findings. Empty means no finding under the recorded policy.
    pub health: Vec<String>,
}
/// Counts over exactly the declared cases in a group.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Counts {
    /// Declared cases.
    pub expected: usize,
    /// Both current inputs exist with recorded hashes.
    pub captured: usize,
    /// Input-bound measurements, including measured failures.
    pub measured: usize,
    /// Explicit producer refusals or refused measurements.
    pub refused: usize,
    /// Outcome counts, summing to expected.
    pub outcomes: BTreeMap<String, usize>,
}
/// A group of declared variants.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Values of selected axes; an empty map is the all-cases group.
    pub variants: BTreeMap<String, String>,
    /// Stable case IDs in this group.
    pub case_ids: Vec<String>,
    /// Coverage accounting for this group.
    pub counts: Counts,
}
/// Audit-ready grouped view with reproducible freshness policy.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// [`REPORT_SCHEMA`].
    pub schema: String,
    /// SHA-256 of the exact input manifest bytes.
    pub manifest_sha256: String,
    /// Observation time supplied by the caller.
    pub now_unix: u64,
    /// Maximum age for runs and known approval times.
    pub max_age_seconds: u64,
    /// Selected axes, defaulting to every declared axis.
    pub group_by: Vec<String>,
    /// complete or incomplete for required cases; domain coverage remains unknown.
    pub coverage: String,
    /// All-case accounting.
    pub counts: Counts,
    /// Every declared case, in stable case-ID order.
    pub rows: Vec<Row>,
    /// Stable groups. Missing cases remain members.
    pub groups: Vec<Group>,
}

fn read(path: &Path) -> Result<Vec<u8>> {
    let file =
        std::fs::File::open(path).map_err(crate::run::io_err("reading coverage input".into()))?;
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(crate::run::io_err("reading coverage input".into()))?;
    if data.len() as u64 > MAX_BYTES {
        return Err(Error::Config("coverage input exceeds 64 MiB".into()));
    }
    Ok(data)
}
fn sha(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control)
}

impl Declaration {
    /// Refuse ambiguous IDs, undeclared variants and malformed references.
    pub fn validate(&self) -> Result<()> {
        let mut ids = BTreeSet::new();
        let valid = self.schema == CASES_SCHEMA
            && !self.cases.is_empty()
            && self.cases.len() <= MAX_CASES
            && self.axes.len() <= 32
            && self.axes.iter().all(|(key, values)| {
                text(key)
                    && !values.is_empty()
                    && values.len() <= MAX_CASES
                    && values.iter().all(|v| text(v))
                    && values.iter().collect::<BTreeSet<_>>().len() == values.len()
            })
            && self.cases.iter().all(|c| {
                text(&c.case_id)
                    && ids.insert(&c.case_id)
                    && c.variants.len() == self.axes.len()
                    && c.variants
                        .iter()
                        .all(|(k, v)| self.axes.get(k).is_some_and(|values| values.contains(v)))
                    && [&c.baseline, &c.capture, &c.approved_anchor, &c.last_good]
                        .into_iter()
                        .flatten()
                        .all(|r| text(&r.path) && valid_sha(&r.sha256))
                    && [&c.latest, &c.anchor_comparison, &c.last_good_comparison]
                        .into_iter()
                        .flatten()
                        .all(|r| {
                            text(&r.entry)
                                && r.report_id.strip_prefix("sha256:").is_some_and(valid_sha)
                        })
                    && c.refusal.as_ref().is_none_or(|s| text(s))
                    && (c.approved_at_unix.is_none() || c.approved_anchor.is_some())
            });
        if !valid {
            return Err(Error::Config("invalid case declaration: schema, duplicate/empty IDs, axes, reference hashes or approval time".into()));
        }
        Ok(())
    }
}
/// Read and validate a bounded case declaration.
pub fn read_declaration(path: &Path) -> Result<Declaration> {
    let declaration: Declaration = serde_json::from_slice(&read(path)?)?;
    declaration.validate()?;
    Ok(declaration)
}
/// Build a v2 manifest using the existing output discovery and report index.
/// Paths inside `declaration` resolve from `dir`, regardless of its input location.
pub fn write_manifest(
    dir: &Path,
    anchors: manifest::Anchors<'_>,
    declaration: &Declaration,
) -> Result<(std::path::PathBuf, Value)> {
    declaration.validate()?;
    let mut value = manifest::build(dir, anchors)?;
    value["schema"] = MANIFEST_SCHEMA.into();
    value["axes"] = serde_json::to_value(&declaration.axes)?;
    value["cases"] = serde_json::to_value(&declaration.cases)?;
    let path = dir.join(manifest::MANIFEST_FILE);
    // Explicit successor upgrade only. Never replace a v2 declaration with v1.
    if path.is_file()
        && serde_json::from_slice::<Value>(&read(&path)?)?["schema"] == manifest::MANIFEST_SCHEMA
    {
        manifest::write_owned(&path, manifest::MANIFEST_SCHEMA, &value)?;
    } else {
        manifest::write_owned(&path, MANIFEST_SCHEMA, &value)?;
    }
    Ok((path, value))
}
fn file_state(file: Option<&FileRef>, document: &Path) -> &'static str {
    let Some(file) = file else {
        return "missing";
    };
    match read(&crate::paths::resolve(&file.path, document)) {
        Ok(data) if sha(&data) == file.sha256 => "present",
        Ok(_) => "stale",
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => "missing",
        Err(_) => "unavailable",
    }
}
fn measurement(
    reference: Option<&MeasurementRef>,
    baseline: Option<&FileRef>,
    capture: Option<&FileRef>,
    manifest: &Value,
    document: &Path,
) -> Measurement {
    let mut out = Measurement {
        state: "missing".into(),
        verdict: None,
        metric: None,
        value: None,
        report_id: reference.map(|r| r.report_id.clone()),
        report_path: None,
        run_at_unix: None,
    };
    let Some(reference) = reference else {
        return out;
    };
    let indexed: Vec<_> = manifest["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["report_id"] == reference.report_id)
        .collect();
    if indexed.len() != 1 {
        out.state = "unavailable".into();
        return out;
    }
    let row = indexed[0];
    let (Some(path), Some(hash)) = (row["path"].as_str(), row["sha256"].as_str()) else {
        out.state = "unavailable".into();
        return out;
    };
    out.report_path = Some(path.into());
    if row["local"] != true {
        out.state = "unavailable".into();
        return out;
    }
    let data = match read(&crate::paths::resolve(path, document)) {
        Ok(data) => data,
        Err(_) => return out,
    };
    if sha(&data) != hash {
        out.state = "stale".into();
        return out;
    }
    let Ok(value) = serde_json::from_slice::<Value>(&data) else {
        out.state = "unavailable".into();
        return out;
    };
    if !matches!(
        value["schema"].as_str(),
        Some("saccade-report.v1" | "saccade-report.v2")
    ) || value["report_id"] != reference.report_id
        || !report_links::decorate(&value).is_ok_and(|v| v["report_id"] == reference.report_id)
    {
        out.state = "unbound".into();
        return out;
    }
    if report_links::decode::<crate::Report>(&data).is_err() {
        out.state = "unavailable".into();
        return out;
    }
    let entries: Vec<_> = value["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["name"] == reference.entry)
        .collect();
    if entries.len() != 1 {
        out.state = "unavailable".into();
        return out;
    }
    let entry = entries[0];
    if !matches!(entry["status"].as_str(), Some("pass" | "fail"))
        || entry["capture_validity"]["status"] == "invalid"
    {
        out.state = "refused".into();
        return out;
    }
    if baseline.is_none_or(|r| entry["baseline_sha256"] != r.sha256)
        || capture.is_none_or(|r| entry["capture_sha256"] != r.sha256)
        || file_state(baseline, document) != "present"
        || file_state(capture, document) != "present"
    {
        out.state = "unbound".into();
        return out;
    }
    let Some(v) = entry["value"].as_f64().filter(|v| v.is_finite()) else {
        out.state = "unavailable".into();
        return out;
    };
    out.state = "measured".into();
    out.verdict = entry["status"].as_str().map(str::to_owned);
    out.metric = entry["metric_used"].as_str().map(str::to_owned);
    out.value = Some(v);
    out.run_at_unix = value["generated_at_unix"].as_u64();
    out
}
fn counts<'a>(rows: impl Iterator<Item = &'a Row>) -> Counts {
    let mut counts = Counts::default();
    for row in rows {
        counts.expected += 1;
        counts.captured +=
            usize::from(row.baseline_state == "present" && row.capture_state == "present");
        counts.measured += usize::from(row.coverage == "measured");
        counts.refused += usize::from(row.coverage == "refused");
        *counts.outcomes.entry(row.coverage.clone()).or_default() += 1;
    }
    counts
}
/// Produce views without writing or modifying inputs. Unavailable references remain rows.
pub fn analyze(
    path: &Path,
    group_by: &[String],
    now_unix: u64,
    max_age_seconds: u64,
) -> Result<Report> {
    let bytes = read(path)?;
    let manifest: Value = serde_json::from_slice(&bytes)?;
    if manifest["schema"] != MANIFEST_SCHEMA {
        return Err(Error::Config(format!(
            "views require {MANIFEST_SCHEMA}; use manifest build --cases"
        )));
    }
    let declaration = Declaration {
        schema: CASES_SCHEMA.into(),
        axes: serde_json::from_value(manifest["axes"].clone())?,
        cases: serde_json::from_value(manifest["cases"].clone())?,
    };
    declaration.validate()?;
    let group_by = if group_by.is_empty() {
        declaration.axes.keys().cloned().collect::<Vec<_>>()
    } else {
        group_by.to_vec()
    };
    if group_by.iter().collect::<BTreeSet<_>>().len() != group_by.len()
        || group_by.iter().any(|k| !declaration.axes.contains_key(k))
    {
        return Err(Error::Config(
            "group-by requires unique declared axes".into(),
        ));
    }
    let mut rows = Vec::new();
    for case in declaration.cases {
        let baseline_state = file_state(case.baseline.as_ref(), path).to_owned();
        let capture_state = file_state(case.capture.as_ref(), path).to_owned();
        let latest = measurement(
            case.latest.as_ref(),
            case.baseline.as_ref(),
            case.capture.as_ref(),
            &manifest,
            path,
        );
        let approved_anchor = measurement(
            case.anchor_comparison.as_ref(),
            case.approved_anchor.as_ref(),
            case.capture.as_ref(),
            &manifest,
            path,
        );
        let last_good = measurement(
            case.last_good_comparison.as_ref(),
            case.last_good.as_ref(),
            case.capture.as_ref(),
            &manifest,
            path,
        );
        let coverage = if case.refusal.is_some() {
            "refused"
        } else if baseline_state == "missing" || capture_state == "missing" {
            "missing"
        } else if baseline_state == "stale" || capture_state == "stale" {
            "stale"
        } else if baseline_state != "present" || capture_state != "present" {
            "unavailable"
        } else {
            &latest.state
        }
        .to_owned();
        let mut health = Vec::new();
        if baseline_state != "present" {
            health.push(format!("baseline_{baseline_state}"));
        }
        if case.approved_anchor.is_none() {
            health.push("never_approved".into());
        } else {
            let state = file_state(case.approved_anchor.as_ref(), path);
            if state != "present" {
                health.push(format!("approved_anchor_{state}"));
            }
            match case.approved_at_unix {
                None => health.push("approval_age_unknown".into()),
                Some(t) if t > now_unix => health.push("approval_time_in_future".into()),
                Some(t) if now_unix - t > max_age_seconds => {
                    health.push("approved_anchor_old".into())
                }
                Some(_) => {}
            }
            if latest.state == "measured" && approved_anchor.state != "measured" {
                health.push("anchor_comparison_unavailable".into());
            }
        }
        if case.last_good.is_some() {
            let state = file_state(case.last_good.as_ref(), path);
            if state != "present" {
                health.push(format!("last_good_{state}"));
            }
        }
        match latest.run_at_unix {
            None => health.push("no_recent_run".into()),
            Some(t) if t > now_unix => health.push("run_time_in_future".into()),
            Some(t) if now_unix - t > max_age_seconds => health.push("no_recent_run".into()),
            Some(_) => {}
        }
        rows.push(Row {
            case,
            baseline_state,
            capture_state,
            coverage,
            latest,
            approved_anchor,
            last_good,
            health,
        });
    }
    rows.sort_by(|a, b| a.case.case_id.cmp(&b.case.case_id));
    let mut grouped: BTreeMap<BTreeMap<String, String>, Vec<&Row>> = BTreeMap::new();
    for row in &rows {
        let variants = group_by
            .iter()
            .map(|k| (k.clone(), row.case.variants[k].clone()))
            .collect();
        grouped.entry(variants).or_default().push(row);
    }
    let groups = grouped
        .into_iter()
        .map(|(variants, rows)| Group {
            variants,
            case_ids: rows.iter().map(|r| r.case.case_id.clone()).collect(),
            counts: counts(rows.into_iter()),
        })
        .collect();
    Ok(Report {
        schema: REPORT_SCHEMA.into(),
        manifest_sha256: sha(&bytes),
        now_unix,
        max_age_seconds,
        group_by,
        coverage: if rows
            .iter()
            .all(|r| !r.case.required || r.coverage == "measured")
        {
            "complete"
        } else {
            "incomplete"
        }
        .into(),
        counts: counts(rows.iter()),
        rows,
        groups,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, Declaration) {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("base.png"), b"base").unwrap();
        std::fs::write(tmp.path().join("cap.png"), b"capture").unwrap();
        std::fs::write(tmp.path().join("anchor.png"), b"anchor").unwrap();
        let base = FileRef {
            path: "base.png".into(),
            sha256: sha(b"base"),
        };
        let cap = FileRef {
            path: "cap.png".into(),
            sha256: sha(b"capture"),
        };
        let anchor = FileRef {
            path: "anchor.png".into(),
            sha256: sha(b"anchor"),
        };
        let scope = report_links::scope(vec![], None).unwrap();
        let mut pair = json!({"schema":"saccade-report.v1", "tool_version":"fixture", "generated_at_unix":100,
            "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
            "totals":{"total":1,"pass":0,"fail":1,"error":0,"missing":0,"new":0},
            "entries":[{"name":"pair", "status":"fail", "baseline_sha256":base.sha256, "capture_sha256":cap.sha256, "metric_used":"mean", "value":0.2, "threshold":0.01, "paths":{}}]});
        let latest = report_links::write(&tmp.path().join("latest.json"), &pair).unwrap();
        pair["entries"][0]["baseline_sha256"] = anchor.sha256.clone().into();
        let drift = report_links::write(&tmp.path().join("drift.json"), &pair).unwrap();
        pair["entries"][0]["baseline_sha256"] = base.sha256.clone().into();
        pair["entries"][0]["status"] = "pass".into();
        pair["totals"]["pass"] = 1.into();
        pair["totals"]["fail"] = 0.into();
        pair["entries"][0]["value"] = 0.0.into();
        let continuity = report_links::write(&tmp.path().join("continuity.json"), &pair).unwrap();
        drop(scope);
        let reference = |v: &Value| MeasurementRef {
            report_id: v["report_id"].as_str().unwrap().into(),
            entry: "pair".into(),
        };
        let declaration = Declaration {
            schema: CASES_SCHEMA.into(),
            axes: BTreeMap::from([("theme".into(), vec!["light".into(), "dark".into()])]),
            cases: vec![Case {
                case_id: "declared".into(),
                variants: BTreeMap::from([("theme".into(), "light".into())]),
                required: true,
                baseline: Some(base.clone()),
                capture: Some(cap),
                approved_anchor: Some(anchor),
                last_good: Some(base),
                approved_at_unix: Some(99),
                refusal: None,
                latest: Some(reference(&latest)),
                anchor_comparison: Some(reference(&drift)),
                last_good_comparison: Some(reference(&continuity)),
            }],
        };
        (tmp, declaration)
    }
    fn write(tmp: &Path, d: &Declaration) -> std::path::PathBuf {
        write_manifest(tmp, manifest::Anchors::default(), d)
            .unwrap()
            .0
    }

    #[test]
    fn missing_both_sides_and_refusals_remain_grouped_rows_while_failures_are_measured() {
        let (tmp, mut d) = fixture();
        let mut missing = d.cases[0].clone();
        missing.case_id = "absent-both".into();
        missing.variants.insert("theme".into(), "dark".into());
        missing.baseline = None;
        missing.capture = None;
        missing.latest = None;
        missing.approved_anchor = None;
        missing.approved_at_unix = None;
        let mut refused = missing.clone();
        refused.case_id = "refused".into();
        refused.refusal = Some("acquisition failed".into());
        d.cases.extend([missing, refused]);
        let report = analyze(&write(tmp.path(), &d), &["theme".into()], 110, 20).unwrap();
        assert_eq!(
            (
                report.counts.expected,
                report.counts.captured,
                report.counts.measured,
                report.counts.refused
            ),
            (3, 1, 1, 1)
        );
        assert_eq!(report.counts.outcomes.values().sum::<usize>(), 3);
        assert_eq!(report.groups.len(), 2);
        assert_eq!(report.groups[0].counts.expected, 2);
        assert_eq!(report.coverage, "incomplete");
        assert_eq!(report.rows[0].coverage, "missing");
        assert!(report.rows[0].health.contains(&"never_approved".into()));
        assert!(report.rows[0].health.contains(&"no_recent_run".into()));
        assert!(report.rows[1].latest.verdict.as_deref() == Some("fail"));
        d.cases.truncate(1);
        assert_eq!(
            analyze(&write(tmp.path(), &d), &[], 110, 20)
                .unwrap()
                .coverage,
            "complete"
        );
    }

    #[test]
    fn approval_and_last_good_are_separate_and_health_uses_explicit_times_and_hashes() {
        let (tmp, d) = fixture();
        let path = write(tmp.path(), &d);
        let r = analyze(&path, &[], 110, 20).unwrap();
        assert_eq!(r.rows[0].approved_anchor.verdict.as_deref(), Some("fail"));
        assert_eq!(r.rows[0].last_good.verdict.as_deref(), Some("pass"));
        assert!(r.rows[0].health.is_empty());
        let old = analyze(&path, &[], 140, 20).unwrap();
        assert!(old.rows[0].health.contains(&"approved_anchor_old".into()));
        assert!(old.rows[0].health.contains(&"no_recent_run".into()));
        let future = analyze(&path, &[], 90, 20).unwrap();
        assert!(future.rows[0].health.contains(&"run_time_in_future".into()));
        std::fs::write(tmp.path().join("anchor.png"), b"moving anchor").unwrap();
        let stale = analyze(&path, &[], 110, 20).unwrap();
        assert!(
            stale.rows[0]
                .health
                .contains(&"approved_anchor_stale".into())
        );
        assert!(stale.rows[0].approved_anchor.verdict.is_none());
        assert_eq!(stale.rows[0].last_good.verdict.as_deref(), Some("pass"));
        assert!(
            manifest::verify(&path)
                .unwrap()
                .iter()
                .any(|f| f["code"] == "stale_link")
        );
    }

    #[test]
    fn changed_report_and_unbound_inputs_never_become_measurements_or_recent_runs() {
        let (tmp, mut d) = fixture();
        let path = write(tmp.path(), &d);
        std::fs::write(tmp.path().join("latest.json"), b"{}").unwrap();
        let stale = analyze(&path, &[], 110, 20).unwrap();
        assert_eq!(stale.rows[0].latest.state, "stale");
        assert_eq!(stale.counts.measured, 0);
        assert!(stale.rows[0].health.contains(&"no_recent_run".into()));
        d.cases[0].latest = d.cases[0].anchor_comparison.clone();
        let unbound = analyze(&write(tmp.path(), &d), &[], 110, 20).unwrap();
        assert_eq!(unbound.rows[0].latest.state, "unbound");
        std::fs::remove_file(tmp.path().join("base.png")).unwrap();
        assert!(
            analyze(&path, &[], 110, 20).unwrap().rows[0]
                .health
                .contains(&"baseline_missing".into())
        );
    }

    #[test]
    fn duplicate_ids_unknown_axes_invalid_hashes_and_empty_declarations_are_refused() {
        let (tmp, mut d) = fixture();
        d.cases.push(d.cases[0].clone());
        assert!(d.validate().is_err());
        d.cases.pop();
        let path = write(tmp.path(), &d);
        assert!(analyze(&path, &["undeclared".into()], 110, 20).is_err());
        assert!(analyze(&path, &["theme".into(), "theme".into()], 110, 20).is_err());
        d.cases[0].variants.insert("theme".into(), "unknown".into());
        assert!(d.validate().is_err());
        d.cases[0].variants.insert("theme".into(), "light".into());
        d.cases[0].baseline.as_mut().unwrap().sha256 = "invalid".into();
        assert!(d.validate().is_err());
        d.cases.clear();
        assert!(d.validate().is_err());
    }

    #[test]
    fn successor_upgrade_keeps_links_and_refuses_lossy_v1_rebuild() {
        let (tmp, d) = fixture();
        manifest::write(tmp.path(), manifest::Anchors::default()).unwrap();
        let path = write(tmp.path(), &d);
        assert!(manifest::verify(&path).unwrap().is_empty());
        let value: Value = serde_json::from_slice(&read(&path).unwrap()).unwrap();
        let link = tmp.path().join("link.json");
        manifest::link(
            tmp.path(),
            value["reports"][0]["report_id"].as_str().unwrap(),
            &link,
        )
        .unwrap();
        assert!(manifest::verify(&link).unwrap().is_empty());
        assert!(manifest::write(tmp.path(), manifest::Anchors::default()).is_err());
        assert_eq!(
            serde_json::from_slice::<Value>(&read(&path).unwrap()).unwrap()["cases"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
