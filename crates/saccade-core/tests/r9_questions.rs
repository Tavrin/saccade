//! R9 contracts: offline catalog, recorded responses, human rubrics and identity.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
#[cfg(feature = "ai")]
use saccade_core::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    case::*,
    human::*,
    request::*,
};
#[cfg(feature = "ai")]
use saccade_core::labels;
use saccade_core::questions;
use serde_json::Value;
#[cfg(feature = "ai")]
use serde_json::json;

#[cfg(feature = "ai")]
fn case() -> EvidenceCase {
    let d: Document = canonical::decode(include_bytes!("fixtures/evidence/case.json")).unwrap();
    match d.artifact {
        Artifact::Case(c) => *c,
        _ => panic!("case fixture"),
    }
}
#[cfg(feature = "ai")]
fn fact(c: &EvidenceCase, name: &str, value: Availability<FactValue>) -> Fact {
    Fact {
        id: format!("test-{name}"),
        name: name.into(),
        units: "fixture".into(),
        scope: c.scope.clone(),
        source: FactSource::Measured,
        artifact: c.measurement.report.clone(),
        source_identity: c.measurement.semantic_sha256.clone(),
        value,
        depends_on_model_observation: false,
        observation_refs: vec![],
    }
}

#[test]
fn exactly_five_schemas_define_evidence_choices_abstention_constraints_and_scoring() {
    let expected = [
        "triage.route.v1",
        "vision.route.v1",
        "perf.interpret.v1",
        "capture.disposition.v1",
        "intent.match.v1",
    ];
    assert_eq!(questions::CATALOG.map(|q| q.id), expected);
    for q in questions::CATALOG {
        assert_eq!(q.answers.len(), 5);
        assert_eq!(q.answers.last(), Some(&"abstain"));
        assert!(!q.required_evidence.is_empty());
        assert!(!q.abstention.is_empty());
        assert!(!q.deterministic_constraints.is_empty());
        assert!(!q.scoring.is_empty());
        assert_eq!(
            q.json_schema()["const"],
            serde_json::to_value(q.question()).unwrap()
        );
        assert!(q.constraints().required_citations);
    }
}

#[test]
fn removed_deferred_and_unversioned_questions_are_rejected() {
    for q in [
        "queue.priority",
        "retry.capture",
        "baseline.propose",
        "bisect.next",
        "queue.priority.v1",
        "retry.capture.v1",
        "baseline.propose.v1",
        "bisect.next.v1",
        "triage.route",
        "accept",
        "cause",
    ] {
        assert!(questions::lookup(q).is_err(), "{q}");
    }
}

#[test]
fn each_question_has_task_scoring_and_missing_responses_are_not_abstentions() {
    let fixtures: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/r9/scoring.json")).unwrap();
    assert_eq!(fixtures.len(), 5);
    for (q, f) in questions::CATALOG.iter().zip(fixtures) {
        assert_eq!(f["question"], q.id);
        let score = questions::score(
            q.id,
            f["human"].as_str().unwrap(),
            f["proposed"].as_str(),
            questions::ScoringContext {
                capture_invalid: f["capture_invalid"].as_bool(),
                unsupported_performance_claim: f["unsupported_claim"].as_bool().unwrap_or(false),
            },
        )
        .unwrap();
        assert_eq!(score.agreement, Some(false));
        assert_eq!(score.harmful_miss, f["harmful_miss"].as_bool().unwrap());
        let missing = questions::score(
            q.id,
            q.answers[0],
            None,
            questions::ScoringContext::default(),
        )
        .unwrap();
        assert_eq!(missing.agreement, None);
        assert!(!missing.abstained);
        let abstain = questions::score(
            q.id,
            q.answers[0],
            Some("abstain"),
            questions::ScoringContext::default(),
        )
        .unwrap();
        assert!(abstain.abstained);
        assert_eq!(abstain.agreement, None);
    }
    assert_eq!(
        questions::score(
            "intent.match.v1",
            "insufficient_intent",
            Some("insufficient_intent"),
            questions::ScoringContext::default()
        )
        .unwrap()
        .insufficient_intent_detected,
        Some(true)
    );
    assert_eq!(
        questions::score(
            "intent.match.v1",
            "insufficient_intent",
            None,
            questions::ScoringContext::default()
        )
        .unwrap()
        .insufficient_intent_detected,
        None
    );
}

#[cfg(feature = "ai")]
mod ai {
    use super::*;
    use saccade_core::{decision_provider::*, judge_evidence::*, review};

    fn encoded_case(id: &str) -> EvidenceCase {
        let mut c = case();
        c.validity.status = ValidityStatus::Valid;
        c.refresh_id().unwrap();
        prepare_context(&mut c).unwrap();
        for requirement in questions::lookup(id).unwrap().required_evidence {
            if !c.facts.iter().any(|f| f.name == requirement.name) {
                let value = if requirement.missing_is_evidence {
                    Availability::missing("fixture has no optional evidence")
                } else if requirement.name == "qualification" {
                    Availability::Available {
                        value: FactValue::Boolean(false),
                    }
                } else {
                    Availability::Available {
                        value: FactValue::Text("synthetic contract control".into()),
                    }
                };
                c.facts.push(fact(&c, requirement.name, value));
            }
        }
        c.refresh_id().unwrap();
        c.validate().unwrap();
        c
    }
    fn request(id: &str) -> DecisionRequest {
        review::prepare_question(&encoded_case(id), id, EncodingOptions::default()).unwrap()
    }
    fn provider() -> ProviderIdentity {
        ProviderIdentity {
            provider: "recorded".into(),
            model: "fixture-decision".into(),
            revision: Some("1".into()),
        }
    }
    fn capabilities() -> Capabilities {
        Capabilities {
            identity: provider(),
            modalities: vec![Modality::Structured],
            closed_choices: true,
            probabilities: true,
            batch_limit: 8,
            usage_reporting: true,
            retry_classes: vec![
                RetryClass::Transient,
                RetryClass::RateLimited,
                RetryClass::AuthenticationOrConfiguration,
                RetryClass::InvalidResponse,
            ],
        }
    }
    fn response(r: &DecisionRequest, answer: &str, citation: &str) -> ProviderResponse {
        let body = json!({"request_id":r.request_id,"answer":answer,"probabilities":null,"reason_codes":[],"evidence_ids":[citation],
            "missing_evidence":[],"depends_on_model_observation":false,"note":"Recorded contract response."});
        ProviderResponse::from_bytes(
            provider(),
            &canonical::bytes(r).unwrap(),
            &serde_json::to_vec(&body).unwrap(),
            None,
        )
        .unwrap()
    }
    fn enriched(r: &DecisionRequest) -> DecisionRequest {
        let mut r = r.clone();
        let bytes = b"recorded visual fact";
        let mut observation = r.evidence.facts[0].clone();
        observation.id = "observation-1".into();
        observation.name = "shadow_boundary".into();
        observation.source = FactSource::ModelObservation;
        observation.artifact = ArtifactRef {
            path: "observations.json".into(),
            sha256: Digest::of_bytes(bytes),
        };
        observation.source_identity = observation.artifact.sha256.clone();
        observation.value = Availability::Available {
            value: FactValue::Text("softer".into()),
        };
        r.evidence.observations = vec![observation];
        r.evidence.observation_context = Some(ObservationContext {
            extractor: ProviderIdentity {
                provider: "gemini".into(),
                model: "recorded-vision".into(),
                revision: Some("1".into()),
            },
            rubric_version: "visual-observation/1".into(),
            transform_identity: r.evidence.presentation_identity.clone(),
            fallback_chain_identity: Digest::of_bytes(b"chain"),
            fallback_outcome_identity: Digest::of_bytes(b"outcome"),
        });
        r.refresh_id().unwrap();
        r
    }

    #[test]
    fn required_evidence_and_deterministic_limits_are_enforced_per_question() {
        for id in questions::CATALOG.map(|q| q.id) {
            let mut r = request(id);
            questions::validate_request(&r).unwrap();
            if let Some(required) = questions::lookup(id)
                .unwrap()
                .required_evidence
                .iter()
                .find(|f| !f.missing_is_evidence)
            {
                r.evidence.facts.retain(|f| f.name != required.name);
                r.refresh_id().unwrap();
                assert!(questions::validate_request(&r).is_err());
            }
        }
        let mut r = request("capture.disposition.v1");
        questions::validate_answer(&r, "continue_review").unwrap();
        r.evidence
            .facts
            .iter_mut()
            .find(|f| f.name == "validity")
            .unwrap()
            .value = Availability::Available {
            value: FactValue::Text("invalid".into()),
        };
        r.refresh_id().unwrap();
        assert!(questions::validate_answer(&r, "continue_review").is_err());
        questions::validate_answer(&r, "recapture").unwrap();
        r.evidence
            .facts
            .iter_mut()
            .find(|f| f.name == "validity")
            .unwrap()
            .source = FactSource::Declared;
        r.refresh_id().unwrap();
        assert!(questions::validate_request(&r).is_err());
        assert!(questions::validate_answer(&request("triage.route.v1"), "likely_noise").is_err());
        assert!(
            questions::validate_answer(&request("perf.interpret.v1"), "consistent_with_intent")
                .is_err()
        );
        assert!(questions::validate_answer(&request("intent.match.v1"), "consistent").is_err());
    }

    #[test]
    fn encoder_selects_features_preserves_units_missingness_and_full_artifact_refs() {
        let mut c = encoded_case("triage.route.v1");
        c.inputs[0]
            .provenance
            .source_roots
            .push("denied-source-root".into());
        prepare_context(&mut c).unwrap();
        let r =
            review::prepare_question(&c, "triage.route.v1", EncodingOptions::default()).unwrap();
        r.validate_for(&c).unwrap();
        let FactValue::Text(provenance) = questions::feature(&r, "provenance").unwrap() else {
            panic!("provenance projection")
        };
        let provenance: Value = serde_json::from_str(provenance).unwrap();
        assert!(
            provenance["source_roots"]
                .as_array()
                .unwrap()
                .contains(&json!("denied-source-root"))
        );
        assert_eq!(r.evidence.encoder_version, ENCODER_VERSION);
        assert!(r.evidence.missing.iter().any(|m| m == "noise"));
        assert!(
            r.evidence
                .missing
                .iter()
                .any(|m| m.starts_with("omitted:fact-1; full artifact: sha256:"))
        );
        assert!(!r.evidence.facts.iter().any(|f| f.id == "fact-1"));
        assert!(
            r.evidence
                .facts
                .iter()
                .all(|f| !f.units.is_empty() && !f.artifact.path.contains('\\'))
        );
        let mut relocated = c.clone();
        for f in &mut relocated.facts {
            f.artifact.path = "moved/report.json".into();
        }
        relocated.measurement.report.path = "moved/report.json".into();
        if let Availability::Available { value } = &mut relocated.intent {
            value.provenance.paths = vec!["relocated/intent.json".into()];
            if let Some(source) = &mut value.source {
                source.path = "relocated/intent.json".into();
            }
        }
        prepare_context(&mut relocated).unwrap();
        assert_eq!(c.case_id, relocated.case_id);
        assert_eq!(
            r.request_id,
            encode(&relocated, "triage.route.v1", EncodingOptions::default())
                .unwrap()
                .request_id
        );
        let mut huge = c.clone();
        huge.facts
            .iter_mut()
            .find(|f| f.name == "deltas")
            .unwrap()
            .value = Availability::Available {
            value: FactValue::Text("x".repeat(MAX_EVIDENCE_BYTES)),
        };
        huge.refresh_id().unwrap();
        assert!(encode(&huge, "triage.route.v1", EncodingOptions::default()).is_err());
    }

    #[test]
    fn spatial_summary_preserves_sub_cent_precision_maxima_and_nonfinite_counts() {
        let mut map = vec![0.004_f32; 16 * 16];
        map[0] = 0.007;
        map[1] = f32::NAN;
        let grid = spatial_summary(&map, 16, 16).unwrap();
        assert_eq!(grid.maximum[0][0], Some(f64::from(0.007_f32)));
        assert!(grid.mean[0][0].unwrap() > 0.004 && grid.mean[0][0].unwrap() < 0.007);
        assert_eq!((grid.finite_count, grid.nonfinite_count), (255, 1));
        assert_eq!(spatial_summary(&[f32::NAN], 1, 1).unwrap().mean[7][7], None);
        assert!(spatial_summary(&map, 8, 8).is_err());
    }

    #[test]
    fn report_encoder_reuses_diagnostics_without_inventing_qualification() {
        let report: saccade_core::Report = serde_json::from_value(json!({
            "schema":"saccade-report.v1", "tool_version":"0.1.0", "generated_at_unix":0,
            "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
            "totals":{"pass":1,"fail":0,"error":0,"missing":0,"new":0,"total":1},
            "entries":[{"name":"scene.png","status":"pass","metric_used":"mean","threshold":0.01,"value":0.00000123,
                "metrics":{"mean":0.00000123,"max":0.002,"p50":0.0,"p95":0.001,"p99":0.0015,"frac_above_0_1":0.0,"frac_above_0_5":0.0,"width":16,"height":16},
                "properties":null,"paths":{},"error":null,
                "diagnostics":{"class":"local_structure","description":"recorded fixture", "signed":{"mean_delta":-0.0000123,"frac_brighter":0.1,"frac_darker":0.2,"scale":0.004},
                    "perf":[],"elapsed_ms":0.0}}]
        })).unwrap();
        let mut c = case();
        c.measurement.semantic_sha256 = Measurement::report_identity(&report).unwrap();
        c.measurement.report.sha256 = Digest::of_bytes(&canonical::bytes(&report).unwrap());
        for f in &mut c.facts {
            if f.source == FactSource::Measured {
                f.artifact = c.measurement.report.clone();
                f.source_identity = c.measurement.semantic_sha256.clone();
            }
        }
        c.refresh_id().unwrap();
        report_features(&mut c, &report).unwrap();
        prepare_context(&mut c).unwrap();
        let r = encode(&c, "triage.route.v1", EncodingOptions::default()).unwrap();
        let tone = r
            .evidence
            .facts
            .iter()
            .find(|f| f.name == "color_tone")
            .unwrap();
        let FactValue::Text(encoded) = tone.value.value().unwrap() else {
            panic!("tone projection")
        };
        let data: Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(data[0]["signed"]["mean_delta"], -0.0000123);
        assert!(
            c.facts
                .iter()
                .find(|f| f.name == "qualification")
                .unwrap()
                .value
                .value()
                .is_none()
        );
        let mut changed = report;
        changed.entries[0].threshold = 0.5;
        assert!(report_features(&mut c, &changed).is_err());
    }

    #[test]
    fn provider_contract_uses_recorded_answers_and_cannot_assign_authority_or_audit() {
        struct Recorded;
        impl DecisionProvider for Recorded {
            fn capabilities(&self) -> Capabilities {
                capabilities()
            }
            fn answer(
                &self,
                r: &DecisionRequest,
            ) -> std::result::Result<ProviderResponse, ProviderFailure> {
                Ok(response(r, "needs_eyes", "encoded-validity"))
            }
        }
        let r = request("triage.route.v1");
        let mock = Recorded;
        let proposal =
            review::record_proposal(&r, mock.answer(&r).unwrap(), &mock.capabilities()).unwrap();
        assert_eq!(proposal.response.probabilities, None);
        assert!(!proposal.response.depends_on_model_observation);
        let mut body = serde_json::to_value(&proposal.response).unwrap();
        body["provider"] = json!({"provider":"human","model":"self-assigned"});
        assert!(
            ProviderResponse::from_bytes(
                provider(),
                b"payload",
                &serde_json::to_vec(&body).unwrap(),
                None
            )
            .is_err()
        );
        let mut wrong_model = response(&r, "needs_eyes", "encoded-validity");
        wrong_model.audit.identity.model = "different-model".into();
        assert!(wrong_model.into_proposal(&r, &capabilities()).is_err());
        let mut bad = response(&r, "needs_eyes", "encoded-validity");
        bad.answer.probabilities = Some(std::collections::BTreeMap::from([(
            "needs_eyes".into(),
            1.0,
        )]));
        assert!(bad.into_proposal(&r, &capabilities()).is_err());
    }

    #[test]
    fn observations_change_request_identity_and_proposal_dependency_is_derived() {
        let direct = request("intent.match.v1");
        let r = enriched(&direct);
        questions::validate_request(&r).unwrap();
        assert_ne!(r.request_id, direct.request_id);
        let p = response(&r, "consistent", "observation-1")
            .into_proposal(&r, &capabilities())
            .unwrap();
        assert!(p.response.depends_on_model_observation);
        let mut missing = r.clone();
        missing.evidence.observation_context = None;
        missing.refresh_id().unwrap();
        assert!(questions::validate_request(&missing).is_err());
        let mut transformed = r;
        transformed
            .evidence
            .observation_context
            .as_mut()
            .unwrap()
            .transform_identity = Digest::of_bytes(b"different transform");
        transformed.refresh_id().unwrap();
        assert!(questions::validate_request(&transformed).is_err());
    }

    #[test]
    fn calibrators_bind_actual_models_rubrics_transforms_fallbacks_labels_and_splits() {
        let direct = request("intent.match.v1");
        let r = enriched(&direct);
        let cal = review::CalibratorIdentity::for_request(
            "project/1".into(),
            &r,
            provider(),
            Digest::of_bytes(b"labels"),
            Digest::of_bytes(b"split"),
        )
        .unwrap();
        cal.validate_for(&r, &provider()).unwrap();
        assert!(cal.validate_for(&direct, &provider()).is_err());
        for field in ["model", "rubric", "transform", "chain", "outcome", "policy"] {
            let mut changed = r.clone();
            let context = changed.evidence.observation_context.as_mut().unwrap();
            match field {
                "model" => context.extractor.model = "changed".into(),
                "rubric" => context.rubric_version = "changed/2".into(),
                "transform" => {
                    context.transform_identity = Digest::of_bytes(b"changed");
                    changed.evidence.presentation_identity = context.transform_identity.clone();
                }
                "chain" => context.fallback_chain_identity = Digest::of_bytes(b"changed"),
                "outcome" => context.fallback_outcome_identity = Digest::of_bytes(b"changed"),
                _ => {
                    changed.policy.insert("routing".into(), json!("changed"));
                }
            }
            changed.refresh_id().unwrap();
            assert_ne!(changed.request_id, r.request_id, "{field}");
            assert!(cal.validate_for(&changed, &provider()).is_err(), "{field}");
        }
        for field in ["project", "labels", "split", "decision_model"] {
            let mut changed = cal.clone();
            match field {
                "project" => changed.project = "other".into(),
                "labels" => changed.label_set_hash = Digest::of_bytes(b"changed"),
                "split" => changed.split_hash = Digest::of_bytes(b"changed"),
                _ => changed.decision_provider.model = "changed".into(),
            }
            assert_ne!(changed.digest().unwrap(), cal.digest().unwrap(), "{field}");
        }
    }

    pub(super) fn labeled_vision() -> (DecisionRequest, HumanDecision, labels::QuestionLabel) {
        let c = encoded_case("vision.route.v1");
        let r = encode(&c, "vision.route.v1", EncodingOptions::default()).unwrap();
        let mut binding = ReviewBinding::for_case(
            &c,
            ArtifactRef {
                path: "anonymous-review.json".into(),
                sha256: Digest::of_bytes(b"reviewed"),
            },
        )
        .unwrap();
        binding.request_ids = vec![r.request_id.clone()];
        let exposure = ReviewerExposure {
            mapping_access: Availability::Available { value: false },
            implementation_context: Availability::Available { value: false },
            reviewer: Some("human-1".into()),
        };
        let mut d = HumanDecision {
            decision_id: Digest::of_bytes(b""),
            binding,
            disposition: Disposition::NeedsWork,
            approve_deletions: false,
            channel: Channel::Cli,
            exposure: exposure.clone(),
            note: "Explicit human question label.".into(),
            timestamp_unix_ms: Some(30),
        };
        d.refresh_id().unwrap();
        let rubric = labels::VisionRoutingRubric {
            version: labels::VISION_RUBRIC_VERSION.into(),
            initial: labels::StructuredAssessment {
                packet_sha256: canonical::digest(&r.evidence).unwrap(),
                supports_disposition: labels::PixelNeed::No,
                unresolved_visual_fact: Some("shadow context outside the crop".into()),
                provisional_route: "inspect_regions".into(),
                uncertainty: "context unknown".into(),
                recorded_unix_ms: 10,
            },
            final_assessment: labels::VisualAssessment {
                requires_pixels: labels::PixelNeed::Yes,
                minimum_scope: labels::VisualScope::FullFrame,
                requires_human: false,
                necessary_visual_fact: Some("shadow belongs to an object outside the crop".into()),
                region_refs: vec!["region-1".into()],
                full_frame_seen: true,
                route: "inspect_full_frame".into(),
                recorded_unix_ms: 20,
            },
        };
        let mut label = labels::QuestionLabel {
            label_id: Digest::of_bytes(b""),
            label: Label {
                case_id: c.case_id,
                request_id: r.request_id.clone(),
                question_id: r.question.id.clone(),
                answer: "inspect_full_frame".into(),
                human_decision_id: d.decision_id.clone(),
                exposure,
            },
            saw_model_proposals: Availability::Available { value: false },
            vision_rubric: Some(rubric),
            supersedes: None,
            test_retest_of: None,
        };
        label.label_id = label.identity().unwrap();
        (r, d, label)
    }

    #[test]
    fn human_vision_rubric_retains_initial_final_scope_fact_and_structured_first_order() {
        let (r, d, label) = labeled_vision();
        label.validate_for(&r, &d).unwrap();
        let rubric = label.vision_rubric.as_ref().unwrap();
        assert_ne!(
            rubric.initial.provisional_route,
            rubric.final_assessment.route
        );
        let mut wrong = rubric.clone();
        wrong.initial.recorded_unix_ms = 20;
        assert!(wrong.validate_for(&r, "inspect_full_frame").is_err());
        wrong = rubric.clone();
        wrong.final_assessment.necessary_visual_fact = None;
        assert!(wrong.validate_for(&r, "inspect_full_frame").is_err());
        wrong = rubric.clone();
        wrong.initial.packet_sha256 = Digest::of_bytes(b"different packet");
        assert!(wrong.validate_for(&r, "inspect_full_frame").is_err());
    }

    #[test]
    fn pixel_requirement_and_route_are_scored_separately_with_undetermined_preserved() {
        let (r, _, label) = labeled_vision();
        let mut rubric = label.vision_rubric.unwrap();
        let scored = labels::score_vision(
            &rubric,
            Some("inspect_regions"),
            Some(labels::PixelNeed::Yes),
        )
        .unwrap();
        assert_eq!(scored.route_agreement, Some(false));
        assert_eq!(scored.pixel_requirement_detected, Some(true));
        rubric.final_assessment.requires_pixels = labels::PixelNeed::Undetermined;
        rubric.final_assessment.minimum_scope = labels::VisualScope::None;
        rubric.final_assessment.requires_human = true;
        rubric.final_assessment.route = "human_directly".into();
        rubric.validate_for(&r, "human_directly").unwrap();
        let scored = labels::score_vision(&rubric, Some("human_directly"), None).unwrap();
        assert_eq!(scored.route_agreement, Some(true));
        assert_eq!(scored.pixel_requirement_detected, None);
    }

    #[test]
    fn labels_bind_human_questions_and_preserve_exposure_disagreement_and_test_retest() {
        let (r, d, first) = labeled_vision();
        let mut repeated = first.clone();
        repeated.test_retest_of = Some(first.label_id.clone());
        repeated.supersedes = Some(first.label_id.clone());
        repeated
            .vision_rubric
            .as_mut()
            .unwrap()
            .final_assessment
            .requires_human = true;
        repeated
            .vision_rubric
            .as_mut()
            .unwrap()
            .final_assessment
            .route = "human_directly".into();
        repeated.label.answer = "human_directly".into();
        repeated.label_id = repeated.identity().unwrap();
        let mut unresolved = repeated.clone();
        unresolved.supersedes = None;
        unresolved.label_id = unresolved.identity().unwrap();
        let disagreement = labels::QuestionLabels {
            schema: labels::LABEL_RECORDS_SCHEMA.into(),
            kind: "human_label_records".into(),
            items: vec![first.clone(), unresolved],
        };
        assert!(
            disagreement
                .eligible_labels(std::slice::from_ref(&r), std::slice::from_ref(&d))
                .unwrap()
                .items
                .is_empty()
        );
        let set = labels::QuestionLabels {
            schema: labels::LABEL_RECORDS_SCHEMA.into(),
            kind: "human_label_records".into(),
            items: vec![first.clone(), repeated],
        };
        set.validate_for(std::slice::from_ref(&r), std::slice::from_ref(&d))
            .unwrap();
        assert_eq!(set.items.len(), 2);
        assert_eq!(
            set.eligible_labels(std::slice::from_ref(&r), std::slice::from_ref(&d))
                .unwrap()
                .items[0]
                .answer,
            "human_directly"
        );
        let tmp = tempfile::tempdir().unwrap();
        let records = tmp.path().join("label-records.json");
        let exported = tmp.path().join("labels.json");
        set.write(&records, std::slice::from_ref(&r), std::slice::from_ref(&d))
            .unwrap();
        assert_eq!(
            labels::QuestionLabels::read(
                &records,
                std::slice::from_ref(&r),
                std::slice::from_ref(&d)
            )
            .unwrap(),
            set
        );
        set.export(
            &exported,
            std::slice::from_ref(&r),
            std::slice::from_ref(&d),
        )
        .unwrap();
        let canonical_labels: Labels =
            canonical::decode(&std::fs::read(&exported).unwrap()).unwrap();
        assert_eq!(canonical_labels.schema, "saccade-labels.v2");
        canonical_labels.items[0].validate_for(&r, &d).unwrap();
        let mut exposed = first;
        exposed.saw_model_proposals = Availability::Available { value: true };
        exposed.label_id = exposed.identity().unwrap();
        exposed.validate_for(&r, &d).unwrap();
        assert!(!exposed.eligible());
        let mut stale = exposed;
        stale.label.request_id = Digest::of_bytes(b"different request");
        assert!(stale.validate_for(&r, &d).is_err());
    }
}
