//! Stable-case capture coverage, independent of measurement verdicts.
use crate::{Error, Report, Result, Status};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One required or advisory suite case, with its former/expected filename.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Stable producer case ID, separate from a filename.
    pub case_id: String,
    /// Expected comparison entry name.
    pub entry: String,
    /// Whether absence prevents complete required coverage.
    pub required: bool,
}
/// Outcome recorded by a capture producer, including unsuccessful attempts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supplied {
    /// Stable producer case ID.
    pub case_id: String,
    /// Actual comparison entry name; None if no image was produced.
    pub entry: Option<String>,
    /// captured, skipped, quarantined, stale, missing or unusable.
    pub state: String,
    /// Hash of actual encoded capture; required for compared coverage.
    pub capture_sha256: Option<String>,
}
/// Expected suite and every attempted/supplied capture.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be saccade-inventory.v1.
    pub schema: String,
    /// Required/advisory cases, including passes.
    pub expected: Vec<Expected>,
    /// Capture attempts; duplicate identities are retained as coverage failures.
    pub supplied: Vec<Supplied>,
}
/// One unique expected case, assigned exactly one accounting outcome.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    /// Stable ID.
    pub case_id: String,
    /// Whether required for completeness.
    pub required: bool,
    /// Exactly one of compared, missing, unusable, uncompared, skipped, quarantined, stale, duplicate.
    pub outcome: String,
    /// Expected filename.
    pub expected_entry: String,
    /// Supplied filename, if any.
    pub supplied_entry: Option<String>,
    /// Same stable ID with a changed filename. Hash equality never infers a rename.
    pub renamed: bool,
    /// Measurement verdict, kept separate from coverage.
    pub measurement: Option<Status>,
}
/// Exact suite accounting and explicit anomalies.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// saccade-inventory-report.v1.
    pub schema: String,
    /// SHA-256 of exact manifest bytes, assigned by the caller.
    pub manifest_sha256: String,
    /// SHA-256 of exact report bytes, assigned by the caller.
    pub report_sha256: String,
    /// Number of unique expected cases, not raw duplicate declarations.
    pub expected: usize,
    /// Number of unique expected cases declaring a captured artifact.
    pub supplied: usize,
    /// Expected cases with usable, hash-bound comparison outcomes.
    pub compared: usize,
    /// Outcome counts; sum equals expected.
    pub outcomes: BTreeMap<String, usize>,
    /// Every unique expected case in stable order.
    pub cases: Vec<Case>,
    /// Duplicate case IDs or artifact filenames; never silently deduplicated into completeness.
    pub duplicates: Vec<String>,
    /// Supplied case IDs absent from expectations.
    pub unexpected: Vec<String>,
    /// Compared report entries not assigned to a supplied case.
    pub unassigned_comparisons: Vec<String>,
    /// complete or incomplete; an empty declaration cannot establish completeness.
    pub coverage: String,
}

/// Reconciles stable case identities against actual comparison outcomes.
pub fn reconcile(manifest: &Manifest, report: &Report) -> Result<Inventory> {
    if manifest.schema != "saccade-inventory.v1"
        || manifest
            .expected
            .iter()
            .any(|e| e.case_id.is_empty() || e.entry.is_empty())
        || manifest.supplied.iter().any(|s| {
            s.case_id.is_empty()
                || !matches!(
                    s.state.as_str(),
                    "captured" | "missing" | "unusable" | "stale" | "skipped" | "quarantined"
                )
        })
    {
        return Err(Error::Config(
            "invalid capture inventory schema, IDs or producer states".into(),
        ));
    }
    let mut expected: BTreeMap<&str, Vec<&Expected>> = BTreeMap::new();
    let mut supplied: BTreeMap<&str, Vec<&Supplied>> = BTreeMap::new();
    let mut filenames: BTreeMap<&str, usize> = BTreeMap::new();
    let mut comparisons: BTreeMap<&str, Vec<&crate::Entry>> = BTreeMap::new();
    for e in &manifest.expected {
        expected.entry(&e.case_id).or_default().push(e);
    }
    for s in &manifest.supplied {
        supplied.entry(&s.case_id).or_default().push(s);
        if let Some(name) = &s.entry {
            *filenames.entry(name).or_default() += 1;
        }
    }
    for e in &report.entries {
        comparisons.entry(&e.name).or_default().push(e);
    }
    let mut duplicates = BTreeSet::new();
    for (id, rows) in &expected {
        if rows.len() > 1 {
            duplicates.insert(format!("expected:{id}"));
        }
    }
    for (id, rows) in &supplied {
        if rows.len() > 1 {
            duplicates.insert(format!("supplied:{id}"));
        }
    }
    for (name, count) in &filenames {
        if *count > 1 {
            duplicates.insert(format!("filename:{name}"));
        }
    }
    for (name, rows) in &comparisons {
        if rows.len() > 1 {
            duplicates.insert(format!("comparison:{name}"));
        }
    }
    let mut outcomes = BTreeMap::new();
    let mut cases = vec![];
    let mut captured = 0;
    let mut compared = 0;
    for (id, expectations) in &expected {
        let e = expectations[0];
        let attempts = supplied.get(id);
        let s = attempts.and_then(|v| v.first()).copied();
        let measurement = s
            .and_then(|s| s.entry.as_ref())
            .and_then(|name| comparisons.get(name.as_str()))
            .filter(|v| v.len() == 1)
            .map(|v| v[0]);
        if attempts.is_some_and(|v| v.iter().any(|s| s.state == "captured")) {
            captured += 1;
        }
        let duplicated = expectations.len() > 1
            || attempts.is_some_and(|v| v.len() > 1)
            || s.and_then(|s| s.entry.as_ref()).is_some_and(|n| {
                filenames[n.as_str()] > 1
                    || comparisons.get(n.as_str()).is_some_and(|v| v.len() > 1)
            });
        let outcome = if duplicated {
            "duplicate"
        } else if let Some(s) = s {
            if s.state != "captured" {
                s.state.as_str()
            } else if let Some(m) = measurement {
                if m.status == Status::Error
                    || m.capture_validity.status == crate::meta::Validity::Invalid
                {
                    "unusable"
                } else if matches!(m.status, Status::Pass | Status::Fail)
                    && m.capture_sha256.is_some()
                    && m.capture_sha256 == s.capture_sha256
                {
                    "compared"
                } else {
                    "uncompared"
                }
            } else {
                "uncompared"
            }
        } else {
            "missing"
        };
        if outcome == "compared" {
            compared += 1;
        }
        *outcomes.entry(outcome.to_string()).or_default() += 1;
        cases.push(Case {
            case_id: (*id).into(),
            required: expectations.iter().any(|e| e.required),
            outcome: outcome.into(),
            expected_entry: e.entry.clone(),
            supplied_entry: s.and_then(|s| s.entry.clone()),
            renamed: s
                .and_then(|s| s.entry.as_ref())
                .is_some_and(|name| name != &e.entry),
            measurement: measurement.map(|m| m.status),
        });
    }
    let unexpected: Vec<_> = supplied
        .keys()
        .filter(|id| !expected.contains_key(**id))
        .map(|id| (*id).to_string())
        .collect();
    let unassigned_comparisons: Vec<_> = comparisons
        .keys()
        .filter(|name| !filenames.contains_key(**name))
        .map(|s| (*s).to_string())
        .collect();
    let complete = !expected.is_empty()
        && cases.iter().all(|c| !c.required || c.outcome == "compared")
        && duplicates.is_empty()
        && unexpected.is_empty()
        && unassigned_comparisons.is_empty();
    Ok(Inventory {
        schema: "saccade-inventory-report.v1".into(),
        manifest_sha256: String::new(),
        report_sha256: String::new(),
        expected: expected.len(),
        supplied: captured,
        compared,
        outcomes,
        cases,
        duplicates: duplicates.into_iter().collect(),
        unexpected,
        unassigned_comparisons,
        coverage: if complete { "complete" } else { "incomplete" }.into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn accounting_covers_pass_fail_missing_decode_rename_skip_quarantine_and_duplicates() {
        let raw = serde_json::json!({"schema":"saccade-report.v1","tool_version":"fixture","generated_at_unix":0,"config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},"totals":{"total":4,"pass":2,"fail":1,"error":1,"missing":0,"new":0},"entries":[
            {"name":"pass.png","status":"pass","metric_used":"mean","threshold":0.01,"paths":{},"capture_sha256":"same"},
            {"name":"fail.png","status":"fail","metric_used":"mean","threshold":0.01,"paths":{},"capture_sha256":"same"},
            {"name":"bad.png","status":"error","metric_used":"mean","threshold":0.01,"paths":{}},
            {"name":"renamed.png","status":"pass","metric_used":"mean","threshold":0.01,"paths":{},"capture_sha256":"same"}]});
        let report: Report = serde_json::from_value(raw).unwrap();
        let mut manifest = Manifest {
            schema: "saccade-inventory.v1".into(),
            expected: vec![],
            supplied: vec![],
        };
        for (id, state, entry) in [
            ("pass", "captured", "pass.png"),
            ("fail", "captured", "fail.png"),
            ("bad", "captured", "bad.png"),
            ("rename", "captured", "renamed.png"),
            ("missing", "missing", ""),
            ("skip", "skipped", ""),
            ("quarantine", "quarantined", ""),
            ("dup", "captured", "dup.png"),
            ("stale", "stale", ""),
        ] {
            manifest.expected.push(Expected {
                case_id: id.into(),
                entry: format!("{id}.png"),
                required: true,
            });
            manifest.supplied.push(Supplied {
                case_id: id.into(),
                entry: (!entry.is_empty()).then(|| entry.into()),
                state: state.into(),
                capture_sha256: Some("same".into()),
            });
        }
        manifest.supplied.push(manifest.supplied[7].clone());
        let inv = reconcile(&manifest, &report).unwrap();
        assert_eq!(inv.expected, 9);
        assert_eq!(inv.compared, 3);
        assert_eq!(inv.outcomes.values().sum::<usize>(), 9);
        assert_eq!(inv.coverage, "incomplete");
        assert!(
            inv.cases
                .iter()
                .find(|c| c.case_id == "rename")
                .unwrap()
                .renamed
        );
        assert_eq!(inv.outcomes["unusable"], 1);
        manifest
            .expected
            .retain(|e| matches!(e.case_id.as_str(), "pass" | "fail" | "rename"));
        manifest
            .supplied
            .retain(|e| matches!(e.case_id.as_str(), "pass" | "fail" | "rename"));
        let mut paired = report.clone();
        paired.entries.retain(|e| e.status != Status::Error);
        assert_eq!(reconcile(&manifest, &paired).unwrap().coverage, "complete"); // a failing measurement still has complete capture coverage
        manifest.supplied[0].capture_sha256 = Some("stale-bytes".into());
        assert_eq!(
            reconcile(&manifest, &paired).unwrap().coverage,
            "incomplete"
        );
    }
}
