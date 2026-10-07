//! One closed answer schema for Gemini, OpenRouter and offline stage-2 plans.
use super::{Result, decode};
use serde_json::{Value, json};

/// Pinned projection: remove only schema array-cardinality keywords recursively.
pub const PROJECTION_POLICY: &str = "assist-openrouter-drop-array-bounds/1";
/// Version in the wire name binds the projection rule into every payload hash.
pub const PROJECTED_SCHEMA_NAME: &str = "saccade_assist_answer_drop_array_bounds_v1";

/// Shared JSON Schema; semantic, citation and geometry checks remain local.
pub fn answer_schema() -> Result<Value> {
    decode(include_bytes!("answer.schema.json"))
}

/// Provider-compatible schema; local decoding keeps the full shared contract.
pub fn openrouter_schema() -> Result<Value> {
    fn project(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("minItems");
                map.remove("maxItems");
                for child in map.values_mut() {
                    project(child);
                }
            }
            Value::Array(array) => array.iter_mut().for_each(project),
            _ => {}
        }
    }
    let mut schema = answer_schema()?;
    project(&mut schema);
    Ok(schema)
}

/// Validate every wire answer against the full schema, including removed bounds.
/// This closed evaluator supports exactly the keywords in our pinned schema.
pub fn validate_answer(bytes: &[u8]) -> Result<()> {
    fn matches(value: &Value, schema: &Value) -> bool {
        let Some(map) = schema.as_object() else {
            return false;
        };
        if map.keys().any(|key| {
            ![
                "type",
                "additionalProperties",
                "required",
                "properties",
                "items",
                "enum",
                "anyOf",
                "minItems",
                "maxItems",
                "minimum",
                "maximum",
            ]
            .contains(&key.as_str())
        }) {
            return false;
        }
        if let Some(variants) = schema.get("anyOf") {
            return variants
                .as_array()
                .is_some_and(|a| a.iter().any(|s| matches(value, s)));
        }
        if schema
            .get("enum")
            .is_some_and(|choices| choices.as_array().is_none_or(|a| !a.contains(value)))
        {
            return false;
        }
        match schema["type"].as_str() {
            Some("object") => {
                let (Some(object), Some(properties), Some(required)) = (
                    value.as_object(),
                    schema["properties"].as_object(),
                    schema["required"].as_array(),
                ) else {
                    return false;
                };
                schema["additionalProperties"] == false
                    && required
                        .iter()
                        .all(|key| key.as_str().is_some_and(|k| object.contains_key(k)))
                    && object
                        .iter()
                        .all(|(key, child)| properties.get(key).is_some_and(|s| matches(child, s)))
            }
            Some("array") => value.as_array().is_some_and(|a| {
                schema
                    .get("minItems")
                    .is_none_or(|n| n.as_u64().is_some_and(|n| a.len() as u64 >= n))
                    && schema
                        .get("maxItems")
                        .is_none_or(|n| n.as_u64().is_some_and(|n| a.len() as u64 <= n))
                    && a.iter().all(|child| matches(child, &schema["items"]))
            }),
            Some("string") => value.is_string(),
            Some("number") => value.as_f64().is_some_and(|n| {
                n.is_finite()
                    && schema
                        .get("minimum")
                        .is_none_or(|v| v.as_f64().is_some_and(|min| n >= min))
                    && schema
                        .get("maximum")
                        .is_none_or(|v| v.as_f64().is_some_and(|max| n <= max))
            }),
            _ => false,
        }
    }
    let value: Value = decode(bytes)?;
    super::require(
        matches(&value, &answer_schema()?),
        "closed schema or JSON violation",
    )
}

/// Exact strict OpenRouter format, pinned before authorization or accounting.
pub fn openrouter_format() -> Result<Value> {
    Ok(json!({"type":"json_schema","json_schema":{
        "name":PROJECTED_SCHEMA_NAME,"strict":true,"schema":openrouter_schema()?
    }}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn g12_projection_drops_exactly_array_bounds_and_keeps_full_local_schema() {
        fn compare(full: &Value, projected: &Value) -> usize {
            match full {
                Value::Object(map) => {
                    let actual = projected.as_object().unwrap();
                    let removed = map
                        .keys()
                        .filter(|k| ["minItems", "maxItems"].contains(&k.as_str()))
                        .count();
                    assert_eq!(actual.len(), map.len() - removed);
                    removed
                        + map
                            .iter()
                            .filter(|(k, _)| !["minItems", "maxItems"].contains(&k.as_str()))
                            .map(|(key, child)| compare(child, &actual[key]))
                            .sum::<usize>()
                }
                Value::Array(a) => {
                    let b = projected.as_array().unwrap();
                    assert_eq!(a.len(), b.len());
                    a.iter().zip(b).map(|(a, b)| compare(a, b)).sum()
                }
                _ => {
                    assert_eq!(full, projected);
                    0
                }
            }
        }
        assert_eq!(PROJECTION_POLICY, "assist-openrouter-drop-array-bounds/1");
        assert_eq!(
            compare(&answer_schema().unwrap(), &openrouter_schema().unwrap()),
            6
        );
        assert_eq!(
            answer_schema().unwrap()["properties"]["observations"]["maxItems"],
            64
        );
    }

    #[test]
    fn g12_full_local_schema_refuses_too_long_arrays_and_preserves_other_keywords() {
        let mut answer = json!({"request_hash":"fixture","outcome":"observed","observations":[{
            "slot":"P1","kind":"appearance","statement":"appearance:changed",
            "geometry":{"type":"box","pixels":[0.1,0.1,0.5,0.5]},"visibility":"visible",
            "evidence_refs":["P1:R0"],"uncertainty":0.1
        }]});
        validate_answer(&serde_json::to_vec(&answer).unwrap()).unwrap();
        let valid = answer.clone();
        for (path, value) in [
            (
                "/observations/0/geometry/pixels",
                json!([0.1, 0.1, 0.5, 0.5, 0.0]),
            ),
            ("/observations/0/geometry/pixels", json!([0.1, 0.1, 0.5])),
            ("/observations/0/evidence_refs", json!([])),
            ("/observations/0/uncertainty", json!(1.1)),
            ("/observations/0/slot", json!("P3")),
        ] {
            answer = valid.clone();
            *answer.pointer_mut(path).unwrap() = value;
            assert!(validate_answer(&serde_json::to_vec(&answer).unwrap()).is_err());
        }
        answer = valid.clone();
        answer["observations"] = json!(vec![valid["observations"][0].clone(); 65]);
        assert!(validate_answer(&serde_json::to_vec(&answer).unwrap()).is_err());
        answer["observations"] = json!(vec![valid["observations"][0].clone(); 64]);
        validate_answer(&serde_json::to_vec(&answer).unwrap()).unwrap();
    }
}
