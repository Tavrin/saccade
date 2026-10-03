//! Bundled offline identity and intended-change demonstrations.
use crate::agent::CliError;
use saccade_core::config::RunConfig;
#[cfg(feature = "ai")]
use saccade_core::report::REPORT_FILE_NAME;
use saccade_core::report::{Metric, Mode};
use std::path::Path;

pub(crate) const FILES: &[(&str, &[u8])] = &[
    (
        "baseline/ui_label.png",
        include_bytes!("../assets/demo/baseline/ui_label.png"),
    ),
    (
        "capture/ui_label.png",
        include_bytes!("../assets/demo/capture/ui_label.png"),
    ),
    (
        "identity/baseline/sphere.png",
        include_bytes!("../assets/demo/identity/baseline/sphere.png"),
    ),
    (
        "identity/capture/sphere.png",
        include_bytes!("../assets/demo/identity/capture/sphere.png"),
    ),
    (
        "review/baseline/sphere.png",
        include_bytes!("../assets/demo/review/baseline/sphere.png"),
    ),
    (
        "review/capture/sphere.png",
        include_bytes!("../assets/demo/review/capture/sphere.png"),
    ),
    (
        "review/intent.json",
        include_bytes!("../assets/demo/review/intent.json"),
    ),
    (
        "provenance.json",
        include_bytes!("../assets/demo/provenance.json"),
    ),
];

pub(crate) fn reports(dir: &Path, absolute: bool) -> Result<(), CliError> {
    let identity = dir.join("identity");
    let proof = saccade_core::run::run(
        &identity.join("baseline"),
        &identity.join("capture"),
        &identity.join("report"),
        &RunConfig {
            mode: Mode::Identity,
            default_metric: Metric::Max,
            default_threshold: 0.0,
            record_absolute_paths: absolute,
            ..Default::default()
        },
    )?;
    if proof.is_regression() {
        return Err(CliError::new(
            "invalid_demo",
            "bundled identity fixture failed",
        ));
    }
    let review = dir.join("review");
    let report = saccade_core::run::run(
        &review.join("baseline"),
        &review.join("capture"),
        &review.join("report"),
        &RunConfig {
            default_threshold: 0.001,
            record_absolute_paths: absolute,
            ..Default::default()
        },
    )?;
    #[cfg(feature = "ai")]
    review_fixture(&review, &report)?;
    #[cfg(not(feature = "ai"))]
    let _ = report;
    crate::local_cmd::write_value(
        &review.join("fixture.json"),
        &serde_json::json!({
            "illustrative": true, "provider_calls": 0, "baseline_writes": 0,
            "resolution": "Illustrative reviewer accepts the declared material adjustment within the numerical bound; capture context remains unknown.",
            "human_attestation": null,
            "limits": ["Offline fixture, not a real human decision or live provider result.", "Canonical proposals require the ai feature."]
        }),
    )?;
    Ok(())
}

#[cfg(feature = "ai")]
fn review_fixture(dir: &Path, report: &saccade_core::Report) -> Result<(), CliError> {
    use saccade_core::evidence::canonical::Digest;
    use saccade_core::evidence::case::{
        ArtifactRef, Availability, Fact, FactSource, FactValue, Intent,
    };
    use saccade_core::evidence::human::{
        Channel, Disposition, HumanDecision, ReviewBinding, ReviewerExposure,
    };
    use saccade_core::evidence::proposal::{DecisionProposal, ProviderAnswer, ProviderAudit};
    use saccade_core::evidence::request::ProviderIdentity;
    use saccade_core::evidence::{Artifact, Document};
    let out = dir.join("report");
    let document = out.join("evidence.json");
    let mut case = crate::local_cmd::case_from_report(report, &out.join(REPORT_FILE_NAME))?;
    let intent_path = dir.join("intent.json");
    let mut intent: Intent = serde_json::from_slice(
        &std::fs::read(&intent_path).map_err(|e| CliError::io(e.to_string()))?,
    )?;
    intent.source = Some(ArtifactRef::from_file(&intent_path, &document, false)?);
    case.intent = Availability::Available { value: intent };
    let mean = report
        .entries
        .first()
        .and_then(|e| e.metrics.as_ref())
        .map(|m| m.mean)
        .ok_or_else(|| CliError::new("invalid_demo", "review fixture lacks a measurement"))?;
    if mean > 0.02 {
        return Err(CliError::new(
            "invalid_demo",
            "review fixture exceeds its declared mean FLIP bound",
        ));
    }
    case.facts.push(Fact {
        id: "demo-mean-flip".into(),
        name: "Mean perceptual error".into(),
        units: "FLIP".into(),
        scope: case.scope.clone(),
        source: FactSource::Measured,
        artifact: case.measurement.report.clone(),
        source_identity: case.measurement.semantic_sha256.clone(),
        value: Availability::Available {
            value: FactValue::Number(mean),
        },
        depends_on_model_observation: false,
        observation_refs: Vec::new(),
    });
    case.limits.push("Provider proposals and reviewer resolution below are illustrative offline fixtures; no provider was called and no human was attested.".into());
    case.refresh_id()?;
    saccade_core::judge_evidence::report_features(&mut case, report)?;
    saccade_core::judge_evidence::prepare_context(&mut case)?;
    let request =
        saccade_core::judge_evidence::encode(&case, "intent.match.v1", Default::default())?;
    for (model, answer, note) in [
        (
            "fixture-jev",
            "consistent",
            "Illustrative response: the material adjustment matches declared intent.",
        ),
        (
            "fixture-independent-review",
            "partially_consistent",
            "Illustrative disagreement: numerical bounds pass but capture context remains unknown.",
        ),
    ] {
        let response = ProviderAnswer {
            request_id: request.request_id.clone(),
            answer: answer.into(),
            probabilities: None,
            reason_codes: vec!["illustrative_fixture".into()],
            evidence_ids: request
                .evidence
                .facts
                .first()
                .map(|f| vec![f.id.clone()])
                .unwrap_or_default(),
            missing_evidence: request.evidence.missing.clone(),
            depends_on_model_observation: false,
            note: note.into(),
        };
        let proposal = DecisionProposal::from_response(
            &request,
            response.clone(),
            ProviderAudit {
                identity: ProviderIdentity {
                    provider: "offline-fixture".into(),
                    model: model.into(),
                    revision: Some("1".into()),
                },
                payload_sha256: Digest::of_bytes(&serde_json::to_vec(&request)?),
                response_sha256: Digest::of_bytes(&serde_json::to_vec(&response)?),
                execution_ref: None,
                timestamp_unix_ms: None,
            },
        )?;
        case.proposals.push(proposal);
    }
    case.requests.push(request);
    let mut resolution = HumanDecision {
        decision_id: Digest::of_bytes(b""),
        binding: ReviewBinding::for_case(&case, case.measurement.report.clone())?,
        disposition: Disposition::Accept, approve_deletions: false, channel: Channel::Cli,
        exposure: ReviewerExposure { mapping_access: Availability::Available { value: true },
            implementation_context: Availability::Available { value: true }, reviewer: Some("illustrative-fixture".into()) },
        note: "Illustrative human resolution fixture: accept the declared material adjustment within mean FLIP 0.02. This is not an attested human decision, renderer correctness proof, or authorization to update a baseline.".into(),
        timestamp_unix_ms: None,
    };
    resolution.refresh_id()?;
    resolution.validate_for(&case)?;
    case.human_decisions.push(resolution.clone());
    case.validate()?;
    crate::local_cmd::write_value(
        &document,
        &serde_json::to_value(Document::new(Artifact::Case(Box::new(case))))?,
    )?;
    crate::local_cmd::write_value(
        &out.join("decisions.json"),
        &serde_json::to_value(vec![resolution])?,
    )?;
    saccade_core::render::render_html(report, &out)?;
    Ok(())
}
