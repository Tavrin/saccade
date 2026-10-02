//! Focused R4 contracts, also runnable without computational/provider features.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
use saccade_core::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    case::*,
    human::*,
    legacy::{Authority, HistoricalArtifact},
    proposal::*,
    request::*,
};
use serde_json::{Value, json};

type NamedChange<T> = (&'static str, fn(&mut T));
use std::collections::BTreeMap;

fn fixture(name: &str) -> Document {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/evidence/{name}.json"));
    Document::read(&file).unwrap()
}
fn case() -> EvidenceCase {
    match fixture("case").artifact {
        Artifact::Case(c) => *c,
        _ => panic!("case fixture"),
    }
}
fn request() -> DecisionRequest {
    match fixture("decision_request").artifact {
        Artifact::DecisionRequest(r) => *r,
        _ => panic!("request fixture"),
    }
}
fn human() -> HumanDecision {
    match fixture("human_decision").artifact {
        Artifact::HumanDecision(h) => *h,
        _ => panic!("human fixture"),
    }
}
fn proposal() -> DecisionProposal {
    match fixture("decision_proposal").artifact {
        Artifact::DecisionProposal(p) => *p,
        _ => panic!("proposal fixture"),
    }
}
fn receipt() -> ApprovalReceipt {
    match fixture("approval_receipt").artifact {
        Artifact::ApprovalReceipt(r) => *r,
        _ => panic!("receipt fixture"),
    }
}

#[test]
fn shared_fixtures_bind_cases_requests_proposals_human_records_and_labels() {
    let mut case = case();
    let request = request();
    let proposal = proposal();
    request.validate_for(&case).unwrap();
    proposal.validate_for(&request).unwrap();
    case.requests.push(request.clone());
    case.proposals.push(proposal);
    case.human_decisions.push(human());
    case.validate().unwrap();
    receipt()
        .validate_for(&case.human_decisions[0], &case)
        .unwrap();
    let labels: Labels =
        serde_json::from_str(include_str!("fixtures/evidence/labels.json")).unwrap();
    labels.items[0]
        .validate_for(&request, &case.human_decisions[0])
        .unwrap();
    for name in [
        "case",
        "decision_request",
        "decision_proposal",
        "human_decision",
        "approval_receipt",
        "presentation_map",
        "next_action",
        "execution_audit",
    ] {
        let document = fixture(name);
        let serialized = serde_json::to_vec(&document).unwrap();
        let decoded: Document = canonical::decode(&serialized).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, document);
        let mut unknown = serde_json::to_value(&document).unwrap();
        unknown["extra_authority"] = json!("human");
        assert!(serde_json::from_value::<Document>(unknown).is_err());
    }
}

#[test]
fn case_identity_binds_inputs_sidecars_scope_configuration_calibration_and_intent() {
    let original = case();
    let changes: Vec<NamedChange<EvidenceCase>> = vec![
        ("input", |c| {
            c.inputs[0].content.sha256 = Digest::of_bytes(b"changed")
        }),
        ("sidecar", |c| {
            c.inputs[0].sidecars[0].sha256 = Digest::of_bytes(b"changed")
        }),
        ("native", |c| {
            if let Availability::Available { value } = &mut c.inputs[0].native_samples {
                value.sample_type = "u16".into();
            }
        }),
        ("capture", |c| {
            c.inputs[0].capture = Availability::Available {
                value: BTreeMap::from([("frame".into(), json!(42))]),
            }
        }),
        ("build", |c| {
            c.inputs[0].build = Availability::Available {
                value: BTreeMap::from([("binary".into(), json!("changed"))]),
            }
        }),
        ("scope", |c| c.scope.exclusions.push("HUD".into())),
        ("configuration", |c| {
            c.effective_config
                .insert("pixels_per_degree".into(), json!(40.0));
        }),
        ("calibration", |c| {
            c.calibration = Availability::Available {
                value: Digest::of_bytes(b"noise"),
            }
        }),
        ("measurement", |c| {
            c.measurement.semantic_sha256 = Digest::of_bytes(b"different measurements")
        }),
        ("intent", |c| {
            if let Availability::Available { value } = &mut c.intent {
                value.objective = "new intent".into();
            }
        }),
        ("criterion", |c| {
            if let Availability::Available { value } = &mut c.intent {
                value.criteria[0].expected = FactValue::Number(0.01);
            }
        }),
        ("fact", |c| {
            c.facts[0].value = Availability::Available {
                value: FactValue::Number(0.02),
            }
        }),
    ];
    for (name, change) in changes {
        let mut changed = original.clone();
        change(&mut changed);
        assert_ne!(changed.identity().unwrap(), original.case_id, "{name}");
        assert!(changed.validate().is_err(), "stale {name}");
    }
    let mut relocated = original.clone();
    relocated.inputs[0].content.path = "elsewhere/scene.png".into();
    relocated.inputs[0].sidecars[0].path = "elsewhere/card.json".into();
    relocated.inputs[0].provenance.timestamp_unix_ms = Some(1000);
    relocated.measurement.report.path = "elsewhere/report.json".into();
    relocated.measurement.report.sha256 = Digest::of_bytes(b"report with relocated provenance");
    relocated.facts[0].artifact.sha256 = relocated.measurement.report.sha256.clone();
    relocated.facts[0].artifact.path = "elsewhere/report.json".into();
    relocated.provenance.paths = vec!["elsewhere/inputs".into()];
    relocated.provenance.timestamp_unix_ms = Some(5000);
    if let Availability::Available { value } = &mut relocated.intent {
        value.provenance.timestamp_unix_ms = Some(7000);
        value.source.as_mut().unwrap().path = "elsewhere/intent.json".into();
    }
    assert_eq!(relocated.identity().unwrap(), original.case_id);
}

#[test]
fn requests_and_calibration_domains_bind_encoder_rubric_model_transforms_and_fallback() {
    let original = request();
    let changes: Vec<NamedChange<DecisionRequest>> = vec![
        ("encoder", |r| {
            r.evidence.encoder_version = "structured-evidence/2".into()
        }),
        ("question", |r| r.question.id = "intent.match.v2".into()),
        ("rubric", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .rubric_version = "observations/2".into()
        }),
        ("provider", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .extractor
                .provider = "other-provider".into()
        }),
        ("model", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .extractor
                .model = "fallback-model".into()
        }),
        ("revision", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .extractor
                .revision = Some("2".into())
        }),
        ("transform", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .transform_identity = Digest::of_bytes(b"different crop")
        }),
        ("fallback policy", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .fallback_chain_identity = Digest::of_bytes(b"different chain")
        }),
        ("fallback outcome", |r| {
            r.evidence
                .observation_context
                .as_mut()
                .unwrap()
                .fallback_outcome_identity = Digest::of_bytes(b"different actual outcome")
        }),
        ("presentation", |r| {
            r.evidence.presentation_identity = Digest::of_bytes(b"different presentation")
        }),
        ("policy", |r| {
            r.policy.insert("routing".into(), json!("policy/2"));
        }),
    ];
    for (name, change) in changes {
        let mut changed = original.clone();
        change(&mut changed);
        assert_ne!(changed.identity().unwrap(), original.request_id, "{name}");
        assert_ne!(
            changed.calibration_identity().unwrap(),
            original.calibration_identity().unwrap(),
            "{name}"
        );
    }
    let mut changed = original.clone();
    changed.evidence.observations[0].artifact.sha256 = Digest::of_bytes(b"new observation");
    changed.evidence.observations[0].source_identity =
        changed.evidence.observations[0].artifact.sha256.clone();
    assert_ne!(changed.identity().unwrap(), original.request_id);
    let mut changed_case = case();
    if let Availability::Available { value } = &mut changed_case.intent {
        value.objective = "different objective".into();
    }
    changed_case.refresh_id().unwrap();
    changed.case_id = changed_case.case_id;
    assert_ne!(changed.identity().unwrap(), original.request_id);
    let mut unresolved = original.clone();
    unresolved.evidence.observation_context = None;
    unresolved.refresh_id().unwrap();
    assert!(unresolved.validate().is_err());
}

#[test]
fn canonical_json_is_ordered_round_trip_safe_and_rejects_ambiguous_keys() {
    let a: Value =
        serde_json::from_str(r#"{"z":[3,2,1],"nested":{"b":1.0000000000000002,"a":1}}"#).unwrap();
    let b: Value =
        serde_json::from_str(r#"{"nested":{"a":1,"b":1.0000000000000002},"z":[3,2,1]}"#).unwrap();
    assert_eq!(canonical::bytes(&a).unwrap(), canonical::bytes(&b).unwrap());
    assert_eq!(
        canonical::decode::<Value>(&canonical::bytes(&a).unwrap()).unwrap(),
        a
    );
    assert_ne!(
        canonical::digest(&json!([1, 2])).unwrap(),
        canonical::digest(&json!([2, 1])).unwrap()
    );
    assert_eq!(
        Digest::of_bytes(b"abc").as_str(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(Digest::parse("sha256:placeholder").is_err());
    assert!(canonical::decode::<Value>(br#"{"x":{"a":1,"a":2}}"#).is_err());
    let mut invalid = case().facts.remove(0);
    invalid.value = Availability::Available {
        value: FactValue::Number(f64::NAN),
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn report_identity_omits_provenance_but_exact_content_remains_verifiable() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("report.json");
    let document = tmp.path().join("bundle/evidence.json");
    std::fs::create_dir_all(document.parent().unwrap()).unwrap();
    let mut report: saccade_core::Report = serde_json::from_value(json!({
        "schema":"saccade-report.v1", "tool_version":"fixture", "generated_at_unix":0,
        "baseline_dir":"../baseline", "capture_dir":"../candidate",
        "config":{"default_threshold":0.1,"default_metric":"max","pixels_per_degree":67.0,"fail_on_new":false},
        "totals":{"total":1,"pass":1,"fail":0,"new":0,"missing":0,"error":0},
        "entries":[{"name":"scene.png","status":"pass","metric_used":"max","threshold":0.1,"value":0.05,"paths":{}}]
    })).unwrap();
    std::fs::write(&file, serde_json::to_vec(&report).unwrap()).unwrap();
    let original = Measurement::from_report(
        &file,
        &document,
        vec!["scene.png".into()],
        "flip/1".into(),
        "config/1".into(),
    )
    .unwrap();
    report.generated_at_unix = 5000;
    report.baseline_dir = Some("elsewhere/baseline".into());
    report.capture_dir = Some("elsewhere/candidate".into());
    report.entries[0].paths.capture = Some("elsewhere/scene.png".into());
    std::fs::write(&file, serde_json::to_vec(&report).unwrap()).unwrap();
    let relocated = Measurement::from_report(
        &file,
        &document,
        vec!["scene.png".into()],
        "flip/1".into(),
        "config/1".into(),
    )
    .unwrap();
    assert_eq!(original.semantic_sha256, relocated.semantic_sha256);
    assert_ne!(original.report.sha256, relocated.report.sha256);
    assert!(original.report.verify(&document).is_err());
    relocated.report.verify(&document).unwrap();
    report.entries[0].value = Some(0.09);
    assert_ne!(
        Measurement::report_identity(&report).unwrap(),
        original.semantic_sha256
    );
}

#[test]
fn provider_proposals_validate_choices_probabilities_citations_and_observation_dependence() {
    let request = request();
    let original = proposal();
    let mut response: ProviderAnswer =
        serde_json::from_str(include_str!("fixtures/evidence/provider-answer.json")).unwrap();
    response.depends_on_model_observation = false;
    let p = DecisionProposal::from_response(&request, response.clone(), original.provider.clone())
        .unwrap();
    assert!(p.response.depends_on_model_observation);
    assert!(p.response.probabilities.is_none());
    let cases: Vec<fn(&mut ProviderAnswer)> = vec![
        |r| r.answer = "approve".into(),
        |r| r.evidence_ids = vec!["invented-fact".into()],
        |r| r.request_id = Digest::of_bytes(b"wrong request"),
        |r| r.probabilities = Some(BTreeMap::from([("consistent".into(), 0.5)])),
        |r| r.probabilities = Some(BTreeMap::from([("consistent".into(), f64::INFINITY)])),
        |r| r.note = "x".repeat(601),
    ];
    for change in cases {
        let mut invalid = response.clone();
        change(&mut invalid);
        assert!(
            DecisionProposal::from_response(&request, invalid, original.provider.clone()).is_err()
        );
    }
    let mut forged = serde_json::to_value(response).unwrap();
    forged["source"] = json!("human");
    assert!(serde_json::from_value::<ProviderAnswer>(forged).is_err());
    let mut tampered = original;
    tampered.response.depends_on_model_observation = false;
    tampered.proposal_id = tampered.identity().unwrap();
    assert!(tampered.validate_for(&request).is_err());
}

#[test]
fn historical_reads_preserve_promotions_source_labels_and_unknown_authority() {
    let bytes = include_bytes!("fixtures/evidence/historical-decisions.json");
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("decisions.json");
    std::fs::write(&file, bytes).unwrap();
    let artifact = HistoricalArtifact::read(&file).unwrap();
    assert_eq!(std::fs::read(file).unwrap(), bytes);
    assert_eq!(
        artifact.raw,
        serde_json::from_slice::<Value>(bytes).unwrap()
    );
    assert_eq!(
        artifact
            .decisions
            .iter()
            .map(|d| d.authority)
            .collect::<Vec<_>>(),
        [
            Authority::ModelPromoted,
            Authority::HumanLabelUnattested,
            Authority::Unknown,
            Authority::ProposalOnly
        ]
    );
    assert_eq!(artifact.decisions[0].proposals[0]["promoted"], true);
    assert_eq!(artifact.decisions[0].disposition.as_deref(), Some("accept"));
    assert!(
        artifact
            .decisions
            .iter()
            .all(|d| d.case_binding.value().is_none())
    );
    let mut old = artifact.raw;
    old["schema"] = json!("flipdiff-decisions.v1");
    assert_eq!(
        HistoricalArtifact::from_bytes(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .schema,
        "flipdiff-decisions.v1"
    );
}

#[test]
fn historical_missingness_never_becomes_zero_background_or_abstention() {
    let artifact =
        HistoricalArtifact::from_bytes(include_bytes!("fixtures/evidence/historical-perf.json"))
            .unwrap();
    assert!(artifact.qualification.value().is_none());
    for field in [
        "/repeat_noise",
        "/semantic_labels",
        "/provider_response",
        "/timing/gpu_clock_qualified",
    ] {
        assert!(
            matches!(artifact.evidence(field), Availability::Missing { .. }),
            "{field}"
        );
    }
    assert_eq!(
        artifact.evidence("/frame/value").value(),
        Some(&json!(10.0))
    );
    let artifact = HistoricalArtifact::from_bytes(include_bytes!(
        "fixtures/evidence/historical-decisions.json"
    ))
    .unwrap();
    assert!(artifact.decisions[0].input_hashes.value().is_none());
    assert!(matches!(
        artifact.evidence("/sets/3/decision"),
        Availability::Missing { .. }
    ));
    assert!(HistoricalArtifact::from_bytes(br#"{"schema":"future-record.v1"}"#).is_err());
}

#[test]
fn review_bindings_reject_stale_inputs_attested_cli_receipts_and_tie_acceptance() {
    let mut case = case();
    case.requests.push(request());
    let decision = human();
    let mut receipt = receipt();
    receipt.validate_for(&decision, &case).unwrap();
    let attestation = HumanAttestation {
        decision_id: decision.decision_id.clone(),
        binding: decision.binding.clone(),
        audit_ref: ArtifactRef {
            path: "audit.json".into(),
            sha256: Digest::of_bytes(b"audit"),
        },
        timestamp_unix_ms: 0,
    };
    receipt.human_attestation = Some(attestation);
    assert!(receipt.validate().is_err());
    receipt.human_attestation = None;
    let mut changed = case.clone();
    changed.inputs[1].content.sha256 = Digest::of_bytes(b"new input");
    changed.refresh_id().unwrap();
    assert!(decision.validate_for(&changed).is_err());
    let mut tie = decision.clone();
    tie.disposition = Disposition::Tie;
    tie.refresh_id().unwrap();
    receipt.decision_id = tie.decision_id.clone();
    assert!(receipt.validate_for(&tie, &case).is_err());
    receipt.decision_id = decision.decision_id.clone();
    receipt.applied[0].after = None;
    assert!(receipt.validate_for(&decision, &case).is_err());
}

#[test]
fn default_result_actions_are_bounded_and_stale_preconditions_fail() {
    use saccade_core::evidence::action::*;
    let mut result: ResultEnvelope =
        serde_json::from_str(include_str!("fixtures/evidence/result.json")).unwrap();
    result.validate().unwrap();
    assert!(
        result.next_actions[0]
            .validate_for(&Digest::of_bytes(b"changed case"))
            .is_err()
    );
    let mut action = result.next_actions[0].clone();
    action.kind = ActionKind::RequestVision;
    assert!(action.validate().is_err());
    action.requires.push(Requirement::NetworkAuthorization);
    action.validate().unwrap();
    result.next_actions = vec![action; 4];
    assert!(result.validate().is_err());
    result.next_actions.clear();
    result.limits.push("x".repeat(4096));
    assert!(result.validate().is_err());
}

#[test]
fn portable_bundles_keep_markers_and_verify_relocated_content() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input with spaces.txt");
    std::fs::write(&input, b"content").unwrap();
    let document = tmp.path().join("bundle/evidence.json");
    let reference = ArtifactRef::from_file(&input, &document, false).unwrap();
    assert_eq!(reference.path, "../input with spaces.txt");
    assert!(!reference.path.contains('\\'));
    fixture("case")
        .write_bundle(document.parent().unwrap())
        .unwrap();
    assert!(document.parent().unwrap().join(".saccade-run").is_file());
    reference.verify(&document).unwrap();
    let moved = tmp.path().join("moved");
    std::fs::create_dir(&moved).unwrap();
    std::fs::rename(&input, moved.join("input with spaces.txt")).unwrap();
    std::fs::rename(document.parent().unwrap(), moved.join("bundle")).unwrap();
    let relocated = moved.join("bundle/evidence.json");
    reference.verify(&relocated).unwrap();
    Document::read(&relocated).unwrap();
    std::fs::write(moved.join("input with spaces.txt"), b"changed").unwrap();
    assert!(reference.verify(&relocated).is_err());
}
