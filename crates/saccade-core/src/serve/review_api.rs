//! Human session attestation. Scoped secrets exist only in live browser sessions.
use super::{Resp, State, read_body};
use crate::evidence::canonical::Digest;
use crate::evidence::case::{ArtifactRef, EvidenceCase};
use crate::evidence::human::{
    AppliedEntry, ApprovalReceipt, Channel, Disposition, HumanAttestation, HumanDecision,
    ReviewBinding, ReviewerExposure,
};
use crate::evidence::{Artifact, Document};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tiny_http::Request;

pub(crate) struct ReviewSession {
    document: PathBuf,
    case: EvidenceCase,
    binding: ReviewBinding,
    files: Vec<ArtifactRef>,
    pub(super) expires: Instant,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64)
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn write(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(crate::paths::native(path))
        .map_err(err)?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(err)?)
        .map_err(err)
}
fn verify_reference(reference: &ArtifactRef, document: &Path) -> Result<(), String> {
    let base = document.parent().ok_or("case has no directory")?;
    let resolved = crate::paths::canonicalize(crate::paths::resolve(&reference.path, document))
        .map_err(err)?;
    if !resolved.starts_with(base) {
        return Err("review reference escapes its portable bundle".into());
    }
    reference.verify(document).map_err(err)
}
fn live(document: &Path) -> Result<EvidenceCase, String> {
    let Artifact::Case(case) = Document::read(document).map_err(err)?.artifact else {
        return Err("review requires an evidence case".into());
    };
    verify_reference(&case.measurement.report, document)?;
    for input in &case.inputs {
        verify_reference(&input.content, document)?;
        for sidecar in &input.sidecars {
            verify_reference(sidecar, document)?;
        }
    }
    if let Some(intent) = case.intent.value()
        && let Some(source) = &intent.source
    {
        verify_reference(source, document)?;
    }
    Ok(*case)
}
fn presentation(document: &Path) -> Result<Vec<ArtifactRef>, String> {
    let base = document.parent().ok_or("case has no directory")?;
    let mut files = Vec::new();
    for name in [
        "index.html",
        "evidence.json",
        "saccade-report.v1.json",
        "report-pixels.js",
        "saccade-decisions.v1.js",
        "assets",
        "images",
    ] {
        let path = base.join(name);
        if !path.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(path).follow_links(false) {
            let entry = entry.map_err(err)?;
            if entry.file_type().is_symlink() {
                return Err("review presentation contains a symlink".into());
            }
            if entry.file_type().is_file() {
                let reference =
                    ArtifactRef::from_file(entry.path(), document, false).map_err(err)?;
                verify_reference(&reference, document)?;
                files.push(reference);
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    case: String,
    scope: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attest {
    token: String,
    case_id: Digest,
    scope: Vec<String>,
    disposition: Disposition,
    approve_deletions: bool,
    exposure: ReviewerExposure,
    note: String,
    #[serde(default)]
    labels: Option<crate::labels::QuestionLabels>,
    #[serde(default)]
    answers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    saw_model_proposals: Option<bool>,
}

pub(super) fn get(state: &State, path: &str, query: &HashMap<String, String>) -> Option<Resp> {
    if path != "/review" {
        return None;
    }
    let case = query.get("case").cloned().unwrap_or_default();
    let document = match crate::serve::browse::resolve_under(state, &case) {
        Ok(p) => p,
        Err(_) => return Some(Resp::error(400, "case is outside allowed storage")),
    };
    let case_data = match live(&document) {
        Ok(c) => c,
        Err(e) => return Some(Resp::error(409, &e)),
    };
    let data = json!({"csrf":state.token,"path":case,"scope":case_data.scope.entries});
    let data = match serde_json::to_string(&data) {
        Ok(s) => s.replace("</", "<\\/").replace("<!--", "<\\u0021--"),
        Err(e) => return Some(Resp::error(500, &e.to_string())),
    };
    let css = crate::render::shared::page_css(&[]);
    Some(Resp::html(
        include_str!("../../assets/review.html")
            .replace("__CSS__", &css)
            .replace("__DATA__", &data),
    ))
}

pub(super) fn post(state: &State, req: &mut Request, path: &str) -> Resp {
    let body = match read_body(req, 256 * 1024) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let result = match path {
        "/api/review/session" => serde_json::from_slice::<Start>(&body)
            .map_err(err)
            .and_then(|start| {
                let document = crate::serve::browse::resolve_under(state, &start.case)
                    .map_err(|_| "case is outside allowed storage".to_string())?;
                let case = live(&document)?;
                if start.scope != case.scope.entries {
                    return Err("session scope differs from the live case".into());
                }
                let shown = ArtifactRef::from_file(
                    &document.with_file_name("index.html"),
                    &document,
                    false,
                )
                .map_err(err)?;
                let binding = ReviewBinding::for_case(&case, shown).map_err(err)?;
                let files = presentation(&document)?;
                let mut sessions = state
                    .reviews
                    .lock()
                    .map_err(|_| "review lock unavailable")?;
                sessions.retain(|_, s| Instant::now() < s.expires);
                if sessions.len() >= 128 {
                    return Err("too many active review sessions".into());
                }
                let token = crate::serve::random_token();
                sessions.insert(
                    token.clone(),
                    ReviewSession {
                        document,
                        case: case.clone(),
                        binding: binding.clone(),
                        files,
                        expires: Instant::now() + state.review_ttl,
                    },
                );
                Ok(json!({"token":token,"binding":binding,"case":case,"expires_in_seconds":600}))
            }),
        "/api/review/attest" => serde_json::from_slice::<Attest>(&body)
            .map_err(err)
            .and_then(|answer| attest(state, answer)),
        _ => return Resp::error(404, "unknown review route"),
    };
    match result {
        Ok(value) => Resp::json(&value),
        Err(e) => Resp::error(409, &e),
    }
}
fn attest(state: &State, answer: Attest) -> Result<serde_json::Value, String> {
    // Consume before validation: failures cannot replay a partially used session.
    let session = state
        .reviews
        .lock()
        .map_err(|_| "review lock unavailable")?
        .remove(&answer.token)
        .ok_or("missing, wrong or reused scoped token")?;
    if Instant::now() >= session.expires {
        return Err("expired scoped token".into());
    }
    if answer.case_id != session.binding.case_id || answer.scope != session.binding.scope.entries {
        return Err("wrong case or scope for scoped token".into());
    }
    let current = live(&session.document)?;
    if current != session.case || presentation(&session.document)? != session.files {
        return Err("stale reviewed content".into());
    }
    session.binding.validate_for(&current).map_err(err)?;
    verify_reference(&session.binding.reviewed_content, &session.document)?;
    if answer.disposition == Disposition::Accept {
        let report: crate::Report = serde_json::from_slice(
            &std::fs::read(crate::paths::resolve(
                &current.measurement.report.path,
                &session.document,
            ))
            .map_err(err)?,
        )
        .map_err(err)?;
        for name in &current.scope.entries {
            let entry = report
                .entries
                .iter()
                .find(|e| &e.name == name)
                .ok_or("unknown selected report entry")?;
            if entry.status == crate::report::Status::Error
                || entry.capture_validity.status == crate::meta::Validity::Invalid
            {
                return Err("invalid or failed captures cannot be accepted".into());
            }
            let candidate = current
                .inputs
                .iter()
                .find(|i| i.id == format!("capture:{name}"));
            if let Some(candidate) = candidate {
                if !crate::run::is_decodable(&crate::paths::resolve(
                    &candidate.content.path,
                    &session.document,
                )) {
                    return Err("candidate cannot be decoded".into());
                }
            } else if !answer.approve_deletions {
                return Err("deletions need explicit approval".into());
            }
        }
    }
    let mut decision = HumanDecision {
        decision_id: Digest::of_bytes(b""),
        binding: session.binding,
        disposition: answer.disposition,
        approve_deletions: answer.approve_deletions,
        channel: Channel::Workbench,
        exposure: answer.exposure,
        note: answer.note,
        timestamp_unix_ms: Some(now()),
    };
    decision.refresh_id().map_err(err)?;
    decision.validate_for(&current).map_err(err)?;
    let mut records = answer.labels.unwrap_or(crate::labels::QuestionLabels {
        schema: crate::labels::LABEL_RECORDS_SCHEMA.into(),
        kind: "human_label_records".into(),
        items: Vec::new(),
    });
    for (request_id, value) in answer.answers {
        let request = current
            .requests
            .iter()
            .find(|r| r.request_id.as_str() == request_id)
            .ok_or("answer names an unknown request")?;
        let mut item = crate::labels::QuestionLabel {
            label_id: Digest::of_bytes(b""),
            label: crate::evidence::human::Label {
                case_id: current.case_id.clone(),
                request_id: request.request_id.clone(),
                question_id: request.question.id.clone(),
                answer: value,
                human_decision_id: decision.decision_id.clone(),
                exposure: decision.exposure.clone(),
            },
            saw_model_proposals: answer.saw_model_proposals.map_or_else(
                || {
                    crate::evidence::case::Availability::missing(
                        "proposal exposure was not declared",
                    )
                },
                |value| crate::evidence::case::Availability::Available { value },
            ),
            vision_rubric: None,
            supersedes: None,
            test_retest_of: None,
        };
        item.label_id = item.identity().map_err(err)?;
        item.validate_for(request, &decision).map_err(err)?;
        records.items.push(item);
    }
    let labels = if !records.items.is_empty() {
        Some(
            records
                .eligible_labels(&current.requests, std::slice::from_ref(&decision))
                .map_err(err)?,
        )
    } else {
        None
    };
    let id = crate::serve::random_token();
    let dir = state.decisions.join("reviews").join(&id);
    let parent = dir.parent().ok_or("review has no directory")?;
    std::fs::create_dir_all(crate::paths::native(parent)).map_err(err)?;
    if crate::paths::canonicalize(parent).map_err(err)? != parent {
        return Err("review output directory is a symlink".into());
    }
    std::fs::create_dir(crate::paths::native(&dir)).map_err(err)?;
    let mut cleanup = Pending {
        dir: dir.clone(),
        complete: false,
    };
    // Retain a portable copy of exactly what was reviewed; never link back to the archive.
    for reference in &session.files {
        let source = crate::paths::resolve(&reference.path, &session.document);
        let bytes = std::fs::read(crate::paths::native(&source)).map_err(err)?;
        if Digest::of_bytes(&bytes) != reference.sha256 {
            return Err("stale presentation during recording".into());
        }
        let target = dir.join("reviewed").join(&reference.path);
        std::fs::create_dir_all(target.parent().ok_or("presentation has no parent")?)
            .map_err(err)?;
        std::fs::write(crate::paths::native(&target), bytes).map_err(err)?;
    }
    decision.binding.reviewed_content.path = "reviewed/index.html".into();
    decision.refresh_id().map_err(err)?;
    let audit = json!({"decision_id":decision.decision_id,"binding":decision.binding,"timestamp_unix_ms":now(),"scope":"local reviewed snapshot; capture archive remains read-only","approved_directory":"approved"});
    write(&dir.join("attestation-audit.json"), &audit)?;
    let attestation = HumanAttestation {
        decision_id: decision.decision_id.clone(),
        binding: decision.binding.clone(),
        audit_ref: ArtifactRef::from_file(
            &dir.join("attestation-audit.json"),
            &dir.join("receipt.json"),
            false,
        )
        .map_err(err)?,
        timestamp_unix_ms: now(),
    };
    let mut receipt = None;
    if decision.disposition == Disposition::Accept {
        let mut applied = Vec::new();
        for name in &decision.binding.scope.entries {
            if !crate::serve::browse::is_plain_rel(name) || name.is_empty() {
                return Err("invalid selected entry path".into());
            }
            let baseline = current
                .inputs
                .iter()
                .find(|i| i.id == format!("baseline:{name}"));
            let candidate = current
                .inputs
                .iter()
                .find(|i| i.id == format!("capture:{name}"));
            if candidate.is_none() && !decision.approve_deletions {
                return Err("deletions need explicit approval".into());
            }
            let target = dir.join("approved").join(name);
            if let Some(baseline) = baseline {
                let bytes = std::fs::read(crate::paths::native(&crate::paths::resolve(
                    &baseline.content.path,
                    &session.document,
                )))
                .map_err(err)?;
                if Digest::of_bytes(&bytes) != baseline.content.sha256 {
                    return Err("stale baseline before applying".into());
                }
                std::fs::create_dir_all(target.parent().ok_or("entry has no parent")?)
                    .map_err(err)?;
                std::fs::write(crate::paths::native(&target), bytes).map_err(err)?;
            }
            if let Some(candidate) = candidate {
                let bytes = std::fs::read(crate::paths::native(&crate::paths::resolve(
                    &candidate.content.path,
                    &session.document,
                )))
                .map_err(err)?;
                if Digest::of_bytes(&bytes) != candidate.content.sha256 {
                    return Err("stale candidate before applying".into());
                }
                std::fs::create_dir_all(target.parent().ok_or("entry has no parent")?)
                    .map_err(err)?;
                std::fs::write(crate::paths::native(&target), bytes).map_err(err)?;
            }
            if candidate.is_none() && baseline.is_some() {
                std::fs::remove_file(crate::paths::native(&target)).map_err(err)?;
            }
            applied.push(AppliedEntry {
                entry_id: name.clone(),
                before: baseline.map(|i| i.content.sha256.clone()),
                after: candidate.map(|i| i.content.sha256.clone()),
            });
        }
        let value = ApprovalReceipt {
            decision_id: decision.decision_id.clone(),
            binding: decision.binding.clone(),
            channel: Channel::Workbench,
            human_attestation: Some(attestation.clone()),
            applied,
            timestamp_unix_ms: now(),
        };
        value.validate_for(&decision, &current).map_err(err)?;
        write(
            &dir.join("receipt.json"),
            &Document::new(Artifact::ApprovalReceipt(Box::new(value.clone()))),
        )?;
        receipt = Some(value);
    }
    write(&dir.join(".saccade-run"), &json!("human review"))?;
    write(
        &dir.join("decision.json"),
        &Document::new(Artifact::HumanDecision(Box::new(decision.clone()))),
    )?;
    write(&dir.join("attestation.json"), &attestation)?;
    if !records.items.is_empty() {
        write(&dir.join("human-label-records.json"), &records)?;
    }
    if let Some(labels) = labels {
        write(&dir.join("labels.json"), &labels)?;
    }
    let report: crate::Report = serde_json::from_slice(
        &std::fs::read(dir.join("reviewed/saccade-report.v1.json")).map_err(err)?,
    )
    .map_err(err)?;
    let mut resolved = current.clone();
    resolved.human_decisions.push(decision.clone());
    let summary = crate::render::bundle::summary(&report, Some(&resolved))
        .map_err(err)?
        .replace(
            "href=\"saccade-report.v1.json\"",
            "href=\"reviewed/saccade-report.v1.json\"",
        )
        .replace("href=\"evidence.json\"", "href=\"reviewed/evidence.json\"");
    write(&dir.join("decisions.json"), &vec![&decision])?;
    let css = crate::render::shared::page_css(&[include_str!("../../assets/report.css")]);
    let destination = if receipt.is_some() {
        "Applied destination: local approved snapshot in this bundle."
    } else {
        "No baseline change applied; disposition remains recorded for review."
    };
    let receipt_link = if receipt.is_some() {
        " · <a href=\"receipt.json\">Approval receipt</a>"
    } else {
        ""
    };
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>saccade reviewed selection</title><style>{css}</style></head><body><main>{summary}<p>{destination}</p><p><a href=\"reviewed/index.html\">Exact reviewed presentation</a> · <a href=\"decision.json\">Human decision</a> · <a href=\"attestation.json\">Attestation</a>{receipt_link}</p></main></body></html>"
    );
    std::fs::write(crate::paths::native(&dir.join("index.html")), html).map_err(err)?;
    cleanup.complete = true;
    Ok(
        json!({"decision":decision,"attestation":attestation,"receipt":receipt,"review_id":id,"applied_to":"local reviewed snapshot in decisions directory"}),
    )
}

struct Pending {
    dir: PathBuf,
    complete: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_dir_all(crate::paths::native(&self.dir));
        }
    }
}
