//! Offline validation of the incremental-index persistence and receipt contracts.
#![allow(clippy::unwrap_used)]
use saccade_core::general::{embedding, embedding_index};
use serde_json::{Value, json};

#[test]
fn segmented_manifest_and_update_receipt_match_embedded_schemas() {
    let model=embedding::parse_model(&serde_json::to_vec(&json!({"schema":embedding::MODEL_SCHEMA,"family":"dinov2-small","artifact":{"role":"embedding","version":"a".repeat(40),"format":"onnx","url":format!("https://example.org/{}/model.onnx","a".repeat(40)),"bytes":1,"sha256":"a".repeat(64),"license":"Apache-2.0"},"input":"pixels","output":"embedding","size":[8,8],"mean":[0.,0.,0.],"std":[1.,1.,1.],"dimensions":2,"calibration":null})).unwrap()).unwrap();
    let id = saccade_core::localized::digest(&serde_json::to_vec(&model).unwrap());
    let d = tempfile::tempdir().unwrap();
    let mut u = embedding_index::Update::begin(d.path(), model).unwrap();
    u.upsert(&id, "generated.png", &"a".repeat(64), vec![1., 0.])
        .unwrap();
    let receipt = u.commit().unwrap();
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(d.path().join(format!("{}.json", embedding_index::SCHEMA))).unwrap(),
    )
    .unwrap();
    for (id, value) in [
        (embedding_index::SCHEMA, manifest),
        (embedding_index::UPDATE_SCHEMA, receipt),
    ] {
        let schema: Value =
            serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&value), "{value}");
        let mut corrupt = value.clone();
        corrupt["model_contract_sha256"] = json!("invalid");
        assert!(!validator.is_valid(&corrupt));
    }
}
