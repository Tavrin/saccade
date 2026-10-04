//! Shared R4 fixtures for future CLI/MCP envelopes and provider bodies.
//! Writer migration belongs to R5; this checks the common serialized types.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
use saccade_core::evidence::{
    Artifact, Document, action::ResultEnvelope, human::Labels, proposal::ProviderAnswer,
};
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn shared_cli_mcp_and_provider_contracts_validate_against_the_same_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("crates/saccade-core/tests/fixtures/evidence");
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("crates/saccade-core/schemas/saccade-evidence.v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
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
        let body: Value =
            serde_json::from_slice(&std::fs::read(fixtures.join(format!("{name}.json"))).unwrap())
                .unwrap();
        validator.validate(&body).unwrap();
        let cli: Document = serde_json::from_value(body.clone()).unwrap();
        cli.validate().unwrap();
        let mcp = json!({"content":[{"type":"text","text":serde_json::to_string(&body).unwrap()}],"isError":false});
        let decoded: Document =
            serde_json::from_str(mcp["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(cli, decoded);
        if let Artifact::DecisionRequest(request) = decoded.artifact {
            // A provider uses the exact same request; no transport-specific duplicate type.
            let provider = serde_json::to_value(&request).unwrap();
            assert_eq!(provider["request_id"], body["request_id"]);
            let answer: ProviderAnswer = serde_json::from_slice(
                &std::fs::read(fixtures.join("provider-answer.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(answer.request_id, request.request_id);
        }
    }
    for (file, schema) in [
        ("result.json", "saccade-result.v2.schema.json"),
        ("labels.json", "saccade-labels.v2.schema.json"),
    ] {
        let body: Value =
            serde_json::from_slice(&std::fs::read(fixtures.join(file)).unwrap()).unwrap();
        let schema: Value = serde_json::from_slice(
            &std::fs::read(root.join("crates/saccade-core/schemas").join(schema)).unwrap(),
        )
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&body)
            .unwrap();
        if file == "result.json" {
            serde_json::from_value::<ResultEnvelope>(body)
                .unwrap()
                .validate()
                .unwrap();
        } else {
            assert_eq!(
                serde_json::from_value::<Labels>(body).unwrap().items.len(),
                1
            );
        }
    }
    // Retained historical validators still validate the same source records.
    for (file, id) in [
        ("historical-decisions.json", "saccade-decisions.v1"),
        ("historical-perf.json", "saccade-perf.v1"),
    ] {
        let body: Value =
            serde_json::from_slice(&std::fs::read(fixtures.join(file)).unwrap()).unwrap();
        let schema: Value =
            serde_json::from_str(&saccade_core::evidence::legacy::schema(id).unwrap()).unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&body)
            .unwrap();
        let old_id = id.replacen("saccade-", "flipdiff-", 1);
        let mut old = body;
        old["schema"] = json!(old_id);
        let old_schema: Value =
            serde_json::from_str(&saccade_core::evidence::legacy::schema(&old_id).unwrap())
                .unwrap();
        jsonschema::validator_for(&old_schema)
            .unwrap()
            .validate(&old)
            .unwrap();
    }
}
