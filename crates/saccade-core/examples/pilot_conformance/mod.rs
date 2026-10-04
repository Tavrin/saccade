//! Real encoder/adapter replay with explicitly synthetic recorded responses.
use saccade_core::evidence::{Artifact, Document, canonical, request::ProviderIdentity};
use saccade_core::judge_evidence::vision::*;
use saccade_core::judge_provider::observations::*;
use serde_json::json;
use std::path::Path;

pub fn run(folder: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let doc = Document::read(&folder.join("case.json"))?;
    let Artifact::Case(c) = doc.artifact else {
        return Err("missing case".into());
    };
    let entry = c.scope.entries.first().ok_or("missing view")?;
    let load = |role: &str| -> Result<DisplayImage, Box<dyn std::error::Error>> {
        let id = format!("{role}:{entry}");
        let input = c
            .inputs
            .iter()
            .find(|i| i.id == id)
            .ok_or("missing source")?;
        let file = saccade_core::paths::resolve(
            &input.content.path,
            &folder
                .parent()
                .ok_or("missing folder")?
                .join("measurement/evidence.json"),
        );
        Ok(DisplayImage::load(
            &id,
            &file,
            DisplayTransform::Srgb { background: 128 },
        )?)
    };
    let a = load("baseline")?;
    let b = load("capture")?;
    let policy = FallbackPolicy {
        models: vec!["gemini-3.8-flash".into()],
        version: "pinned-pilot/1".into(),
    };
    let outcomes = [FallbackOutcome {
        model: policy.models[0].clone(),
        answered: true,
        failure: None,
    }];
    let mut cells = Vec::new();
    for swap in [false, true] {
        let p = prepare_visual(&c, entry, [&a, &b], &[], VisionTask::Observations, swap)?;
        let payload = gemini_payload(&c, &p)?;
        let answer = json!({"presentation_identity":p.mapping.presentation_identity,"preference":null,
            "observations":[{"region_id":"full_frame","observation":"mock_difference","change":"synthetic control only",
                "visibility":"clear","evidence_refs":["full-frame-P1","full-frame-P2"],"uncertainty":"not human truth"}]});
        let body = canonical::bytes(
            &json!({"modelVersion":"mock-revision/1","candidates":[{"finishReason":"STOP",
            "content":{"parts":[{"text":serde_json::to_string(&answer)?}]}}]}),
        )?;
        let completed = decode_gemini(
            &c,
            &p,
            GeminiExchange {
                payload: &payload,
                response: &body,
                identity: ProviderIdentity {
                    provider: "gemini".into(),
                    model: policy.models[0].clone(),
                    revision: Some("mock-revision/1".into()),
                },
                policy: &policy,
                outcomes: &outcomes,
                response_path: Path::new("synthetic-observation-response.json"),
            },
        )?;
        for schema in saccade_core::questions::CATALOG {
            let q = schema.id;
            let Ok(direct) = saccade_core::review::prepare_question(&c, q, Default::default())
            else {
                continue;
            };
            if ["triage.route.v1", "vision.route.v1", "intent.match.v1"].contains(&q) {
                let gemini_question = super::pilot_gemini::payload(&c, &p, &direct)?;
                let mock = json!({"request_id":direct.request_id,"presentation_identity":p.mapping.presentation_identity,
                    "answer":"abstain","requires_pixels":"undetermined"});
                let envelope = canonical::bytes(
                    &json!({"modelVersion":"mock-revision/1","candidates":[{"finishReason":"STOP",
                    "content":{"parts":[{"text":serde_json::to_string(&mock)?}]}}]}),
                )?;
                let decoded = super::pilot_gemini::decode(&direct, &p, &envelope)?;
                cells.push(json!({"question":q,"order":if swap{"ba"}else{"ab"},"encoding":"gemini_alone",
                    "adapter":super::pilot_gemini::VERSION,"payload_sha256":canonical::Digest::of_bytes(&gemini_question),"answer":decoded["answer"]}));
            }
            let enriched = saccade_core::review::prepare_enriched_question(
                &c,
                q,
                &completed,
                Default::default(),
            )?;
            let jev = jev_payload(&enriched, "jev-latest")?;
            let response = canonical::bytes(
                &json!({"model":"jev-latest","answers":{"q":{"choice":"abstain"}}}),
            )?;
            let decoded = decode_jev(
                &enriched,
                &jev,
                &response,
                ProviderIdentity {
                    provider: "jev".into(),
                    model: "jev-latest".into(),
                    revision: None,
                },
                None,
            )?;
            if direct.request_id == enriched.request_id
                || !decoded.answer.depends_on_model_observation
            {
                return Err("enrichment identity/dependency was lost".into());
            }
            cells.push(json!({"question":q,"order":if swap{"ba"}else{"ab"},"direct_request_sha256":canonical::digest(&direct)?,
                "enriched_request_sha256":canonical::digest(&enriched)?,"gemini_payload_sha256":canonical::Digest::of_bytes(&payload),
                "jev_payload_sha256":canonical::Digest::of_bytes(&jev),"outcome":"abstained","depends_on_model_observation":true}));
        }
    }
    Ok(
        json!({"synthetic":true,"real_support":0,"provider_calls":0,"both_orders":true,"adapter_round_trip":true,"cells":cells}),
    )
}
