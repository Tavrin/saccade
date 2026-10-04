//! Bind staged workbench labels to production R9 records. These are labels,
//! never baseline approval receipts or authority to publish/dispatch.
use saccade_core::evidence::{Artifact, Document, canonical, case::*, human::*};
use saccade_core::labels::*;
use serde_json::{Value, json};
use std::path::Path;

fn exposure(v: &Value) -> Availability<bool> {
    match v.as_str() {
        Some("yes") => Availability::Available { value: true },
        Some("no") => Availability::Available { value: false },
        _ => Availability::missing("reviewer exposure unknown"),
    }
}
pub fn build(folder: &Path, input: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let v: Value = serde_json::from_slice(&std::fs::read(input)?)?;
    let doc = Document::read(&folder.join("case.json"))?;
    let Artifact::Case(c) = doc.artifact else {
        return Err("missing case".into());
    };
    let mut c = *c;
    let answers = v["final"]["answers"]
        .as_object()
        .ok_or("missing human answers")?;
    for q in answers.keys() {
        c.requests.push(serde_json::from_slice(&std::fs::read(
            folder.join(format!("{q}.json")),
        )?)?);
    }
    c.validate()?;
    let exp = ReviewerExposure {
        mapping_access: exposure(&v["final"]["exposure"]["mapping"]),
        implementation_context: exposure(&v["final"]["exposure"]["implementation"]),
        reviewer: Some("single-pilot-labeler".into()),
    };
    let mut h = HumanDecision {
        decision_id: canonical::Digest::of_bytes(b""),
        binding: ReviewBinding::for_case(
            &c,
            ArtifactRef::from_file(input, &folder.join("human.json"), true)?,
        )?,
        disposition: Disposition::NeedsWork,
        approve_deletions: false,
        channel: Channel::Workbench,
        exposure: exp.clone(),
        note: "Question labels; unresolved human approval.".into(),
        timestamp_unix_ms: v["final"]["recorded_unix_ms"].as_u64(),
    };
    h.refresh_id()?;
    h.validate_for(&c)?;
    let mut items = Vec::new();
    for r in &c.requests {
        let answer = answers[&r.question.id]
            .as_str()
            .ok_or("missing closed answer")?;
        let vision = if r.question.id == "vision.route.v1" {
            let a = &v["initial"]["vision"];
            let b = &v["final"]["vision"];
            Some(VisionRoutingRubric {
                version: VISION_RUBRIC_VERSION.into(),
                initial: StructuredAssessment {
                    packet_sha256: canonical::digest(&r.evidence)?,
                    supports_disposition: serde_json::from_value(
                        a["supports_disposition"].clone(),
                    )?,
                    unresolved_visual_fact: a["unresolved_visual_fact"].as_str().map(String::from),
                    provisional_route: a["provisional_route"]
                        .as_str()
                        .ok_or("missing provisional route")?
                        .into(),
                    uncertainty: a["uncertainty"]
                        .as_str()
                        .ok_or("missing uncertainty")?
                        .into(),
                    recorded_unix_ms: v["initial"]["recorded_unix_ms"]
                        .as_u64()
                        .ok_or("missing initial time")?,
                },
                final_assessment: VisualAssessment {
                    requires_pixels: serde_json::from_value(b["requires_pixels"].clone())?,
                    minimum_scope: serde_json::from_value(b["minimum_scope"].clone())?,
                    requires_human: b["requires_human"]
                        .as_bool()
                        .ok_or("missing authority assessment")?,
                    necessary_visual_fact: b["necessary_visual_fact"].as_str().map(String::from),
                    region_refs: serde_json::from_value(v["region_refs"].clone())?,
                    full_frame_seen: v["full_frame_seen"]
                        .as_bool()
                        .ok_or("missing display audit")?,
                    route: answer.into(),
                    recorded_unix_ms: v["final"]["recorded_unix_ms"]
                        .as_u64()
                        .ok_or("missing final time")?,
                },
            })
        } else {
            None
        };
        let mut label = QuestionLabel {
            label_id: canonical::Digest::of_bytes(b""),
            label: saccade_core::evidence::human::Label {
                case_id: c.case_id.clone(),
                request_id: r.request_id.clone(),
                question_id: r.question.id.clone(),
                answer: answer.into(),
                human_decision_id: h.decision_id.clone(),
                exposure: exp.clone(),
            },
            saw_model_proposals: exposure(&v["final"]["exposure"]["model"]),
            vision_rubric: vision,
            supersedes: None,
            test_retest_of: v["test_retest_of"][&r.question.id]
                .as_str()
                .map(canonical::Digest::parse)
                .transpose()?,
        };
        label.label_id = label.identity()?;
        label.validate_for(r, &h)?;
        items.push(label);
    }
    let mut history: Vec<QuestionLabel> =
        serde_json::from_value(v.get("prior_records").cloned().unwrap_or_else(|| json!([])))?;
    history.extend(items);
    let collection = QuestionLabels {
        schema: LABEL_RECORDS_SCHEMA.into(),
        kind: "human_label_records".into(),
        items: history,
    };
    let mut decisions = vec![h.clone()];
    if let Some(previous) = v.get("prior_human").filter(|v| !v.is_null()) {
        decisions.push(serde_json::from_value(previous.clone())?);
    }
    collection.validate_for(&c.requests, &decisions)?;
    Ok(
        json!({"human":h,"records":collection,"labels":{"schema":QUESTION_LABELS_SCHEMA,
        "items":collection.items.iter().filter(|l| l.label.human_decision_id==h.decision_id && l.eligible()).map(|l| &l.label).collect::<Vec<_>>()},
        "eligibility":collection.items.iter().filter(|l|l.label.human_decision_id==h.decision_id).map(|l|json!({"question":l.label.question_id,"eligible":l.eligible()})).collect::<Vec<_>>() }),
    )
}
