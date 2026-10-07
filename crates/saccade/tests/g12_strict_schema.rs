//! Independent JSON Schema validation of the shared assist answer contract.
#![cfg(feature = "assist")]
#![allow(clippy::unwrap_used)]

use saccade_core::assist::{decode, workflow::WireAnswer};
use serde_json::{Value, json};

#[test]
fn g12_shared_schema_rejects_recorded_value_and_closed_protocol_drift() {
    let schema = saccade_core::assist::structured_output::answer_schema().unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let pilot: Value = serde_json::from_slice(include_bytes!(
        "../../saccade-core/tests/fixtures/assist-openrouter/value-geometry-pilot.json"
    ))
    .unwrap();
    let mut answer: Value =
        serde_json::from_str(pilot["choices"][0]["message"]["content"].as_str().unwrap()).unwrap();
    assert!(!validator.is_valid(&answer));
    assert!(decode::<WireAnswer>(&serde_json::to_vec(&answer).unwrap()).is_err());
    for observation in answer["observations"].as_array_mut().unwrap() {
        let geometry = observation["geometry"].as_object_mut().unwrap();
        let pixels = geometry.remove("value").unwrap();
        geometry.insert("pixels".into(), pixels);
    }
    assert!(validator.is_valid(&answer));
    let decoded: WireAnswer = decode(&serde_json::to_vec(&answer).unwrap()).unwrap();
    assert!(validator.is_valid(&serde_json::to_value(decoded).unwrap()));
    for path in ["", "/observations/0", "/observations/0/geometry"] {
        let mut invalid = answer.clone();
        invalid
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(true));
        assert!(!validator.is_valid(&invalid));
        assert!(decode::<WireAnswer>(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    for geometry in [
        json!({"type":"point","pixels":[0.1,0.2]}),
        json!({"type":"box","pixels":[0.1,0.2,0.3,0.4]}),
    ] {
        answer["observations"][0]["geometry"] = geometry;
        assert!(validator.is_valid(&answer));
        assert!(decode::<WireAnswer>(&serde_json::to_vec(&answer).unwrap()).is_ok());
    }
    for geometry in [
        json!({"type":"point","pixels":[0.1,0.2,0.3,0.4]}),
        json!({"type":"box","pixels":[0.1,0.2]}),
        json!({"type":"box","pixels":[0.1,0.2,0.3,1.1]}),
        json!({"type":"box","value":[0.1,0.2,0.3,0.4]}),
    ] {
        answer["observations"][0]["geometry"] = geometry;
        assert!(!validator.is_valid(&answer));
    }
    answer["observations"][0]["geometry"] = json!({"type":"point","pixels":[0.1,0.2]});
    for path in [
        "/outcome",
        "/observations/0/kind",
        "/observations/0/visibility",
    ] {
        let mut invalid = answer.clone();
        *invalid.pointer_mut(path).unwrap() = json!("unknown");
        assert!(!validator.is_valid(&invalid));
        assert!(decode::<WireAnswer>(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    for field in ["request_hash", "outcome", "observations"] {
        let mut invalid = answer.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&invalid));
    }
    for field in [
        "slot",
        "kind",
        "statement",
        "geometry",
        "visibility",
        "evidence_refs",
        "uncertainty",
    ] {
        let mut invalid = answer.clone();
        invalid["observations"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(!validator.is_valid(&invalid));
    }
}
