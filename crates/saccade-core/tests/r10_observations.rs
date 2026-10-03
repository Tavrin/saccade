//! Offline R10 conformance; recorded provider envelopes and generated local pixels.
#![cfg(feature = "ai")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use saccade_core::decision_provider::{DecisionProvider, ProviderFailure, RetryClass};
use saccade_core::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    case::*,
    request::ProviderIdentity,
};
use saccade_core::judge_evidence::{self, EncodingOptions, vision::*};
use saccade_core::judge_provider::observations::*;
use saccade_core::review::{self, BlindResolution, Profile};
use serde_json::{Value, json};
use std::path::Path;

struct Fixture {
    _tmp: tempfile::TempDir,
    case: EvidenceCase,
    images: [DisplayImage; 2],
}
fn fixture(width: u32, height: u32) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let document: Document =
        canonical::decode(include_bytes!("fixtures/evidence/case.json")).unwrap();
    let Artifact::Case(case) = document.artifact else {
        panic!("case fixture")
    };
    let mut case = *case;
    case.facts
        .retain(|f| f.source != FactSource::ModelObservation);
    case.inputs[0]
        .provenance
        .source_roots
        .push("denied-root-retained".into());
    let images = [0, 1].map(|i| {
        let path = tmp
            .path()
            .join(format!("private-implementation-claim-{i}.png"));
        RgbImage::from_pixel(width, height, Rgb([20 + i as u8 * 10, 10, 5]))
            .save(&path)
            .unwrap();
        case.inputs[i].content.sha256 = Digest::of_bytes(&std::fs::read(&path).unwrap());
        let Availability::Available { value: native } = &mut case.inputs[i].native_samples else {
            panic!("native")
        };
        native.width = width;
        native.height = height;
        DisplayImage::load(
            &case.inputs[i].id,
            &path,
            DisplayTransform::Srgb { background: 0 },
        )
        .unwrap()
    });
    case.refresh_id().unwrap();
    judge_evidence::prepare_context(&mut case).unwrap();
    for id in ["intent.match.v1", "vision.route.v1"] {
        for r in saccade_core::questions::lookup(id)
            .unwrap()
            .required_evidence
        {
            if !case.facts.iter().any(|f| f.name == r.name) {
                case.facts.push(Fact {
                    id: format!("fixture-{}", r.name),
                    name: r.name.into(),
                    units: "fixture".into(),
                    scope: case.scope.clone(),
                    source: FactSource::Measured,
                    artifact: case.measurement.report.clone(),
                    source_identity: case.measurement.semantic_sha256.clone(),
                    value: if r.missing_is_evidence {
                        Availability::missing("fixture has no optional feature")
                    } else {
                        Availability::Available {
                            value: FactValue::Text("synthetic conformance control".into()),
                        }
                    },
                    depends_on_model_observation: false,
                    observation_refs: vec![],
                });
            }
        }
    }
    case.refresh_id().unwrap();
    case.validate().unwrap();
    Fixture {
        _tmp: tmp,
        case,
        images,
    }
}
fn roi() -> VisionRoi {
    VisionRoi {
        region_id: "r1".into(),
        rect: [1, 2, 5, 7],
        enhance: true,
    }
}
fn presentation(f: &Fixture, task: VisionTask, swap: bool) -> VisionPresentation {
    prepare_visual(
        &f.case,
        "scene.png",
        [&f.images[0], &f.images[1]],
        &[roi()],
        task,
        swap,
    )
    .unwrap()
}
fn policy() -> FallbackPolicy {
    FallbackPolicy {
        models: vec!["gemini-primary".into(), "gemini-fallback".into()],
        version: "fallback/1".into(),
    }
}
fn actual(model: &str) -> ProviderIdentity {
    ProviderIdentity {
        provider: "gemini".into(),
        model: model.into(),
        revision: Some("vision-revision-1".into()),
    }
}
fn outcomes(model: &str) -> Vec<FallbackOutcome> {
    let mut outcomes = vec![];
    if model == "gemini-fallback" {
        outcomes.push(FallbackOutcome {
            model: "gemini-primary".into(),
            answered: false,
            failure: Some(RetryClass::Transient),
        });
    }
    outcomes.push(FallbackOutcome {
        model: model.into(),
        answered: true,
        failure: None,
    });
    outcomes
}
fn envelope(p: &VisionPresentation, preference: Option<Preference>) -> Vec<u8> {
    let observations = if p.payload.task == VisionTask::Observations {
        vec![
            serde_json::from_str::<VisualObservation>(include_str!(
                "fixtures/r10/observation.json"
            ))
            .unwrap(),
        ]
    } else {
        vec![]
    };
    let answer = VisionAnswer {
        presentation_identity: p.mapping.presentation_identity.clone(),
        observations,
        preference,
    };
    canonical::bytes(&json!({"modelVersion":"vision-revision-1", "candidates":[{"finishReason":"STOP","content":{"parts":[
        {"thought":true,"text":"discarded reasoning"}, {"text":serde_json::to_string(&answer).unwrap()}
    ]}}]})).unwrap()
}
fn decode_recorded_gemini(
    case: &EvidenceCase,
    presentation: &VisionPresentation,
    body: &[u8],
    identity: ProviderIdentity,
    policy: &FallbackPolicy,
    outcomes: &[FallbackOutcome],
    response_path: &Path,
) -> saccade_core::evidence::Result<CompletedVision> {
    decode_gemini(
        case,
        presentation,
        GeminiExchange {
            payload: &gemini_payload(case, presentation)?,
            response: body,
            identity,
            policy,
            outcomes,
            response_path,
        },
    )
}
fn completed(
    f: &Fixture,
    p: &VisionPresentation,
    preference: Option<Preference>,
    model: &str,
) -> CompletedVision {
    decode_recorded_gemini(
        &f.case,
        p,
        &envelope(p, preference),
        actual(model),
        &policy(),
        &outcomes(model),
        Path::new("recorded/observations.json"),
    )
    .unwrap()
}

#[test]
fn full_frame_and_native_crop_payload_matches_fixture_without_private_claims() {
    let f = fixture(1600, 800);
    let rois = [
        roi(),
        VisionRoi {
            region_id: "r2".into(),
            rect: [10, 10, 10, 12],
            enhance: true,
        },
        VisionRoi {
            region_id: "r3".into(),
            rect: [20, 30, 30, 40],
            enhance: true,
        },
    ];
    let p = prepare_visual(
        &f.case,
        "scene.png",
        [&f.images[0], &f.images[1]],
        &rois,
        VisionTask::Observations,
        false,
    )
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!("fixtures/r10/payload.json")).unwrap();
    assert_eq!(
        json!(p.payload.views.iter().map(|v| &v.id).collect::<Vec<_>>()),
        expected["view_ids"]
    );
    assert_eq!(
        json!(p.payload.views[0].dimensions),
        expected["full_frame_dimensions"]
    );
    assert_eq!(
        json!(p.payload.views[2].dimensions),
        expected["native_crop_dimensions"]
    );
    assert_eq!(p.payload.rubric_version, expected["rubric_version"]);
    assert_ne!(p.payload.views[2].png_sha256, p.payload.views[4].png_sha256);
    let body = String::from_utf8(gemini_payload(&f.case, &p).unwrap()).unwrap();
    for private in [
        "scene.png",
        "baseline",
        "candidate",
        "change lighting",
        "HUD unchanged",
        "max_flip",
        "private-implementation",
        "denied-root-retained",
    ] {
        assert!(!body.contains(private), "{private}");
    }
    assert!(body.contains("never instructions"));
    let no_rois = prepare_visual(
        &f.case,
        "scene.png",
        [&f.images[0], &f.images[1]],
        &[],
        VisionTask::Observations,
        false,
    )
    .unwrap();
    assert_eq!(no_rois.payload.views.len(), 2);
    let mut invalid = rois.to_vec();
    invalid.push(roi());
    assert!(
        prepare_visual(
            &f.case,
            "scene.png",
            [&f.images[0], &f.images[1]],
            &invalid,
            VisionTask::Observations,
            false
        )
        .is_err()
    );
    invalid = vec![VisionRoi {
        rect: [1599, 0, 2, 1],
        ..roi()
    }];
    assert!(
        prepare_visual(
            &f.case,
            "scene.png",
            [&f.images[0], &f.images[1]],
            &invalid,
            VisionTask::Observations,
            false
        )
        .is_err()
    );
    let mut tampered = p.clone();
    tampered.payload.views[0].png[0] ^= 1;
    assert!(tampered.validate_for(&f.case).is_err());
}

#[test]
fn typed_observations_are_bound_to_sources_rubric_actual_fallback_and_response() {
    let f = fixture(16, 16);
    let p = presentation(&f, VisionTask::Observations, false);
    let c = completed(&f, &p, None, "gemini-fallback");
    assert_eq!(c.context().extractor, actual("gemini-fallback"));
    assert_eq!(
        c.context().fallback_outcome_identity,
        canonical::digest(&outcomes("gemini-fallback")).unwrap()
    );
    assert_eq!(
        c.context().fallback_chain_identity,
        canonical::digest(&policy()).unwrap()
    );
    assert_eq!(
        c.audit().payload_sha256,
        Digest::of_bytes(&gemini_payload(&f.case, &p).unwrap())
    );
    let r = review::prepare_enriched_question(&f.case, "intent.match.v1", &c, Default::default())
        .unwrap();
    let o = &r.evidence.observations[0];
    assert_eq!(o.source, FactSource::ModelObservation);
    assert_eq!(o.source_identity, c.audit().response_sha256);
    let FactValue::Text(value) = o.value.value().unwrap() else {
        panic!("typed text")
    };
    let value: Value = serde_json::from_str(value).unwrap();
    assert_eq!(
        value["direction"],
        json!({"from_input":"baseline","to_input":"candidate"})
    );
    assert_eq!(value["visibility"], "clear");
    assert_eq!(value["source_hashes"], json!(p.sources));
    let provenance = saccade_core::questions::feature(&r, "provenance").unwrap();
    assert!(
        serde_json::to_string(provenance)
            .unwrap()
            .contains("denied-root-retained")
    );
    let mut wrong = f.case.clone();
    wrong.inputs[0].content.sha256 = Digest::of_bytes(b"changed");
    wrong.refresh_id().unwrap();
    assert!(
        review::prepare_enriched_question(&wrong, "intent.match.v1", &c, Default::default())
            .is_err()
    );
    let mut body: Value = serde_json::from_slice(&envelope(&p, None)).unwrap();
    let mut answer: Value = serde_json::from_str(
        body["candidates"][0]["content"]["parts"][1]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for mutation in [
        json!({"provider":"self-assigned"}),
        json!({"observations":[{"region_id":"unknown"}]}),
        json!({"preference":"P1"}),
    ] {
        let original = answer.clone();
        answer
            .as_object_mut()
            .unwrap()
            .extend(mutation.as_object().unwrap().clone());
        body["candidates"][0]["content"]["parts"][1]["text"] = json!(answer.to_string());
        assert!(
            decode_recorded_gemini(
                &f.case,
                &p,
                &canonical::bytes(&body).unwrap(),
                actual("gemini-primary"),
                &policy(),
                &outcomes("gemini-primary"),
                Path::new("recorded.json")
            )
            .is_err()
        );
        answer = original;
    }
    let mut empty = answer;
    empty["observations"] = json!([]);
    body["candidates"][0]["content"]["parts"][1]["text"] = json!(empty.to_string());
    let empty = decode_recorded_gemini(
        &f.case,
        &p,
        &canonical::bytes(&body).unwrap(),
        actual("gemini-primary"),
        &policy(),
        &outcomes("gemini-primary"),
        Path::new("empty.json"),
    )
    .unwrap();
    assert!(empty.observations().is_empty());
    assert!(
        review::prepare_enriched_question(&f.case, "intent.match.v1", &empty, Default::default())
            .is_err()
    );
}

#[test]
fn recorded_hdr_transform_changes_presentation_request_and_calibration_identity() {
    let mut f = fixture(16, 16);
    let path = f._tmp.path().join("source.exr");
    image::Rgb32FImage::from_pixel(16, 16, image::Rgb([0.4, 0.2, 0.1]))
        .save(&path)
        .unwrap();
    for input in &mut f.case.inputs {
        input.content.sha256 = Digest::of_bytes(&std::fs::read(&path).unwrap());
    }
    f.case.refresh_id().unwrap();
    judge_evidence::prepare_context(&mut f.case).unwrap();
    let transform = |tm: &str| DisplayTransform::Hdr {
        version: "hdr-display/1".into(),
        tonemapper: tm.into(),
        exposure_stops: 0.0,
    };
    let images = ["baseline", "candidate"]
        .map(|id| DisplayImage::load(id, &path, transform("aces")).unwrap());
    let a = prepare_visual(
        &f.case,
        "scene.png",
        [&images[0], &images[1]],
        &[roi()],
        VisionTask::Observations,
        false,
    )
    .unwrap();
    let images = ["baseline", "candidate"]
        .map(|id| DisplayImage::load(id, &path, transform("hable")).unwrap());
    let b = prepare_visual(
        &f.case,
        "scene.png",
        [&images[0], &images[1]],
        &[roi()],
        VisionTask::Observations,
        false,
    )
    .unwrap();
    assert_eq!(a.payload.views[0].display_transform, transform("aces"));
    assert_ne!(
        a.mapping.presentation_identity,
        b.mapping.presentation_identity
    );
    let a = review::prepare_enriched_question(
        &f.case,
        "intent.match.v1",
        &completed(&f, &a, None, "gemini-primary"),
        Default::default(),
    )
    .unwrap();
    let b = review::prepare_enriched_question(
        &f.case,
        "intent.match.v1",
        &completed(&f, &b, None, "gemini-primary"),
        Default::default(),
    )
    .unwrap();
    assert_ne!(a.request_id, b.request_id);
    assert_ne!(
        a.calibration_identity().unwrap(),
        b.calibration_identity().unwrap()
    );
    assert!(
        DisplayImage::load("baseline", &path, DisplayTransform::Srgb { background: 0 }).is_err()
    );
    let mut invalid = transform("aces");
    if let DisplayTransform::Hdr { exposure_stops, .. } = &mut invalid {
        *exposure_stops = 1.0;
    }
    assert!(DisplayImage::load("baseline", &path, invalid).is_err());
}

#[test]
fn both_orders_remap_preferences_and_direction_without_position_bias() {
    let f = fixture(16, 16);
    let ab = presentation(&f, VisionTask::BlindPreference, false);
    let ba = presentation(&f, VisionTask::BlindPreference, true);
    let a = completed(&f, &ab, Some(Preference::P2), "gemini-primary");
    let b = completed(&f, &ba, Some(Preference::P1), "gemini-primary");
    assert_eq!(
        review::resolve_blind_orders(&f.case, &a, &b).unwrap(),
        BlindResolution::Preferred {
            input_id: "candidate".into()
        }
    );
    let conflict = completed(&f, &ba, Some(Preference::P2), "gemini-primary");
    assert_eq!(
        review::resolve_blind_orders(&f.case, &a, &conflict).unwrap(),
        BlindResolution::NeedsHuman {
            reason: "contradictory_presentation_orders".into()
        }
    );
    assert!(matches!(
        review::resolve_blind_orders(&f.case, &a, &a).unwrap(),
        BlindResolution::NeedsHuman { .. }
    ));
    let ba = presentation(&f, VisionTask::Observations, true);
    let request = review::prepare_enriched_question(
        &f.case,
        "intent.match.v1",
        &completed(&f, &ba, None, "gemini-primary"),
        Default::default(),
    )
    .unwrap();
    let FactValue::Text(o) = request.evidence.observations[0].value.value().unwrap() else {
        panic!("observation")
    };
    let o: Value = serde_json::from_str(o).unwrap();
    assert_eq!(
        o["direction"],
        json!({"from_input":"candidate","to_input":"baseline"})
    );
}

#[test]
fn mixed_actual_models_or_revisions_remain_unresolved() {
    let f = fixture(16, 16);
    let ab = presentation(&f, VisionTask::BlindPreference, false);
    let ba = presentation(&f, VisionTask::BlindPreference, true);
    let a = completed(&f, &ab, Some(Preference::P2), "gemini-primary");
    let b = completed(&f, &ba, Some(Preference::P1), "gemini-fallback");
    assert_eq!(
        review::resolve_blind_orders(&f.case, &a, &b).unwrap(),
        BlindResolution::NeedsHuman {
            reason: "mixed_actual_models".into()
        }
    );
    let mut body: Value = serde_json::from_slice(&envelope(&ba, Some(Preference::P1))).unwrap();
    body["modelVersion"] = json!("vision-revision-2");
    let mut id = actual("gemini-primary");
    id.revision = None;
    let b = decode_recorded_gemini(
        &f.case,
        &ba,
        &canonical::bytes(&body).unwrap(),
        id,
        &policy(),
        &outcomes("gemini-primary"),
        Path::new("revision-2.json"),
    )
    .unwrap();
    assert!(matches!(
        review::resolve_blind_orders(&f.case, &a, &b).unwrap(),
        BlindResolution::NeedsHuman { .. }
    ));
    assert!(
        decode_recorded_gemini(
            &f.case,
            &ba,
            &envelope(&ba, Some(Preference::P1)),
            actual("gemini-primary"),
            &policy(),
            &outcomes("gemini-fallback"),
            Path::new("wrong.json")
        )
        .is_err()
    );
}

#[test]
fn ties_and_abstentions_never_select_or_accept_a_candidate() {
    let f = fixture(16, 16);
    let ab = presentation(&f, VisionTask::BlindPreference, false);
    let ba = presentation(&f, VisionTask::BlindPreference, true);
    let a = completed(&f, &ab, Some(Preference::Tie), "gemini-primary");
    let b = completed(&f, &ba, Some(Preference::Tie), "gemini-primary");
    assert_eq!(
        review::resolve_blind_orders(&f.case, &a, &b).unwrap(),
        BlindResolution::Tie
    );
    let b = completed(&f, &ba, Some(Preference::Abstain), "gemini-primary");
    assert!(matches!(
        review::resolve_blind_orders(&f.case, &a, &b).unwrap(),
        BlindResolution::NeedsHuman { .. }
    ));
    assert!(
        review::prepare_enriched_question(&f.case, "intent.match.v1", &a, Default::default())
            .is_err()
    );
    assert_eq!(review::candidate_answer("tie"), "needs_human");
}

struct RecordedJev;
impl DecisionTransport for RecordedJev {
    fn exchange(
        &self,
        payload: &[u8],
    ) -> std::result::Result<
        (Vec<u8>, Option<saccade_core::decision_provider::Usage>),
        ProviderFailure,
    > {
        let body: Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(body["state"]["question"]["id"], "intent.match.v1");
        assert_eq!(
            body["state"]["evidence"]["observations"][0]["source"],
            "model_observation"
        );
        Ok((include_bytes!("fixtures/r10/jev.json").to_vec(), None))
    }
}
#[test]
fn enriched_jev_proposals_propagate_observation_dependence_without_confidence_multiplication() {
    let f = fixture(16, 16);
    let p = presentation(&f, VisionTask::Observations, false);
    let c = completed(&f, &p, None, "gemini-fallback");
    let request =
        review::prepare_enriched_question(&f.case, "intent.match.v1", &c, Default::default())
            .unwrap();
    let adapter = JevAdapter {
        identity: ProviderIdentity {
            provider: "jev".into(),
            model: "jev-fixture".into(),
            revision: Some("jev-fixture-revision-1".into()),
        },
        transport: &RecordedJev,
    };
    let response = adapter.answer(&request).unwrap();
    let proposal =
        review::record_proposal(&request, response.clone(), &adapter.capabilities()).unwrap();
    assert!(proposal.response.depends_on_model_observation);
    assert!(
        proposal
            .response
            .evidence_ids
            .contains(&request.evidence.observations[0].id)
    );
    assert_eq!(
        proposal.response.probabilities.as_ref().unwrap()["partially_consistent"],
        0.65
    );
    assert_eq!(
        proposal.provider.response_sha256,
        Digest::of_bytes(include_bytes!("fixtures/r10/jev.json"))
    );
    let direct =
        review::prepare_question(&f.case, "intent.match.v1", EncodingOptions::default()).unwrap();
    assert_ne!(direct.request_id, request.request_id);
    assert_ne!(
        direct.calibration_identity().unwrap(),
        request.calibration_identity().unwrap()
    );
    let payload = jev_payload(&request, "jev-fixture").unwrap();
    let mut missing_probs: Value =
        serde_json::from_slice(include_bytes!("fixtures/r10/jev.json")).unwrap();
    missing_probs["answers"]["q"]["probabilities"] = Value::Null;
    assert!(
        decode_jev(
            &request,
            &payload,
            &canonical::bytes(&missing_probs).unwrap(),
            adapter.identity.clone(),
            None
        )
        .unwrap()
        .answer
        .probabilities
        .is_none()
    );
    missing_probs["answers"]["q"]["probabilities"] = json!({"partially_consistent":1.0});
    assert!(
        decode_jev(
            &request,
            &payload,
            &canonical::bytes(&missing_probs).unwrap(),
            adapter.identity.clone(),
            None
        )
        .is_err()
    );
    struct Failed;
    impl DecisionTransport for Failed {
        fn exchange(
            &self,
            _: &[u8],
        ) -> std::result::Result<
            (Vec<u8>, Option<saccade_core::decision_provider::Usage>),
            ProviderFailure,
        > {
            Err(ProviderFailure {
                class: RetryClass::RateLimited,
                message: "recorded rate limit".into(),
                retry_after_secs: Some(2),
            })
        }
    }
    let failed = JevAdapter {
        identity: adapter.identity,
        transport: &Failed,
    };
    assert_eq!(
        failed.answer(&request).unwrap_err().class,
        RetryClass::RateLimited
    );
}

#[test]
fn profiles_plan_authorized_vision_separately_from_blind_intent_matching() {
    let f = fixture(16, 16);
    let r =
        review::prepare_question(&f.case, "vision.route.v1", EncodingOptions::default()).unwrap();
    let answer = saccade_core::evidence::proposal::ProviderAnswer {
        request_id: r.request_id.clone(),
        answer: "inspect_regions".into(),
        probabilities: None,
        reason_codes: vec![],
        evidence_ids: vec![r.evidence.facts[0].id.clone()],
        missing_evidence: vec![],
        depends_on_model_observation: false,
        note: "Recorded routing".into(),
    };
    let proposal = saccade_core::evidence::proposal::DecisionProposal::from_response(
        &r,
        answer,
        saccade_core::evidence::proposal::ProviderAudit {
            identity: ProviderIdentity {
                provider: "jev".into(),
                model: "fixture".into(),
                revision: None,
            },
            payload_sha256: Digest::of_bytes(b"payload"),
            response_sha256: Digest::of_bytes(b"response"),
            execution_ref: None,
            timestamp_unix_ms: None,
        },
    )
    .unwrap();
    assert!(
        Profile::load("triage")
            .unwrap()
            .visual_tasks(None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        Profile::load("triage")
            .unwrap()
            .visual_tasks(Some((&r, &proposal)))
            .unwrap(),
        vec![VisionTask::Observations]
    );
    assert_eq!(
        Profile::load("lookdev")
            .unwrap()
            .visual_tasks(None)
            .unwrap(),
        vec![VisionTask::Observations, VisionTask::BlindPreference]
    );
    assert!(
        Profile::load("ci")
            .unwrap()
            .visual_tasks(Some((&r, &proposal)))
            .unwrap()
            .is_empty()
    );
}
