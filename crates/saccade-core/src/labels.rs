//! Human final label collection with immutable evidence identities and provenance.
use crate::judge::{EvidenceOptions, JudgeItem, JudgeQuestion, PixelSource, report_items};
use crate::report::{Hotspot, Report};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Labels schema identifier.
pub const SCHEMA: &str = "saccade-labels.v1";

/// One human final label, usable without a live server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Label {
    /// Entry name.
    pub entry: String,
    /// Bounded question name.
    pub question: String,
    /// Human final answer in stable order (a/b for preference).
    pub answer: String,
    /// Hash of the exact evidence this final is about.
    pub evidence_hash: String,
    /// Encoded evidence in stable order.
    pub state: Value,
    /// Intent or rubric.
    pub intent: Option<String>,
    /// Reference and candidate paths, relative to the labels document.
    pub images: Vec<String>,
    /// SHA-256 of image file bytes in the same order.
    pub sha256: Vec<Option<String>>,
    /// Crop selection.
    pub hotspots: Vec<Hotspot>,
    /// Report path, relative to the labels document, when available.
    pub report: Option<String>,
    /// Human identity, timestamp and source file(s).
    pub provenance: Vec<Value>,
}
/// Deduplicated label set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Labels {
    /// Always saccade-labels.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-labels.v1")))]
    pub schema: String,
    /// Evidence-bound human finals.
    pub items: Vec<Label>,
}

/// Canonical identity shared with review; paths and provenance do not affect it.
pub fn identity(label: &Label) -> String {
    let states = if label.state.get("ab").is_some() && label.state.get("ba").is_some() {
        [label.state["ab"].clone(), label.state["ba"].clone()]
    } else {
        [label.state.clone(), label.state.clone()]
    };
    crate::review::hash(&json!([
        label.entry,
        label.question,
        states,
        label.intent,
        label.sha256,
        label.hotspots
    ]))
}

impl Labels {
    /// Read, validate closed answers, hashes and duplicate consistency.
    pub fn read(path: &Path) -> Result<Self> {
        let set: Self = serde_json::from_slice(
            &std::fs::read(path).map_err(crate::run::io_err("reading labels".into()))?,
        )?;
        if set.schema != SCHEMA {
            return Err(Error::Config("unsupported labels schema".into()));
        }
        let mut seen = BTreeMap::new();
        for i in &set.items {
            let q = JudgeQuestion::parse(&i.question)
                .ok_or_else(|| Error::Config("unknown label question".into()))?;
            let valid = if q == JudgeQuestion::Preference {
                ["a", "b", "tie"].contains(&i.answer.as_str())
            } else {
                q.wire_answers().contains(&i.answer) && !crate::judge_stats::is_abstain(&i.answer)
            };
            if i.evidence_hash != identity(i) {
                return Err(Error::Config(format!(
                    "label evidence identity mismatch: {}",
                    i.entry
                )));
            }
            if !valid || i.provenance.is_empty() || i.images.len() != i.sha256.len() {
                return Err(Error::Config(
                    "labels require final answers, evidence, hashes and provenance".into(),
                ));
            }
            if let Some(previous) = seen.insert((&i.evidence_hash, &i.question), &i.answer)
                && previous != &i.answer
            {
                return Err(Error::Config(
                    "conflicting human finals for the same evidence".into(),
                ));
            }
        }
        Ok(set)
    }
}

/// Create a replay item, verifying stored image identities before any upload.
pub fn replay(label: &Label, document: &Path) -> Result<JudgeItem> {
    if label.evidence_hash != identity(label) {
        return Err(Error::Config("label evidence identity mismatch".into()));
    }
    let mut source = None;
    if label.images.len() == 2 {
        let paths: Vec<_> = label
            .images
            .iter()
            .map(|p| crate::paths::resolve(p, document))
            .collect();
        for (p, h) in paths.iter().zip(&label.sha256) {
            let actual = crate::run::sha256_file(p)?;
            if h.as_ref() != Some(&actual) {
                return Err(Error::Config(format!(
                    "label evidence changed: {}",
                    p.display()
                )));
            }
        }
        source = Some(PixelSource::Files(paths[0].clone(), paths[1].clone()));
    }
    Ok(JudgeItem {
        id: format!("{}:{}", label.entry, label.question),
        entry: label.entry.clone(),
        hotspot: None,
        question: JudgeQuestion::parse(&label.question)
            .ok_or_else(|| Error::Config("unknown label question".into()))?,
        request_hash: label.evidence_hash.clone(),
        states: if label.state.get("ab").is_some() && label.state.get("ba").is_some() {
            [label.state["ab"].clone(), label.state["ba"].clone()]
        } else {
            [label.state.clone(), label.state.clone()]
        },
        source,
        hotspots: label.hotspots.clone(),
        canary: None,
        pair: None,
        intent: label.intent.clone(),
    })
}

fn add(out: &mut BTreeMap<(String, String), Label>, label: Label) -> Result<()> {
    let key = (label.evidence_hash.clone(), label.question.clone());
    if let Some(old) = out.get_mut(&key) {
        if old.answer != label.answer {
            return Err(Error::Config(format!(
                "conflicting human finals for {}",
                label.entry
            )));
        }
        for p in label.provenance {
            if !old.provenance.contains(&p) {
                old.provenance.push(p);
            }
        }
        if old.images.is_empty() && !label.images.is_empty() {
            old.images = label.images;
            old.sha256 = label.sha256;
            old.state = label.state;
            old.hotspots = label.hotspots;
        }
    } else {
        out.insert(key, label);
    }
    Ok(())
}

fn from_report(
    path: &Path,
    entry: &str,
    answer: &str,
    question: &str,
    intent: Option<String>,
    out: &Path,
    provenance: Value,
) -> Result<Label> {
    let report: Report = serde_json::from_slice(
        &std::fs::read(path).map_err(crate::run::io_err("reading labelled report".into()))?,
    )?;
    let e = report
        .entries
        .iter()
        .find(|e| e.name == entry)
        .ok_or_else(|| Error::Config("label entry absent from report".into()))?;
    let q = JudgeQuestion::parse(question)
        .ok_or_else(|| Error::Config("unknown label question".into()))?;
    let item = if crate::decision::deterministic_failure(&report, e).is_none() {
        report_items(
            &report,
            path.parent().unwrap_or(Path::new(".")),
            q,
            intent.as_deref(),
            &[entry.into()],
            &EvidenceOptions::default(),
        )
        .map_err(|e| Error::Config(e.to_string()))?
        .0
        .into_iter()
        .next()
    } else {
        None
    };
    let state = item
        .as_ref()
        .map(|i| json!({"ab":i.states[0],"ba":i.states[1]}))
        .unwrap_or_else(|| crate::decision::entry_state(&report, e, intent.as_deref()));
    let images: Vec<_> = [&e.paths.baseline, &e.paths.capture]
        .into_iter()
        .filter_map(|p| p.as_ref())
        .map(|p| {
            crate::paths::record(
                &crate::paths::resolve(p, path),
                out.parent().unwrap_or(Path::new(".")),
                false,
            )
        })
        .collect();
    let sha256 = [
        (&e.paths.baseline, &e.baseline_sha256),
        (&e.paths.capture, &e.capture_sha256),
    ]
    .into_iter()
    .filter(|(p, _)| p.is_some())
    .map(|(_, h)| h.clone())
    .collect::<Vec<_>>();
    let evidence_hash = item
        .as_ref()
        .map(|i| crate::review::evidence_hash(i, &report))
        .unwrap_or_else(|| crate::review::hash(&json!([state, sha256])));
    let mut label = Label {
        entry: entry.into(),
        question: question.into(),
        answer: answer.into(),
        evidence_hash,
        state,
        intent,
        images,
        sha256,
        hotspots: e.hotspots.clone(),
        report: Some(crate::paths::record(
            path,
            out.parent().unwrap_or(Path::new(".")),
            false,
        )),
        provenance: vec![provenance],
    };
    label.evidence_hash = identity(&label);
    Ok(label)
}

/// Harvest report decisions, serve sessions, answered review inbox items and
/// consistent human votes in both orders. Model proposals and promoted gates are excluded.
pub fn collect(decisions_dir: Option<&Path>, reports: &[PathBuf], out: &Path) -> Result<Labels> {
    let mut labels = BTreeMap::new();
    let mut files = Vec::new();
    let mut report_map = BTreeMap::new();
    for report in reports {
        let canon = crate::paths::canonicalize(report)
            .map_err(crate::run::io_err("resolving label report".into()))?;
        let d = canon
            .parent()
            .unwrap_or(Path::new("."))
            .join(crate::decision::DECISIONS_FILE_NAME);
        report_map.insert(d.clone(), canon);
        if d.is_file() {
            files.push(d);
        }
    }
    if let Some(dir) = decisions_dir.filter(|p| p.is_dir()) {
        for f in walkdir::WalkDir::new(dir).follow_links(false) {
            let f = f.map_err(|e| Error::Config(format!("scanning decisions: {e}")))?;
            if !f.file_type().is_file() {
                continue;
            }
            let p = f.path();
            let name = f.file_name().to_string_lossy();
            if name.ends_with("saccade-decisions.v1.json") {
                let p = crate::paths::canonicalize(p)
                    .map_err(crate::run::io_err("resolving decisions".into()))?;
                files.push(p);
            } else if p.parent().is_some_and(|p| p.ends_with("inbox")) && name.ends_with(".json") {
                let it: crate::inbox::Item = serde_json::from_slice(
                    &std::fs::read(p).map_err(crate::run::io_err("reading inbox labels".into()))?,
                )?;
                if it.status != "answered"
                    || !it
                        .answer
                        .as_deref()
                        .is_some_and(|a| matches!(a, "accept" | "reject"))
                {
                    continue;
                }
                let ctx: Value = it
                    .context
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(Value::Null);
                let (Some(report), Some(entry), Some(hash)) = (
                    ctx["report"].as_str(),
                    ctx["entry"].as_str(),
                    ctx["evidence_hash"].as_str(),
                ) else {
                    continue;
                };
                let intent = ctx["intent"].as_str().map(str::to_owned);
                let mut label = from_report(
                    Path::new(report),
                    entry,
                    it.answer.as_deref().unwrap_or(""),
                    "accept",
                    intent,
                    out,
                    json!({"source":crate::paths::portable(p),"human":"inbox","timestamp":it.answered_unix}),
                )?;
                label.state = ctx["state"].clone();
                label.evidence_hash = identity(&label);
                if label.evidence_hash != hash {
                    return Err(Error::Config(
                        "inbox final belongs to changed evidence".into(),
                    ));
                }
                add(&mut labels, label)?;
            } else if name == "human-label-items.json"
                && !p
                    .parent()
                    .is_some_and(|d| d.join("review-labels.json").is_file())
            {
                let dir = p.parent().unwrap_or(Path::new("."));
                let metadata: Vec<Label> = serde_json::from_slice(
                    &std::fs::read(p)
                        .map_err(crate::run::io_err("reading human vote evidence".into()))?,
                )?;
                let run = crate::judge_vote::read_run(dir).map_err(Error::Config)?;
                type Settled = BTreeMap<(String, String), Vec<(String, String, u64)>>;
                let mut paired: Settled = BTreeMap::new();
                for v in crate::judge_vote::votes(dir) {
                    let Some(it) = run.items.iter().find(|i| i.id == v.item) else {
                        continue;
                    };
                    let stable = if it.question == "preference" {
                        match (v.answer.as_str(), it.order.as_str()) {
                            ("P1", "ab") | ("P2", "ba") => "a",
                            ("P2", "ab") | ("P1", "ba") => "b",
                            ("tie", _) => "tie",
                            _ => continue,
                        }
                        .to_owned()
                    } else {
                        if crate::judge_stats::is_abstain(&v.answer) {
                            continue;
                        }
                        v.answer
                    };
                    paired.entry((v.voter, it.item.clone())).or_default().push((
                        it.order.clone(),
                        stable,
                        v.ts,
                    ));
                }
                for ((voter, item), v) in paired {
                    if v.len() != 2 || v[0].0 == v[1].0 || v[0].1 != v[1].1 {
                        continue;
                    }
                    let Some(mut label) = metadata.iter().find(|l| l.entry == item).cloned() else {
                        continue;
                    };
                    label.answer = v[0].1.clone();
                    label.provenance = vec![
                        json!({"human":voter,"source":crate::paths::portable(p),"timestamp_ms":v.iter().map(|v|v.2).max()}),
                    ];
                    label.images = label
                        .images
                        .iter()
                        .map(|i| {
                            crate::paths::record(
                                &crate::paths::resolve(i, p),
                                out.parent().unwrap_or(Path::new(".")),
                                false,
                            )
                        })
                        .collect();
                    if label.evidence_hash != identity(&label) {
                        return Err(Error::Config("changed human vote evidence".into()));
                    }
                    add(&mut labels, label)?;
                }
            } else if name == "review-labels.json" {
                let dir = p.parent().unwrap_or(Path::new("."));
                let metadata: Vec<Value> = serde_json::from_slice(
                    &std::fs::read(p)
                        .map_err(crate::run::io_err("reading vote metadata".into()))?,
                )?;
                let run = crate::judge_vote::read_run(dir).map_err(Error::Config)?;
                type PairedVotes = BTreeMap<(String, String), Vec<(String, String, u64)>>;
                let mut paired: PairedVotes = BTreeMap::new();
                for vote in crate::judge_vote::votes(dir) {
                    let Some(item) = run.items.iter().find(|i| i.id == vote.item) else {
                        continue;
                    };
                    let stable = match (vote.answer.as_str(), item.order.as_str()) {
                        ("P1", "ab") | ("P2", "ba") => "a",
                        ("P2", "ab") | ("P1", "ba") => "b",
                        ("tie", _) => "tie",
                        _ => continue,
                    };
                    paired
                        .entry((vote.voter, item.item.clone()))
                        .or_default()
                        .push((item.order.clone(), stable.into(), vote.ts));
                }
                for ((voter, id), v) in paired {
                    if v.len() != 2 || v[0].0 == v[1].0 || v[0].1 != v[1].1 {
                        continue;
                    }
                    let Some(m) = metadata.iter().find(|m| m["item"] == id) else {
                        continue;
                    };
                    let (Some(report), Some(entry)) = (m["report"].as_str(), m["entry"].as_str())
                    else {
                        continue;
                    };
                    let mut label = from_report(
                        Path::new(report),
                        entry,
                        crate::review::candidate_answer(&v[0].1),
                        "accept",
                        m["intent"].as_str().map(str::to_owned),
                        out,
                        json!({"source":crate::paths::portable(p),"human":voter,"timestamp_ms":v.iter().map(|v|v.2).max()}),
                    )?;
                    label.state = m["state"].clone();
                    label.evidence_hash = identity(&label);
                    if label.evidence_hash != m["evidence_hash"] {
                        return Err(Error::Config(
                            "vote final belongs to changed evidence".into(),
                        ));
                    }
                    add(&mut labels, label)?;
                }
            }
        }
    }
    files.sort();
    files.dedup();
    for file in files {
        let d = crate::view::read_decisions(&file)?;
        if d.blind {
            continue;
        }
        for set in &d.sets {
            let Some(verdict) = set.decision else {
                continue;
            };
            let human = set
                .proposals
                .iter()
                .filter(|p| {
                    crate::judge_stats::is_human_source(&p.source)
                        && !p.proposed
                        && !p.promoted
                        && p.question == "accept"
                })
                .max_by_key(|p| p.timestamp_ms);
            if human.is_none() && set.proposals.iter().any(|p| p.promoted) {
                continue;
            }
            let answer = serde_json::to_value(verdict)?
                .as_str()
                .unwrap_or("")
                .to_owned();
            if human.is_some_and(|p| p.answer != answer) {
                continue;
            }
            let provenance = json!({"source":crate::paths::portable(&file),"human":human.map_or("viewer",|p|p.source.as_str()),"timestamp_ms":set.timestamp_ms});
            if let Some(report) = report_map.get(&file) {
                let label =
                    from_report(report, &set.name, &answer, "accept", None, out, provenance)?;
                let current = &label.sha256;
                if !set.sha256.is_empty() && current != &set.sha256 {
                    return Err(Error::Config(
                        "human decision belongs to changed image hashes".into(),
                    ));
                }
                add(&mut labels, label)?;
            } else {
                if set.sha256.is_empty() || set.sha256.iter().any(Option::is_none) {
                    continue;
                }
                let base = out.parent().unwrap_or(Path::new("."));
                let images = d
                    .dirs
                    .iter()
                    .map(|p| {
                        crate::paths::record(
                            &crate::paths::resolve(p, &file).join(&set.name),
                            base,
                            false,
                        )
                    })
                    .collect();
                let mut label = Label {
                    entry: set.name.clone(),
                    question: "accept".into(),
                    answer,
                    evidence_hash: String::new(),
                    state: json!({"human_evidence":set.sha256}),
                    intent: None,
                    images,
                    sha256: set.sha256.clone(),
                    hotspots: Vec::new(),
                    report: None,
                    provenance: vec![provenance],
                };
                label.evidence_hash = identity(&label);
                add(&mut labels, label)?;
            }
        }
    }
    let set = Labels {
        schema: SCHEMA.into(),
        items: labels.into_values().collect(),
    };
    for label in &set.items {
        let _ = replay(label, out)?;
    }
    crate::review::write_json(out, &serde_json::to_value(&set)?)?;
    Ok(set)
}
