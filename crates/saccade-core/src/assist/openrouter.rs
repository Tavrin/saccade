//! Explicit OpenRouter chat-completions dialect. Live dispatch requires fresh provider accounting.
use super::{Result, decode, require, workflow::WireAnswer};
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
/// Billing source: OpenRouter passes provider prices through without markup.
pub const PRICE_VERSION: &str = "openrouter-recorded-prices/2026-10-06-v1";
/// Pinned strict answer schema and unchanged minimal unified reasoning budgets.
pub const REQUEST_POLICY: &str = "assist-openrouter-provider-schema/1";
/// Keep at least three quarters of the aggregate completion budget for visible output.
/// Missing/unknown task data receives the bounded multi-view policy.
pub fn reasoning_budget(task: Option<&str>, output: u64) -> u64 {
    (if task == Some("check_ui") { 512 } else { 1024 }).min(output / 4)
}
fn request_task(v: &Value) -> Option<String> {
    v["messages"]
        .as_array()?
        .iter()
        .filter(|m| m["role"] == "user")
        .filter_map(|m| m["content"].as_array())
        .flatten()
        .filter(|p| p["type"] == "text")
        .filter_map(|p| serde_json::from_str::<Value>(p["text"].as_str()?).ok())
        .find_map(|data| data["task"].as_str().map(str::to_owned))
}
/// Fixed API dialect; credentials can never be redirected by a project.
pub const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Sanitized HTTP diagnostics; raw provider text is never retained.
#[derive(Debug, Clone, Serialize)]
pub struct HttpError {
    /// Observed HTTP status.
    pub http_status: u16,
    /// Bounded OpenRouter numeric or token code, when well formed.
    pub openrouter_error_code: Option<Value>,
    /// Bounded routing name; spaces are allowed for names such as Google AI Studio.
    pub provider_name: Option<String>,
    /// Bounded provider status token from the embedded error envelope.
    pub provider_status: Option<String>,
    /// An explicit error-only 4xx response with no generation or billing evidence.
    pub zero_cost_refused: bool,
}
/// Classify an already reflection-checked, bounded HTTP rejection without logging it.
pub fn http_error(status: Option<u16>, body: &[u8]) -> Option<HttpError> {
    let status = status.filter(|n| (300..600).contains(n))?;
    let value = decode::<Value>(body).unwrap_or(Value::Null);
    let error = &value["error"];
    let token = |value: &Value, spaces: bool| {
        value
            .as_str()
            .filter(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || b"-_.:/".contains(&b) || (spaces && b == b' ')
                    })
            })
            .map(str::to_owned)
    };
    let code = if error["code"].as_u64().is_some_and(|n| n <= 999_999) {
        Some(error["code"].clone())
    } else {
        token(&error["code"], false).map(Value::String)
    };
    let raw = error["metadata"].get("raw");
    let embedded = raw
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 8192)
        .and_then(|s| decode::<Value>(s.as_bytes()).ok());
    // Any identifier, usage or completion evidence makes zero settlement ambiguous.
    fn generation_evidence(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().any(|(key, child)| {
                [
                    "id",
                    "generation_id",
                    "usage",
                    "choices",
                    "cost",
                    "total_cost",
                ]
                .contains(&key.as_str())
                    || generation_evidence(child)
            }),
            Value::Array(array) => array.iter().any(generation_evidence),
            _ => false,
        }
    }
    let zero_cost_refused = (400..500).contains(&status)
        && error.is_object()
        && code.is_some()
        && !generation_evidence(&value)
        && raw.is_none_or(|_| {
            embedded
                .as_ref()
                .is_some_and(|v| v["error"].is_object() && !generation_evidence(v))
        });
    Some(HttpError {
        http_status: status,
        openrouter_error_code: code,
        provider_name: token(&error["metadata"]["provider_name"], true),
        provider_status: embedded
            .as_ref()
            .and_then(|v| token(&v["error"]["status"], false)),
        zero_cost_refused,
    })
}
/// Build chat messages from the same anonymous image extraction packet.
pub fn request(gemini: &[u8], model: &str) -> Result<Vec<u8>> {
    let price = super::price::openrouter_price(model)?;
    require(
        !model.is_empty()
            && model.len() <= 128
            && model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b)),
        "OpenRouter model ID",
    )?;
    let source: Value = decode(gemini)?;
    if let Some(schema) = source["generationConfig"].get("responseJsonSchema") {
        require(
            *schema == super::structured_output::answer_schema()?,
            "OpenRouter source answer schema drift",
        )?;
    }
    let parts = source["contents"][0]["parts"]
        .as_array()
        .ok_or(super::Error::Invalid("image packet"))?;
    let mut content = Vec::new();
    for part in parts {
        if let Some(text) = part["text"].as_str() {
            content.push(json!({"type":"text","text":text}));
        } else {
            let media = part
                .get("inline_data")
                .or_else(|| part.get("inlineData"))
                .ok_or(super::Error::Invalid("image media"))?;
            let data = media["data"]
                .as_str()
                .ok_or(super::Error::Invalid("image data"))?;
            content.push(json!({"type":"image_url","image_url":{"url":format!("data:image/png;base64,{data}")}}));
        }
    }
    let mut request = json!({"model":model,"messages":[{"role":"system","content":source["systemInstruction"]["parts"][0]["text"]},{"role":"user","content":content}],"temperature":0,"max_tokens":4096,"response_format":super::structured_output::openrouter_format_for(model)?,"provider":{"allow_fallbacks":false,"require_parameters":true,"max_price":price.max_price()},"usage":{"include":true}});
    request["reasoning"] = json!({"max_tokens":reasoning_budget(request_task(&request).as_deref(), super::execution::OUTPUT_LIMIT)});
    crate::evidence::canonical::bytes(&request)
        .map_err(|_| super::Error::Invalid("OpenRouter payload"))
}
/// Recorded execution identity and routing, distinct from immutable model identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Actual execution ID, required even on a semantically unusable answer.
    pub request_id: String,
    /// Returned routed model; never substituted from the request.
    pub model: String,
    /// Returned provider routing field.
    pub provider: String,
    /// Alias-bound and time-specific identity, never claimed immutable.
    pub revision: String,
    /// Complete OpenRouter usage, including provider billed USD cost.
    pub usage: Value,
    /// Billing source and versioned offline price contract.
    pub billing_source: String,
}
/// Bounded, charset-validated identity from an already secret-checked response.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseIdentity {
    /// Actual returned model, including on quarantined drift.
    pub returned_model: String,
    /// Null means the field was omitted or null, never malformed or empty.
    pub system_fingerprint: Option<String>,
    /// Fingerprint or the reserved explicit absence marker `absent`.
    pub returned_revision: String,
    /// Absence remains observable even when explicitly pinned and accepted.
    pub code: &'static str,
}
/// Validate metadata before retaining it in receipts; never retain rejected text.
pub fn response_identity(value: &Value) -> Result<ResponseIdentity> {
    let valid = |s: &str, limit| {
        !s.is_empty()
            && s.len() <= limit
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
    };
    let model = value["model"]
        .as_str()
        .filter(|s| valid(s, 128))
        .ok_or(super::Error::Invalid("invalid OpenRouter returned model"))?;
    let fingerprint = match value.get("system_fingerprint") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if valid(s, 256) && s != "absent" => Some(s.clone()),
        _ => return Err(super::Error::Invalid("invalid OpenRouter fingerprint")),
    };
    Ok(ResponseIdentity {
        returned_model: model.into(),
        returned_revision: fingerprint.clone().unwrap_or_else(|| "absent".into()),
        code: if fingerprint.is_some() {
            "openrouter_fingerprint_present"
        } else {
            "openrouter_fingerprint_absent"
        },
        system_fingerprint: fingerprint,
    })
}
// A dated pin binds the requested alias to an explicit eight-digit revision.
pub(crate) fn dated_pin(model: &str, revision: &str) -> bool {
    revision
        .strip_prefix(model)
        .and_then(|s| s.strip_prefix('-'))
        .is_some_and(|date| date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
}
impl ResponseIdentity {
    /// Absence is accepted only with an explicit absence pin and matching model.
    pub fn check_pin(&self, model: &str, revision: &str) -> Result<()> {
        let dispatch_revision = if dated_pin(model, revision) {
            "absent"
        } else {
            revision
        };
        if self.system_fingerprint.is_none() && dispatch_revision != "absent" {
            return Err(super::Error::Invalid(
                "openrouter_fingerprint_absent_requires_explicit_pin",
            ));
        }
        require(
            self.returned_model == model && self.returned_revision == dispatch_revision,
            "provider revision drift quarantined",
        )
    }
}
/// Decode recorded chat-completions with bounded answers and no Responses-format assumptions.
pub fn reply(body: &[u8], model: &str, request_hash: &Digest) -> Result<(WireAnswer, Receipt)> {
    require(body.len() <= 256 * 1024, "OpenRouter response size")?;
    let value: Value = decode(body)?;
    let id = value["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(super::Error::Invalid("OpenRouter execution ID"))?;
    let provider = value["provider"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(super::Error::Invalid("OpenRouter routing provider"))?;
    let identity = response_identity(&value)?;
    if value["choices"].as_array().is_some_and(|a| a.len() == 1)
        && value["choices"][0]["finish_reason"] == "length"
    {
        return Err(super::Error::Invalid("truncated_output"));
    }
    require(
        value["model"] == model
            && value["choices"].as_array().is_some_and(|a| a.len() == 1)
            && value["choices"][0]["finish_reason"] == "stop"
            && value["choices"][0]["message"]["refusal"].is_null(),
        "OpenRouter incomplete, refused or misrouted answer",
    )?;
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or(super::Error::Invalid("OpenRouter content"))?;
    super::structured_output::validate_answer(text.as_bytes())?;
    let answer: WireAnswer = decode(text.as_bytes())?;
    require(
        answer.request_hash == *request_hash
            && answer.observations.len() <= 64
            && (answer.outcome == super::schema::Outcome::Unverifiable
                || !answer.observations.is_empty()),
        "OpenRouter request-bound answer",
    )?;
    let usage = &value["usage"];
    let input = usage["prompt_tokens"]
        .as_u64()
        .ok_or(super::Error::Invalid("OpenRouter prompt usage"))?;
    let output = usage["completion_tokens"]
        .as_u64()
        .ok_or(super::Error::Invalid("OpenRouter completion usage"))?;
    require(
        usage.get("currency").is_none_or(|c| c == "USD")
            && input.checked_add(output) == usage["total_tokens"].as_u64()
            && usage["cost"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.),
        "OpenRouter inconsistent or unknown billed usage",
    )?;
    Ok((
        answer,
        Receipt {
            request_id: id.into(),
            model: model.into(),
            provider: provider.into(),
            revision: identity.returned_revision,
            usage: usage.clone(),
            billing_source: format!(
                "OpenRouter; provider prices without markup; {PRICE_VERSION}; alias-bound and time-specific"
            ),
        },
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn recorded_openrouter_chat_shape_usage_routing_and_price_source_are_bound() {
        let body = include_bytes!("../../tests/fixtures/assist-openrouter/chat-completion.json");
        let hash = Digest::of_bytes(b"fixture-request");
        let (answer, receipt) = reply(body, "openai/fixture-model", &hash).unwrap();
        assert_eq!(answer.request_hash, hash);
        assert_eq!(receipt.request_id, "gen-fixture-001");
        assert_eq!(receipt.provider, "fixture-provider");
        assert_eq!(
            receipt.usage["completion_tokens_details"]["reasoning_tokens"],
            10
        );
        assert!(receipt.billing_source.contains("OpenRouter"));
        assert!(receipt.billing_source.contains(PRICE_VERSION));
        assert!(reply(body, "other/model", &hash).is_err());
        assert!(
            reply(
                body,
                "openai/fixture-model",
                &Digest::of_bytes(b"other request")
            )
            .is_err()
        );
        for field in ["usage", "provider", "id"] {
            let mut value: Value = serde_json::from_slice(body).unwrap();
            value.as_object_mut().unwrap().remove(field);
            assert!(
                reply(
                    &serde_json::to_vec(&value).unwrap(),
                    "openai/fixture-model",
                    &hash
                )
                .is_err()
            );
        }
        let mut value: Value = serde_json::from_slice(body).unwrap();
        value["choices"][0]["finish_reason"] = json!("length");
        assert!(
            reply(
                &serde_json::to_vec(&value).unwrap(),
                "openai/fixture-model",
                &hash
            )
            .is_err()
        );
        let prices: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/assist-openrouter/prices.json"
        ))
        .unwrap();
        assert_eq!(prices["version"], PRICE_VERSION);
        assert_eq!(prices["live_verified"], false);
    }
    #[test]
    fn g12_task_reasoning_policy_is_closed_and_reserved_inside_output_limit() {
        let model = super::super::price::OPENROUTER_MODEL;
        for (task, cap) in [("check_ui", 512), ("explain", 1024), ("audit_mask", 1024)] {
            let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},
                "contents":[{"parts":[{"text":json!({"task":task}).to_string()}]}]});
            let bytes = request(&serde_json::to_vec(&source).unwrap(), model).unwrap();
            let payload: Value = decode(&bytes).unwrap();
            let admitted = admission(&bytes, model).unwrap();
            assert_eq!(payload["reasoning"], json!({"max_tokens":cap}));
            assert_eq!(admitted.reasoning, cap);
            assert_eq!(
                admitted.bounds.output,
                super::super::execution::OUTPUT_LIMIT
            );
            assert_eq!(
                admitted.reservation,
                admitted.bounds.input * 750 + 4096 * 3750
            );
            for invalid in [
                Value::Null,
                json!({}),
                json!({"max_tokens":0}),
                json!({"max_tokens":4096}),
                json!({"max_tokens":cap+1}),
                json!({"max_tokens":cap,"exclude":true}),
                json!({"effort":"low"}),
                json!({"max_tokens":cap.to_string()}),
            ] {
                let mut drift = payload.clone();
                drift["reasoning"] = invalid;
                assert!(admission(&serde_json::to_vec(&drift).unwrap(), model).is_err());
            }
            let mut absent = payload;
            absent.as_object_mut().unwrap().remove("reasoning");
            assert!(admission(&serde_json::to_vec(&absent).unwrap(), model).is_err());
        }
    }
    #[test]
    fn g12_strict_response_format_is_exact_and_hash_bound() {
        let model = super::super::price::OPENROUTER_MODEL;
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let bytes = request(&serde_json::to_vec(&source).unwrap(), model).unwrap();
        let payload: Value = decode(&bytes).unwrap();
        let format = &payload["response_format"];
        assert_eq!(
            *format,
            super::super::structured_output::openrouter_format().unwrap()
        );
        assert_eq!(REQUEST_POLICY, "assist-openrouter-provider-schema/1");
        assert_eq!(
            format["json_schema"]["name"],
            super::super::structured_output::PROJECTED_SCHEMA_NAME
        );
        for invalid in [
            Value::Null,
            json!({"type":"json_object"}),
            json!({"type":"json_schema","json_schema":{"name":"saccade_assist_answer","strict":false,"schema":format["json_schema"]["schema"]}}),
            json!({"type":"json_schema","json_schema":{"name":"other","strict":true,"schema":format["json_schema"]["schema"]}}),
            json!({"type":"json_schema","json_schema":{"name":"saccade_assist_answer","strict":true,"schema":{}}}),
            json!({"type":"json_schema","json_schema":{"name":super::super::structured_output::PROJECTED_SCHEMA_NAME,"strict":true,"schema":super::super::structured_output::answer_schema().unwrap()}}),
        ] {
            let mut drift = payload.clone();
            drift["response_format"] = invalid;
            let changed = crate::evidence::canonical::bytes(&drift).unwrap();
            assert_ne!(Digest::of_bytes(&bytes), Digest::of_bytes(&changed));
            assert!(validate_request(&changed, model).is_err());
        }
        for path in [
            "/response_format/json_schema/strict",
            "/response_format/json_schema/schema/additionalProperties",
            "/response_format/json_schema/schema/properties/observations/items/additionalProperties",
            "/response_format/json_schema/schema/properties/observations/items/properties/geometry/anyOf/0/additionalProperties",
            "/response_format/json_schema/schema/properties/observations/items/properties/geometry/anyOf/1/additionalProperties",
        ] {
            let mut drift = payload.clone();
            let field = drift.pointer_mut(path).unwrap();
            *field = json!(!field.as_bool().unwrap());
            assert!(validate_request(&serde_json::to_vec(&drift).unwrap(), model).is_err());
        }
        let mut drift = payload.clone();
        drift["response_format"]["json_schema"]["extra"] = json!(true);
        assert!(validate_request(&serde_json::to_vec(&drift).unwrap(), model).is_err());
        let mut drift = payload;
        drift.as_object_mut().unwrap().remove("response_format");
        assert!(validate_request(&serde_json::to_vec(&drift).unwrap(), model).is_err());
    }
    #[test]
    fn g12_http_error_metadata_is_bounded_and_charset_validated() {
        let fixture = include_bytes!("../../tests/fixtures/assist-openrouter/schema-http-400.json");
        for invalid in [
            "x".repeat(65),
            "bad\nvalue".into(),
            "非ascii".into(),
            String::new(),
        ] {
            let mut value: Value = decode(fixture).unwrap();
            value["error"]["code"] = json!(invalid);
            value["error"]["metadata"]["provider_name"] = json!(invalid);
            value["error"]["metadata"]["raw"] =
                json!(json!({"error":{"status":invalid}}).to_string());
            let classified = http_error(Some(400), &serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(classified.openrouter_error_code.is_none());
            assert!(classified.provider_name.is_none());
            assert!(classified.provider_status.is_none());
            assert!(!classified.zero_cost_refused);
            assert!(
                !serde_json::to_string(&classified)
                    .unwrap()
                    .contains(&invalid)
                    || invalid.is_empty()
            );
        }
        assert!(http_error(None, fixture).is_none());
        assert!(!http_error(Some(500), fixture).unwrap().zero_cost_refused);
        assert!(!http_error(Some(400), b"{}").unwrap().zero_cost_refused);
    }
    #[test]
    fn g12_openrouter_reply_refuses_projected_array_overflow_locally() {
        let fixture = include_bytes!("../../tests/fixtures/assist-openrouter/chat-completion.json");
        let mut response: Value = decode(fixture).unwrap();
        let mut answer: Value = decode(
            response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let hash: Digest = serde_json::from_value(answer["request_hash"].clone()).unwrap();
        let observation = json!({"slot":"P1","kind":"appearance","statement":"appearance:changed",
            "geometry":{"type":"box","pixels":[0.1,0.1,0.5,0.5]},"visibility":"visible",
            "evidence_refs":["P1:R0"],"uncertainty":0.1});
        for length in [64, 65] {
            answer["observations"] = json!(vec![observation.clone(); length]);
            response["choices"][0]["message"]["content"] = json!(answer.to_string());
            assert_eq!(
                reply(
                    &serde_json::to_vec(&response).unwrap(),
                    "openai/fixture-model",
                    &hash
                )
                .is_ok(),
                length == 64
            );
        }
    }
    #[test]
    fn g12_recorded_value_geometry_is_still_refused_locally() {
        let body =
            include_bytes!("../../tests/fixtures/assist-openrouter/value-geometry-pilot.json");
        let mut value: Value = decode(body).unwrap();
        let mut answer: Value = decode(
            value["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let hash: Digest = serde_json::from_value(answer["request_hash"].clone()).unwrap();
        assert_eq!(value["choices"][0]["finish_reason"], "stop");
        assert_eq!(value["usage"]["completion_tokens"], 445);
        assert_eq!(
            reply(body, super::super::price::OPENROUTER_MODEL, &hash)
                .unwrap_err()
                .code(),
            "closed schema or JSON violation"
        );
        // Changing only the wrong field name proves the recorded refusal's cause.
        for observation in answer["observations"].as_array_mut().unwrap() {
            let geometry = observation["geometry"].as_object_mut().unwrap();
            let pixels = geometry.remove("value").unwrap();
            geometry.insert("pixels".into(), pixels);
        }
        value["choices"][0]["message"]["content"] = json!(answer.to_string());
        assert!(
            reply(
                &serde_json::to_vec(&value).unwrap(),
                super::super::price::OPENROUTER_MODEL,
                &hash
            )
            .is_ok()
        );
    }
    #[test]
    fn g12_model_price_caps_and_payload_bounds_fail_closed() {
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let source = serde_json::to_vec(&source).unwrap();
        let model = "google/gemini-3.8-flash";
        let mut payload: Value = decode(&request(&source, model).unwrap()).unwrap();
        assert_eq!(
            payload["provider"]["max_price"],
            json!({"prompt":0.75,"completion":3.75})
        );
        assert!(validate_request(&serde_json::to_vec(&payload).unwrap(), model).is_ok());
        payload["messages"][1]["content"][0]["text"] = json!("x".repeat(16000));
        assert!(validate_request(&serde_json::to_vec(&payload).unwrap(), model).is_err());
        assert!(request(&source, "unpriced/expensive-model").is_err());
    }
    #[test]
    fn g12_inline_png_header_ceiling_and_unsupported_content_are_bounded() {
        let model = "google/gemini-3.8-flash";
        // Base64 PNG signature, IHDR length/type and dimensions 512 by 512.
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"inline_data":{"data":"iVBORw0KGgoAAAANSUhEUgAAAgAAAAIA"}}]}]});
        let mut payload: Value =
            decode(&request(&serde_json::to_vec(&source).unwrap(), model).unwrap()).unwrap();
        let bytes = serde_json::to_vec(&payload).unwrap();
        let admitted = admission(&bytes, model).unwrap();
        assert!(admitted.bounds.input >= super::super::price::CALIBRATED_IMAGE_TOKENS + 1024);
        assert_eq!(
            admitted.reservation,
            admitted.bounds.input * 750 + 4096 * 3750
        );
        assert_eq!(admitted.bounds.output, 4096);
        let second = payload["messages"][1]["content"][0].clone();
        payload["messages"][1]["content"]
            .as_array_mut()
            .unwrap()
            .push(second);
        let two = admission(&serde_json::to_vec(&payload).unwrap(), model).unwrap();
        assert!(two.bounds.input < super::super::execution::INPUT_LIMIT);
        assert!(two.reservation <= super::super::execution::INPUT_LIMIT * 750 + 4096 * 3750);
        for url in [
            "https://example.org/image.png",
            "data:image/png;base64,invalid",
            "data:image/jpeg;base64,iVBORw0KGgoAAAANSUhEUgAAAgAAAAIA",
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAEAAAAAAB",
        ] {
            let mut v: Value = decode(&bytes).unwrap();
            v["messages"][1]["content"][0]["image_url"]["url"] = json!(url);
            assert!(validate_request(&serde_json::to_vec(&v).unwrap(), model).is_err());
        }
        for field in ["tools", "audio", "prompt"] {
            let mut v: Value = decode(&bytes).unwrap();
            v[field] = json!({});
            assert!(validate_request(&serde_json::to_vec(&v).unwrap(), model).is_err());
        }
        payload = decode(&bytes).unwrap();
        payload["provider"]["max_price"]["prompt"] = json!(0.76);
        assert!(validate_request(&serde_json::to_vec(&payload).unwrap(), model).is_err());
        payload["provider"]
            .as_object_mut()
            .unwrap()
            .remove("max_price");
        assert!(validate_request(&serde_json::to_vec(&payload).unwrap(), model).is_err());
    }
    #[test]
    fn g12_historical_version_one_reservation_and_receipt_still_verify() {
        use crate::assist::price;
        use crate::budget_ledger::MoneyReceipt;
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"inline_data":{"data":"iVBORw0KGgoAAAANSUhEUgAAAgAAAAIA"}}]}]});
        let payload = request(
            &serde_json::to_vec(&source).unwrap(),
            price::OPENROUTER_MODEL,
        )
        .unwrap();
        let price_pin = price::openrouter_price(price::OPENROUTER_MODEL).unwrap();
        let old =
            price::openrouter_bounds_for_table(&payload, price_pin, price::IMAGE_TABLE).unwrap();
        let current = admission(&payload, price::OPENROUTER_MODEL).unwrap();
        assert_eq!(old.bounds.input - current.bounds.input, 8192 - 3086);
        assert_eq!(old.reservation, old.bounds.input * 750 + 4096 * 3750);
        // Legacy receipts have no image_table field; their stored bounds remain authoritative.
        let receipt: MoneyReceipt = serde_json::from_value(json!({
            "id":"historical-v1", "request_hash":Digest::of_bytes(&payload), "scopes":["fixture"],
            "reserved_nano_usd":old.reservation,"actual_nano_usd":2323500,"outcome":"completed",
            "usage":{"input_bound":old.bounds.input,"output_bound":4096,
                "usage":{"input_tokens":1513,"candidate_tokens":212,"thinking_tokens":0,"total_tokens":1725,"modality_details":null}}
        })).unwrap();
        let usage = serde_json::from_value(receipt.usage["usage"].clone()).unwrap();
        assert!(old.bounds.contains(&usage));
        assert!(receipt.usage.get("image_table").is_none());
        let encoded = serde_json::to_vec(&receipt).unwrap();
        assert_eq!(
            encoded,
            serde_json::to_vec(&serde_json::from_slice::<MoneyReceipt>(&encoded).unwrap()).unwrap()
        );
    }
    #[test]
    fn chat_request_has_chat_messages_bounded_output_and_pinned_routing() {
        let source = json!({"systemInstruction":{"parts":[{"text":"untrusted data"}]},"contents":[{"parts":[{"text":"P1"},{"inline_data":{"data":"fixture-base64"}}]}]});
        let bytes = request(
            &serde_json::to_vec(&source).unwrap(),
            super::super::price::OPENROUTER_MODEL,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["messages"][1]["content"][1]["type"], "image_url");
        assert_eq!(value["max_tokens"], 4096);
        assert_eq!(value["provider"]["allow_fallbacks"], false);
        assert!(value.get("input").is_none());
        assert!(request(&serde_json::to_vec(&source).unwrap(), "../bad?model").is_err());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod execution_tests {
    use super::*;
    use crate::assist::execution::{CacheKey, ENCODER, Executor};
    use crate::budget_ledger::{Caps, Ledger, MoneyScope, Scope};
    use crate::judge_provider::{Keys, transport::*};
    use crate::root_policy::RootPolicy;
    use std::{
        cell::Cell,
        collections::BTreeMap,
        time::{Duration, Instant},
    };
    #[test]
    fn g12_openrouter_reserves_settles_unknown_zero_and_quarantines_usage_breach() {
        scenarios(0..11);
    }
    #[test]
    fn g12_reservation_tracks_payload_and_explicit_output_before_dispatch() {
        scenarios(11..12);
    }
    #[test]
    fn g12_total_output_over_reservation_is_charged_and_stops_campaign() {
        scenarios(47..48);
    }
    #[test]
    fn g12_returned_cost_over_reservation_is_charged_and_stops_campaign() {
        scenarios(12..13);
    }
    #[test]
    fn g12_quarantined_drift_retains_bounded_identity_without_another_dispatch() {
        scenarios(13..15);
        scenarios(18..21);
    }
    #[test]
    fn g12_absent_fingerprint_requires_explicit_pin_and_matching_model() {
        scenarios(15..18);
        scenarios(21..22);
        let body: Value = decode(include_bytes!(
            "../../tests/fixtures/assist-openrouter/chat-completion.json"
        ))
        .unwrap();
        for fingerprint in [
            json!(""),
            json!("absent"),
            json!(false),
            json!("bad\nvalue"),
            json!("x".repeat(257)),
        ] {
            let mut invalid = body.clone();
            invalid["system_fingerprint"] = fingerprint;
            assert_eq!(
                response_identity(&invalid).unwrap_err().code(),
                "invalid OpenRouter fingerprint"
            );
        }
        let mut absent = body;
        absent.as_object_mut().unwrap().remove("system_fingerprint");
        let (_, receipt) = reply(
            &serde_json::to_vec(&absent).unwrap(),
            "openai/fixture-model",
            &Digest::of_bytes(b"fixture-request"),
        )
        .unwrap();
        assert_eq!(receipt.revision, "absent");
    }
    #[test]
    fn g12_dated_revision_pin_matches_or_quarantines_after_reconciliation() {
        scenarios(28..30);
        let identity = response_identity(&json!({"model":"google/gemini-3.8-flash"})).unwrap();
        assert!(
            identity
                .check_pin(
                    "google/gemini-3.8-flash",
                    "google/gemini-3.8-flash-20260902"
                )
                .is_ok()
        );
        assert!(
            identity
                .check_pin(
                    "google/gemini-3.8-flash",
                    "google/gemini-3.8-flash-2026090x"
                )
                .is_err()
        );
        let present = response_identity(
            &json!({"model":"google/gemini-3.8-flash","system_fingerprint":"present-fixture"}),
        )
        .unwrap();
        assert!(
            present
                .check_pin(
                    "google/gemini-3.8-flash",
                    "google/gemini-3.8-flash-20260902"
                )
                .is_err()
        );
    }
    #[test]
    fn g12_recorded_call7_reasoning_hint_is_not_a_campaign_stop() {
        scenarios(30..32);
    }
    #[test]
    fn g12_delayed_generation_retries_and_preserves_terminal_failure_reasons() {
        scenarios(22..28);
    }
    #[test]
    fn g12_generation_deadline_bounds_gets_waits_and_following_receipts() {
        #[derive(Default)]
        struct Clock(Cell<Duration>);
        impl ReconciliationClock for Clock {
            fn elapsed(&self) -> Duration {
                self.0.get()
            }
            fn sleep(&self, duration: Duration) {
                self.0.set(self.0.get() + duration);
            }
        }
        struct Slow<'a> {
            clock: &'a Clock,
            timeouts: std::cell::RefCell<Vec<Duration>>,
        }
        impl Http for Slow<'_> {
            fn get(
                &self,
                _: &str,
                _: (&str, &str),
                timeout: Duration,
            ) -> std::result::Result<HttpReply, String> {
                self.timeouts.borrow_mut().push(timeout);
                self.clock.sleep(timeout);
                Ok(HttpReply {
                    status: 404,
                    retry_after_secs: None,
                    body: br#"{"error":"not published"}"#.to_vec(),
                })
            }
            fn post(
                &self,
                _: &str,
                _: (&str, &str),
                _: &[u8],
                _: Duration,
            ) -> std::result::Result<HttpReply, String> {
                panic!("reconciliation must never dispatch a completion")
            }
        }
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("openrouter.env"),
            "OPENROUTER_API_KEY=fixture-openrouter-key",
        )
        .unwrap();
        let secret = Keys::assist_fixture(temp.path().into())
            .openrouter()
            .unwrap();
        let clock = Clock::default();
        let http = Slow {
            clock: &clock,
            timeouts: Default::default(),
        };
        let deadline = Duration::from_secs(30);
        let first = lookup_generation(&http, &secret, Some("gen-fixture"), deadline, &clock);
        assert_eq!(first.result.unwrap_err(), "openrouter_generation_not_ready");
        assert_eq!(first.attempts, 4);
        assert_eq!(first.waited_ms, 14000);
        assert_eq!(
            *http.timeouts.borrow(),
            vec![
                Duration::from_secs(5),
                Duration::from_secs(5),
                Duration::from_secs(5),
                Duration::from_secs(1)
            ]
        );
        assert_eq!(clock.elapsed(), deadline);
        let second = lookup_generation(&http, &secret, Some("gen-other"), deadline, &clock);
        assert_eq!(
            second.result.unwrap_err(),
            "openrouter_reconciliation_deadline"
        );
        assert_eq!(second.attempts, 0);
        assert_eq!(http.timeouts.borrow().len(), 4);
        assert_eq!(
            lookup_generation(&http, &secret, None, deadline, &clock)
                .result
                .unwrap_err(),
            "openrouter_generation_missing"
        );
    }
    #[test]
    fn g12_recorded_http_400_classifies_and_settles_zero_without_generation_lookup() {
        scenarios(32..42);
    }
    #[test]
    fn g12_call_cap_120_and_transport_receipts_remain_unknown_cost() {
        scenarios(42..47);
    }
    fn scenarios(indices: std::ops::Range<usize>) {
        #[derive(Default)]
        struct Clock(Cell<Duration>);
        impl ReconciliationClock for Clock {
            fn elapsed(&self) -> Duration {
                self.0.get()
            }
            fn sleep(&self, duration: Duration) {
                self.0.set(self.0.get() + duration);
            }
        }
        struct Fake<'a> {
            ledger: &'a Ledger,
            calls: Cell<u32>,
            body: Vec<u8>,
            gets: Cell<u32>,
            scenario: usize,
            generations: Cell<u32>,
        }
        impl Http for Fake<'_> {
            fn get(
                &self,
                url: &str,
                _: (&str, &str),
                timeout: Duration,
            ) -> std::result::Result<HttpReply, String> {
                self.gets.set(self.gets.get() + 1);
                let remaining = if self.scenario == 4 {
                    0.01
                } else if (self.scenario == 5 && self.gets.get() > 2)
                    || (self.scenario == 9 && self.gets.get() > 4)
                {
                    0.0
                } else {
                    1.0
                };
                let usage = if self.scenario == 6 && self.gets.get() > 2 {
                    0.1
                } else {
                    0.0
                };
                let mut body = json!({"data":{"limit":1,"limit_remaining":remaining,"usage":usage,"total_credits":remaining + usage,"total_usage":usage}});
                if self.scenario == 3 {
                    body = json!({"data":{}});
                }
                if url.contains("/generation?") {
                    self.generations.set(self.generations.get() + 1);
                    assert!(timeout <= Duration::from_secs(5));
                    if self.scenario == 23 || (self.scenario == 22 && self.generations.get() < 4) {
                        return Ok(HttpReply {
                            status: 404,
                            retry_after_secs: None,
                            body: br#"{"error":{"message":"untrusted provider detail"}}"#.to_vec(),
                        });
                    }
                    body = json!({"data":{"model":"google/gemini-3.8-flash-20260902","provider_name":"Google AI Studio","id":"gen-fixture-001","total_cost":if self.scenario == 7 {0.001} else if self.scenario == 10 {0.0} else {0.0001875}}});
                    if self.scenario >= 28 {
                        body = serde_json::from_slice(include_bytes!(
                            "../../tests/fixtures/assist-openrouter/generation-ready.json"
                        ))
                        .unwrap();
                        if self.scenario == 29 {
                            body["data"]["model"] = json!("google/gemini-3.8-flash-20261001");
                        }
                    }
                    if self.scenario == 24 {
                        body["data"]["id"] = json!("other-generation");
                    }
                    if self.scenario == 25 {
                        body["data"].as_object_mut().unwrap().remove("total_cost");
                    }
                    if self.scenario == 26 {
                        return Ok(HttpReply {
                            status: 200,
                            retry_after_secs: None,
                            body: b"untrusted provider detail".to_vec(),
                        });
                    }
                    if self.scenario == 27 {
                        body["reflected"] = json!("fixture-openrouter-key");
                    }
                }
                if self.scenario == 8 {
                    body["reflected"] = json!("fixture-openrouter-key");
                }
                Ok(HttpReply {
                    status: 200,
                    retry_after_secs: None,
                    body: serde_json::to_vec(&body).unwrap(),
                })
            }
            fn post(
                &self,
                url: &str,
                _: (&str, &str),
                _: &[u8],
                timeout: Duration,
            ) -> std::result::Result<HttpReply, String> {
                assert_eq!(url, ENDPOINT);
                if (42..47).contains(&self.scenario) {
                    assert!(timeout > Duration::from_secs(60));
                    assert!(timeout <= Duration::from_secs(120));
                }
                assert_eq!(
                    self.ledger
                        .money_receipts()
                        .unwrap()
                        .last()
                        .unwrap()
                        .outcome,
                    "reserved"
                );
                self.calls.set(self.calls.get() + 1);
                if (42..47).contains(&self.scenario) {
                    return Err([
                        "transport_timeout",
                        "transport_connect",
                        "transport_reset",
                        "transport_tls",
                        "untrusted fixture-secret",
                    ][self.scenario - 42]
                        .into());
                }
                if self.scenario == 37 {
                    return Err("fixture network failure".into());
                }
                Ok(HttpReply {
                    status: if self.scenario == 33 {
                        500
                    } else if (32..47).contains(&self.scenario) {
                        400
                    } else {
                        200
                    },
                    retry_after_secs: None,
                    body: self.body.clone(),
                })
            }
        }
        for index in indices {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(
                temp.path().join("openrouter.env"),
                "OPENROUTER_API_KEY=fixture-openrouter-key",
            )
            .unwrap();
            let keys = Keys::assist_fixture(temp.path().into());
            let mut roots = RootPolicy::new(&[temp.path().into()], None, false, &[]).unwrap();
            let user = UserConfig {
                roots: vec![RootSetting {
                    id: "fixture".into(),
                    path: temp.path().into(),
                    egress: EgressSetting::Allow,
                }],
                ..Default::default()
            };
            user.apply(&mut roots).unwrap();
            let ledger = Ledger::new(&temp.path().join("ledger"), false);
            let auth = Authorization {
                enabled: true,
                scopes: vec![Scope {
                    id: "run".into(),
                    caps: Caps {
                        total: 8,
                        providers: BTreeMap::from([("openrouter".into(), 8)]),
                    },
                }],
            };
            let mut value: Value = serde_json::from_slice(include_bytes!(
                "../../tests/fixtures/assist-openrouter/chat-completion.json"
            ))
            .unwrap();
            value["model"] = json!("google/gemini-3.8-flash");
            if [13, 17].contains(&index) {
                value["model"] = json!("google/other-model");
            }
            if index == 14 {
                value["system_fingerprint"] = json!("observed-fixture-revision");
            }
            if [15, 16, 17].contains(&index) {
                value.as_object_mut().unwrap().remove("system_fingerprint");
            }
            if index == 21 {
                value["system_fingerprint"] = Value::Null;
            }
            if index == 18 {
                value["system_fingerprint"] = json!("bad\nmetadata");
            }
            if index == 19 {
                value["model"] = json!("x".repeat(129));
            }
            if index == 20 {
                value["system_fingerprint"] = json!("fixture-openrouter-key");
            }
            if index >= 28 {
                value.as_object_mut().unwrap().remove("system_fingerprint");
                value["usage"] = json!({"prompt_tokens":1513,"completion_tokens":212,"total_tokens":1725,"cost":0.0023235});
            }
            if index >= 30 {
                value = decode(include_bytes!(
                    "../../tests/fixtures/assist-openrouter/truncated-pilot.json"
                ))
                .unwrap();
                if index == 30 {
                    value["usage"]["completion_tokens_details"]["reasoning_tokens"] = json!(1024);
                }
            }
            if matches!(index, 31 | 47) {
                value = decode(include_bytes!(
                    "../../tests/fixtures/assist-openrouter/call7-reasoning-hint.json"
                ))
                .unwrap();
                if index == 47 {
                    value["usage"]["completion_tokens"] = json!(4097);
                    value["usage"]["total_tokens"] = json!(9270);
                }
            }
            if index == 1 {
                value["usage"] = json!({"cost":0});
            }
            if index == 10 {
                value["usage"]["cost"] = json!(0);
            }
            if index == 12 {
                value["usage"]["cost"] = json!(0.04);
            }
            if (32..47).contains(&index) {
                value = decode(include_bytes!(
                    "../../tests/fixtures/assist-openrouter/schema-http-400.json"
                ))
                .unwrap();
                match index {
                    34 => value = json!({"error":"ambiguous"}),
                    35 => value["id"] = json!("gen-fixture-error"),
                    36 => value["error"]["message"] = json!("fixture-openrouter-key"),
                    38 => value["usage"] = json!({"cost":0.0001}),
                    39 => value["id"] = Value::Null,
                    40 => {
                        value["error"]["metadata"]["raw"] =
                            json!("{\"error\":{\"id\":\"gen-fixture\"}}")
                    }
                    41 => value["error"]["metadata"]["raw"] = json!("ambiguous provider text"),
                    _ => {}
                }
            }
            let mut fake = Fake {
                ledger: &ledger,
                calls: Cell::new(0),
                body: serde_json::to_vec(&value).unwrap(),
                gets: Cell::new(0),
                scenario: index,
                generations: Cell::new(0),
            };
            let mut source = json!({"systemInstruction":{"parts":[{"text":"system fixture"}]},
                "contents":[{"parts":[{"text":if index >= 28 { "fixture".repeat(40) } else { "fixture".into() }}]}]});
            if index >= 30 {
                source["contents"][0]["parts"][0]["text"] = json!("fixture".repeat(400));
            }
            if index == 2 {
                source["contents"][0]["parts"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"inline_data":{"data":"iVBORw0KGgoAAAANSUhEUgAAAgAAAAIA"}}));
            }
            let mut payload: Value = decode(
                &request(
                    &serde_json::to_vec(&source).unwrap(),
                    "google/gemini-3.8-flash",
                )
                .unwrap(),
            )
            .unwrap();
            if index == 11 {
                payload["max_tokens"] = json!(64);
                payload["reasoning"] = json!({"max_tokens":16});
            }
            let payload = serde_json::to_vec(&payload).unwrap();
            if index == 2 {
                let input = admission(&payload, "google/gemini-3.8-flash")
                    .unwrap()
                    .bounds
                    .input
                    + 1;
                assert!(input < crate::assist::execution::INPUT_LIMIT);
                value["usage"]["prompt_tokens"] = json!(input);
                value["usage"]["total_tokens"] = json!(input + 30);
                fake.body = serde_json::to_vec(&value).unwrap();
            }
            let transport = Transport {
                user: &user,
                roots: &roots,
                authorization: &auth,
                ledger: &ledger,
                keys: &keys,
                http: &fake,
            };
            let executor = Executor {
                transport: &transport,
                ledger: &ledger,
                money_scopes: vec![MoneyScope {
                    id: "offline".into(),
                    cap_nano_usd: 100_000_000,
                }],
                sources: vec!["fixture".into()],
                deadline: Instant::now()
                    + Duration::from_secs(if (42..47).contains(&index) { 300 } else { 1 }),
            };
            let key = CacheKey {
                evidence_hash: Digest::of_bytes(b"fixture-request"),
                payload_hash: Digest::of_bytes(&payload),
                prompt_hash: Digest::of_bytes(b"fixture"),
                encoder_version: ENCODER.into(),
                provider: "openrouter".into(),
                model: "google/gemini-3.8-flash".into(),
                revision: if index >= 28 {
                    "google/gemini-3.8-flash-20260902"
                } else if [16, 17, 21].contains(&index) {
                    "absent"
                } else {
                    "fixture-revision-1"
                }
                .into(),
                settings: json!({}),
                api_config_hash: Digest::of_bytes(b"user"),
                order: "single".into(),
            };
            let result = executor.call(&key, &payload);
            let receipts = ledger.money_receipts().unwrap();
            let campaign =
                std::fs::read_to_string(temp.path().join("ledger/campaign.json")).unwrap();
            assert!(!campaign.contains("fixture-openrouter-key"));
            if (32..47).contains(&index) {
                assert_eq!(fake.calls.get(), 1);
                assert_eq!(fake.generations.get(), 0);
                let receipt = &receipts[0];
                if (42..47).contains(&index) {
                    assert_eq!(
                        receipt.usage["transport_failure"],
                        ["timeout", "connect", "reset", "tls", "other"][index - 42]
                    );
                    assert!(receipt.usage["http_error"].is_null());
                    assert!(!campaign.contains("untrusted fixture-secret"));
                }
                let refused = index == 32;
                assert_eq!(
                    result.err().unwrap().code(),
                    if refused {
                        "openrouter_http_zero_cost_refused"
                    } else {
                        "assist_provider_execution_incomplete"
                    }
                );
                assert_eq!(
                    receipt.actual_nano_usd,
                    if refused {
                        Some(0)
                    } else if index == 38 {
                        Some(100_000)
                    } else {
                        None
                    }
                );
                assert_eq!(
                    receipt.outcome,
                    if refused {
                        "zero_cost_refused"
                    } else {
                        "incomplete"
                    }
                );
                assert_eq!(receipt.usage["qualification_eligible"], false);
                assert_eq!(receipt.usage["request_policy"], REQUEST_POLICY);
                assert_eq!(
                    receipt.usage["schema_projection"],
                    super::super::structured_output::PROJECTION_POLICY
                );
                assert!(!campaign.contains("Request contains an invalid argument"));
                assert!(!campaign.contains("Provider returned error"));
                assert!(!campaign.contains("ambiguous provider text"));
                if refused {
                    assert_eq!(
                        receipt.usage["http_error"],
                        json!({"http_status":400,"openrouter_error_code":400,"provider_name":"Google AI Studio","provider_status":"INVALID_ARGUMENT","zero_cost_refused":true})
                    );
                    assert_eq!(
                        receipt.usage["reconciliation"]["state"],
                        "zero_cost_refused"
                    );
                    let before = std::fs::read(temp.path().join("ledger/campaign.json")).unwrap();
                    let summary = reconcile_with_clock(
                        &transport,
                        Duration::from_secs(30),
                        &Clock::default(),
                    )
                    .unwrap();
                    assert_eq!(summary.zero_cost_refused, 1);
                    assert_eq!(summary.pending, 0);
                    assert_eq!(summary.matched, 0);
                    assert_eq!(summary.state, "zero_cost_refused");
                    assert_eq!(fake.generations.get(), 0);
                    assert_eq!(
                        before,
                        std::fs::read(temp.path().join("ledger/campaign.json")).unwrap()
                    );
                    let counters: Value = decode(before.as_slice()).unwrap();
                    assert_eq!(counters["money"]["counters"]["offline"][1], 0);
                    assert_eq!(counters["money"]["counters"]["campaign/assist"][1], 0);
                } else if index != 38 {
                    assert_eq!(receipt.usage["reconciliation"]["state"], "pending");
                }
                continue;
            }
            if index == 47 {
                assert_eq!(
                    result.err().unwrap().code(),
                    "provider usage exceeded reservation"
                );
                assert_eq!(receipts[0].actual_nano_usd, Some(10_862_250));
                assert_eq!(receipts[0].outcome, "usage_limit_exceeded");
                assert_eq!(receipts[0].usage["bound_breach"], true);
                assert_eq!(receipts[0].usage["provider_reasoning_over_hint"], true);
                assert!(receipts[0].reserved_nano_usd > 10_862_250);
                assert!(executor.call(&key, &payload).is_err());
                assert_eq!(fake.calls.get(), 1);
                continue;
            }
            if index >= 30 {
                if index == 30 {
                    assert_eq!(result.err().unwrap().code(), "truncated_output");
                } else {
                    let completed = result.unwrap();
                    assert_eq!(completed.provenance.usage.thinking_tokens, Some(1638));
                    assert_eq!(receipts[0].outcome, "completed");
                    assert!(receipts[0].reserved_nano_usd >= 10_862_250);
                    assert_eq!(
                        receipts[0].usage["reasoning_hint"],
                        json!({"requested_tokens":1024,"observed_tokens":1638})
                    );
                }
                assert_eq!(
                    receipts[0].actual_nano_usd,
                    Some(if index == 30 { 17_222_250 } else { 10_862_250 })
                );
                assert_eq!(receipts[0].usage["reasoning_bound"], 1024);
                assert_eq!(
                    receipts[0].usage["provider_reasoning_over_hint"],
                    index == 31
                );
                assert_eq!(receipts[0].usage["request_policy"], REQUEST_POLICY);
                assert_eq!(receipts[0].usage["bound_breach"], false);
                assert_eq!(
                    decode::<Value>(campaign.as_bytes()).unwrap()["money"]["stopped"],
                    false
                );
                assert_eq!(fake.calls.get(), 1);
                assert_eq!(fake.generations.get(), 0);
                continue;
            }
            if index >= 28 {
                let completed = result.unwrap();
                assert_eq!(completed.provenance.returned_revision, "absent");
                assert_eq!(receipts[0].usage["reconciliation"]["state"], "pending");
                assert_eq!(receipts[0].usage["qualification_eligible"], false);
                assert_eq!(fake.generations.get(), 0);
                let reconciled =
                    reconcile_with_clock(&transport, Duration::from_secs(30), &Clock::default());
                assert_eq!(reconciled.is_ok(), index == 28);
                let receipts = ledger.money_receipts().unwrap();
                let receipt = &receipts[0];
                let identity = &receipt.usage["revision_identity"];
                assert_eq!(
                    identity["dated_model"],
                    if index == 28 {
                        "google/gemini-3.8-flash-20260902"
                    } else {
                        "google/gemini-3.8-flash-20261001"
                    }
                );
                assert_eq!(identity["provider_name"], "Google AI Studio");
                assert_eq!(identity["requested_revision"], key.revision);
                assert_eq!(identity["revision_drifted"], index == 29);
                assert_eq!(identity["quarantined"], index == 29);
                assert_eq!(receipt.usage["qualification_eligible"], index == 28);
                assert_eq!(receipt.actual_nano_usd, Some(2_323_500));
                assert_eq!(
                    reconciliation_status(&ledger).unwrap().state,
                    if index == 28 { "matched" } else { "mismatch" }
                );
                let before = std::fs::read(temp.path().join("ledger/campaign.json")).unwrap();
                assert_eq!(
                    reconcile_with_clock(&transport, Duration::from_secs(30), &Clock::default())
                        .is_ok(),
                    index == 28
                );
                assert_eq!(
                    before,
                    std::fs::read(temp.path().join("ledger/campaign.json")).unwrap()
                );
                assert_eq!(fake.generations.get(), 1);
                assert_eq!(fake.calls.get(), 1);
                continue;
            }
            if (13..22).contains(&index) {
                assert_eq!(fake.calls.get(), 1);
                assert_eq!(receipts.len(), 1);
                let identity = &receipts[0].usage["response_identity"];
                if [16, 21].contains(&index) {
                    let completed = result.unwrap();
                    assert_eq!(completed.provenance.returned_revision, "absent");
                    assert_eq!(identity["code"], "openrouter_fingerprint_absent");
                } else {
                    let code = result.err().unwrap().code();
                    assert_eq!(
                        code,
                        match index {
                            15 => "openrouter_fingerprint_absent_requires_explicit_pin",
                            18 => "invalid OpenRouter fingerprint",
                            19 => "invalid OpenRouter returned model",
                            20 => "assist_provider_execution_incomplete",
                            _ => "provider revision drift quarantined",
                        }
                    );
                    if index != 20 {
                        assert_eq!(receipts[0].usage["identity_error"], code);
                    }
                }
                if index < 18 || index == 21 {
                    assert_eq!(receipts[0].actual_nano_usd, Some(187_500));
                    assert_eq!(identity["returned_model"], value["model"]);
                    assert_eq!(identity["system_fingerprint"], value["system_fingerprint"]);
                } else {
                    assert!(identity.is_null());
                }
                assert!(!campaign.contains("bad\\nmetadata"));
                assert!(!campaign.contains(&"x".repeat(129)));
                continue;
            }
            if index == 1 {
                assert_eq!(
                    result.err().unwrap().code(),
                    "assist_provider_execution_incomplete"
                );
                assert_eq!(receipts[0].actual_nano_usd, None);
                assert_eq!(receipts[0].outcome, "incomplete");
                assert!(receipts[0].reserved_nano_usd > 0);
                continue;
            }
            if index == 12 {
                assert!(result.is_err());
                assert_eq!(receipts[0].actual_nano_usd, Some(40_000_000));
                assert_eq!(receipts[0].outcome, "cost_limit_exceeded");
                assert!(executor.call(&key, &payload).is_err());
                assert_eq!(fake.calls.get(), 1);
                continue;
            }
            if [3, 4, 5, 6, 8].contains(&index) {
                assert!(result.is_err(), "scenario {index}");
                assert_eq!(fake.calls.get(), 0);
                assert!(campaign.contains(if index == 3 {
                    "openrouter_ceiling_unavailable"
                } else if index == 4 {
                    "openrouter_allowance_exceeds_ceiling"
                } else if index == 5 {
                    "openrouter_remaining_exhausted"
                } else if index == 6 {
                    "openrouter_concurrent_consumer"
                } else {
                    "openrouter_accounting_rejected"
                }));
                if [5, 6].contains(&index) {
                    assert!(executor.call(&key, &payload).is_err());
                    assert_eq!(fake.calls.get(), 0);
                }
                continue;
            }
            if index == 9 {
                assert!(executor.call(&key, &payload).is_err());
                assert_eq!(fake.calls.get(), 1);
                assert!(
                    std::fs::read_to_string(temp.path().join("ledger/campaign.json"))
                        .unwrap()
                        .contains("openrouter_remaining_exhausted")
                );
            }
            let receipt = &receipts[0];
            if index == 11 {
                assert_eq!(
                    receipt.reserved_nano_usd,
                    (payload.len() as u64 + 1024) * 750 + 64 * 3750
                );
                assert_eq!(
                    receipt.usage["input_bound"],
                    json!(payload.len() as u64 + 1024)
                );
                assert_eq!(receipt.usage["output_bound"], json!(64));
            }
            assert_eq!(fake.calls.get(), 1);
            let clock = Clock::default();
            let reconciliation = reconcile_with_clock(
                &transport,
                Duration::from_secs(if index >= 22 { 30 } else { 1 }),
                &clock,
            );
            assert_eq!(
                reconciliation.is_ok(),
                ![1, 7, 24, 25, 26, 27].contains(&index)
            );
            assert_eq!(
                ledger.money_receipts().unwrap()[0].usage["reconciliation"]["matches"],
                json!(![1, 7, 23, 24, 25, 26, 27].contains(&index))
            );
            if index >= 22 {
                let receipts = ledger.money_receipts().unwrap();
                let r = &receipts[0].usage["reconciliation"];
                assert_eq!(r["attempts"], if index <= 23 { 4 } else { 1 });
                assert_eq!(r["waited_ms"], if index <= 23 { 14000 } else { 0 });
                assert_eq!(
                    r["reason"],
                    match index {
                        23 => json!("openrouter_generation_not_ready"),
                        24 => json!("openrouter_generation_identity"),
                        25 => json!("openrouter_generation_cost_unknown"),
                        26 => json!("openrouter_generation_invalid"),
                        27 => json!("openrouter_accounting_rejected"),
                        _ => Value::Null,
                    }
                );
                assert_eq!(fake.calls.get(), 1);
                let campaign =
                    std::fs::read_to_string(temp.path().join("ledger/campaign.json")).unwrap();
                assert!(!campaign.contains("untrusted provider detail"));
                assert!(!campaign.contains("fixture-openrouter-key"));
                if index == 23 {
                    assert!(r["generation"].is_null());
                    assert_eq!(r["state"], "pending");
                }
            }

            if index == 2 {
                assert!(result.is_err());
                assert_eq!(receipt.actual_nano_usd, Some(187_500));
                assert_eq!(receipt.outcome, "usage_limit_exceeded");
                assert_eq!(receipt.usage["bound_breach"], true);
                assert_eq!(
                    receipt.usage["image_table"],
                    crate::assist::price::OPENROUTER_IMAGE_TABLE
                );
                assert!(executor.call(&key, &payload).is_err());
                assert_eq!(fake.calls.get(), 1);
            } else {
                let result = result.unwrap();
                assert_eq!(
                    result.provenance.execution_id.as_deref(),
                    Some(receipt.id.as_str())
                );
                assert!(result.provenance.cost_basis.contains("OpenRouter"));
                assert_eq!(
                    receipt.actual_nano_usd,
                    if index == 1 {
                        None
                    } else if index == 10 {
                        Some(0)
                    } else {
                        Some(187_500)
                    }
                );
            }
        }
    }
}

/// Fixed consumer-drift tolerance: one microdollar, never caller configurable.
pub const CONSUMER_TOLERANCE: u64 = 1_000;
/// Rounding tolerance when reconciling returned and generation costs: one nanodollar.
pub const RECONCILIATION_TOLERANCE: u64 = 1;
/// Sanitized authoritative accounting snapshot; response bodies are never persisted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ceiling {
    /// Remaining key/credit minimum, rounded down.
    pub remaining: u64,
    /// Key usage rounded up, when parseable.
    pub key_usage: Option<u64>,
    /// Account usage rounded up, when parseable.
    pub account_usage: Option<u64>,
    /// SHA-256 of both exact response bodies.
    pub hashes: [Digest; 2],
}
/// Single-use proof produced only by the existing campaign ledger.
pub struct DispatchPermit {
    hash: Digest,
    created: std::time::Instant,
}
impl DispatchPermit {
    pub(crate) fn new(hash: Digest) -> Self {
        Self {
            hash,
            created: std::time::Instant::now(),
        }
    }
    pub(crate) fn check(self, payload: &[u8]) -> std::result::Result<(), String> {
        if self.hash != Digest::of_bytes(payload) || self.created.elapsed().as_secs() >= 1 {
            return Err("openrouter_stale_permit".into());
        }
        Ok(())
    }
}
/// Preserve original JSON decimals at monetary boundaries, without f64 conversion.
pub fn body_amount(body: &[u8], path: &[&str], round_up: bool) -> Option<u64> {
    let mut raw = std::str::from_utf8(body).ok()?;
    for field in path {
        let object: std::collections::BTreeMap<&str, &serde_json::value::RawValue> =
            serde_json::from_str(raw).ok()?;
        raw = object.get(field)?.get();
    }
    decimal_amount(raw, round_up)
}
/// Validate the closed chat-completions request before authorization or accounting.
pub fn validate_request(payload: &[u8], model: &str) -> Result<()> {
    admission(payload, model).map(|_| ())
}
/// Bind the allowlisted model, route price caps and actual payload reservation.
pub fn admission(payload: &[u8], model: &str) -> Result<super::price::OpenRouterAdmission> {
    require(payload.len() <= 32 * 1024 * 1024, "OpenRouter payload size")?;
    let price = super::price::openrouter_price(model)?;
    let v: Value = decode(payload)?;
    let format = if super::video::is_request(&v) {
        let packet = super::video::packet(&v)?;
        super::video::response_format_for_model(&packet, model)
    } else {
        super::structured_output::openrouter_format_for(model)?
    };
    require(
        v.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "model",
                    "messages",
                    "temperature",
                    "max_tokens",
                    "reasoning",
                    "response_format",
                    "provider",
                    "usage",
                ]
                .contains(&k.as_str())
            })
        }) && v["model"] == model
            && !model.ends_with(":batch")
            && v["messages"].as_array().is_some_and(|a| !a.is_empty())
            && v["temperature"] == 0
            && v["response_format"] == format
            && v["provider"]
                == json!({"allow_fallbacks":false,"require_parameters":true,"max_price":price.max_price()})
            && v["usage"] == json!({"include":true})
            && v["max_tokens"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= super::execution::OUTPUT_LIMIT),
        "OpenRouter closed request shape",
    )?;
    let output = v["max_tokens"].as_u64().unwrap_or(0);
    let reasoning = reasoning_budget(request_task(&v).as_deref(), output);
    require(
        reasoning > 0 && v["reasoning"] == json!({"max_tokens":reasoning}),
        "OpenRouter pinned reasoning policy",
    )?;
    super::price::openrouter_bounds(payload, price)
}
/// Exact decimal USD parser shared by CLI authorization and accounting.
pub fn decimal_amount(text: &str, round_up: bool) -> Option<u64> {
    if text.starts_with('-') || text.starts_with('+') {
        return None;
    }
    let (mantissa, exponent) = text
        .split_once(['e', 'E'])
        .map_or(Some((text, 0)), |(m, e)| Some((m, e.parse::<i32>().ok()?)))?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}").parse::<u128>().ok()?;
    let power = 9i32
        .checked_add(exponent)?
        .checked_sub(fraction.len().try_into().ok()?)?;
    let nano = if power >= 0 {
        digits.checked_mul(10u128.checked_pow(power.try_into().ok()?)?)?
    } else {
        let divisor = 10u128.checked_pow(power.checked_neg()?.try_into().ok()?)?;
        (digits / divisor).checked_add(u128::from(round_up && digits % divisor != 0))?
    };
    nano.try_into().ok()
}
/// Restricted read-only accounting URLs, with IDs encoded as query data.
pub(crate) fn accounting_url(url: &str) -> bool {
    matches!(
        url,
        "https://openrouter.ai/api/v1/key" | "https://openrouter.ai/api/v1/credits"
    ) || url
        .strip_prefix("https://openrouter.ai/api/v1/generation?id=")
        .is_some_and(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_%".contains(&b))
        })
}
fn accounting(
    http: &dyn crate::judge_provider::transport::Http,
    secret: &crate::judge_provider::Secret,
    url: &str,
    timeout: std::time::Duration,
) -> std::result::Result<Vec<u8>, String> {
    let reply = accounting_reply(http, secret, url, timeout).map_err(str::to_owned)?;
    if !(200..300).contains(&reply.status) {
        return Err("openrouter_accounting_rejected".into());
    }
    Ok(reply.body)
}
// Shared credential/reflection/size boundary for ceilings and generation reads.
fn accounting_reply(
    http: &dyn crate::judge_provider::transport::Http,
    secret: &crate::judge_provider::Secret,
    url: &str,
    timeout: std::time::Duration,
) -> std::result::Result<crate::judge_provider::transport::HttpReply, &'static str> {
    let auth = format!("Bearer {}", secret.expose());
    let reply = http
        .get(url, ("Authorization", &auth), timeout)
        .map_err(|_| "openrouter_accounting_unavailable")?;
    if reply.body.len() > 256 * 1024 || secret.reflected(&reply.body) {
        return Err("openrouter_accounting_rejected");
    }
    Ok(reply)
}
/// Fetch both provider accounting endpoints; absent/invalid ceilings fail closed.
pub fn ceiling(
    http: &dyn crate::judge_provider::transport::Http,
    secret: &crate::judge_provider::Secret,
    timeout: std::time::Duration,
) -> std::result::Result<Ceiling, String> {
    let started = std::time::Instant::now();
    let key = accounting(http, secret, "https://openrouter.ai/api/v1/key", timeout);
    let credits = accounting(
        http,
        secret,
        "https://openrouter.ai/api/v1/credits",
        timeout.saturating_sub(started.elapsed()),
    );
    // Both responses must arrive successfully, even when only one supplies a ceiling.
    let (key, credits) = (key?, credits?);
    parse_ceiling(&key, &credits)
}
/// Parse recorded/synthetic API responses without credentials or sockets.
pub fn parse_ceiling(key: &[u8], credits: &[u8]) -> std::result::Result<Ceiling, String> {
    let k: Value = serde_json::from_slice(key).map_err(|_| "openrouter_ceiling_unavailable")?;
    let c: Value = serde_json::from_slice(credits).map_err(|_| "openrouter_ceiling_unavailable")?;
    let negative = |v: &Value| {
        v.as_number()
            .is_some_and(|n| n.to_string().starts_with('-'))
    };
    if negative(&k["data"]["limit"])
        || (!k["data"]["limit"].is_null() && negative(&k["data"]["limit_remaining"]))
        || negative(&k["data"]["usage"])
        || negative(&c["data"]["total_credits"])
        || negative(&c["data"]["total_usage"])
    {
        return Err("openrouter_ceiling_unavailable".into());
    }
    let key_usage = body_amount(key, &["data", "usage"], true);
    let account_usage = body_amount(credits, &["data", "total_usage"], true);
    let key_remaining = if !k["data"]["limit"].is_null() {
        // A present limit cannot fall back to credits when any required key
        // accounting field is malformed, negative or outside the decimal range.
        body_amount(key, &["data", "limit"], false)
            .zip(key_usage)
            .ok_or("openrouter_ceiling_unavailable")?;
        Some(
            body_amount(key, &["data", "limit_remaining"], false)
                .ok_or("openrouter_ceiling_unavailable")?,
        )
    } else {
        None
    };
    let balance = body_amount(credits, &["data", "total_credits"], false)
        .zip(account_usage)
        .map(|(credits, usage)| credits.saturating_sub(usage));
    let remaining = key_remaining
        .into_iter()
        .chain(balance)
        .min()
        .ok_or("openrouter_ceiling_unavailable")?;
    Ok(Ceiling {
        remaining,
        key_usage,
        account_usage,
        hashes: [Digest::of_bytes(key), Digest::of_bytes(credits)],
    })
}
// Four bounded GETs, sleeping 2+4+8 = 14 seconds at most. Each GET is
// capped at five seconds and the entire reconciliation shares a 30-second cap.
const GENERATION_BACKOFF: [u64; 3] = [2, 4, 8];
trait ReconciliationClock {
    fn elapsed(&self) -> std::time::Duration;
    fn sleep(&self, duration: std::time::Duration);
}
struct WallClock(std::time::Instant);
impl ReconciliationClock for WallClock {
    fn elapsed(&self) -> std::time::Duration {
        self.0.elapsed()
    }
    fn sleep(&self, duration: std::time::Duration) {
        std::thread::sleep(duration);
    }
}
/// Sanitized authoritative generation billing and revision identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    /// Settled total cost in nanodollars.
    pub cost_nano_usd: u64,
    /// Hash of the secret-checked generation response.
    pub response_hash: Digest,
    /// Actual generation model, including its dated suffix when supplied.
    pub model: String,
    /// Actual provider route name.
    pub provider_name: String,
}
struct GenerationLookup {
    result: std::result::Result<Generation, &'static str>,
    attempts: u32,
    waited_ms: u64,
}
fn generation_once(
    http: &dyn crate::judge_provider::transport::Http,
    secret: &crate::judge_provider::Secret,
    id: &str,
    timeout: std::time::Duration,
) -> std::result::Result<Generation, &'static str> {
    let encoded: String = id
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let reply = accounting_reply(
        http,
        secret,
        &format!("https://openrouter.ai/api/v1/generation?id={encoded}"),
        timeout,
    )?;
    if reply.status == 404 {
        return Err("openrouter_generation_not_ready");
    }
    if reply.status == 429 || (500..600).contains(&reply.status) {
        return Err("openrouter_accounting_unavailable");
    }
    if !(200..300).contains(&reply.status) {
        return Err("openrouter_accounting_rejected");
    }
    let value: Value =
        serde_json::from_slice(&reply.body).map_err(|_| "openrouter_generation_invalid")?;
    if value["data"].is_null() {
        return Err("openrouter_generation_not_ready");
    }
    if value["data"]["id"] != id {
        return Err("openrouter_generation_identity");
    }
    let cost = body_amount(&reply.body, &["data", "total_cost"], true)
        .ok_or("openrouter_generation_cost_unknown")?;
    let model = value["data"]["model"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
        })
        .ok_or("openrouter_generation_revision_invalid")?;
    let provider = value["data"]["provider_name"]
        .as_str()
        .filter(|s| {
            !s.trim().is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b" -_.:/".contains(&b))
        })
        .ok_or("openrouter_generation_revision_invalid")?;
    Ok(Generation {
        cost_nano_usd: cost,
        response_hash: Digest::of_bytes(&reply.body),
        model: model.into(),
        provider_name: provider.into(),
    })
}
fn lookup_generation(
    http: &dyn crate::judge_provider::transport::Http,
    secret: &crate::judge_provider::Secret,
    id: Option<&str>,
    deadline: std::time::Duration,
    clock: &impl ReconciliationClock,
) -> GenerationLookup {
    use std::time::Duration;
    let mut lookup = GenerationLookup {
        result: Err("openrouter_generation_missing"),
        attempts: 0,
        waited_ms: 0,
    };
    let Some(id) = id.filter(|s| !s.is_empty() && s.len() <= 256) else {
        return lookup;
    };
    lookup.result = Err("openrouter_reconciliation_deadline");
    for attempt in 0..=GENERATION_BACKOFF.len() {
        let remaining = deadline.saturating_sub(clock.elapsed());
        if remaining.is_zero() {
            break;
        }
        lookup.attempts += 1;
        lookup.result = generation_once(http, secret, id, remaining.min(Duration::from_secs(5)));
        if !matches!(
            lookup.result,
            Err("openrouter_generation_not_ready" | "openrouter_accounting_unavailable")
        ) {
            break;
        }
        let Some(delay) = GENERATION_BACKOFF.get(attempt) else {
            break;
        };
        let delay = Duration::from_secs(*delay);
        // Do not sleep unless there will still be time for another lookup.
        if delay >= deadline.saturating_sub(clock.elapsed()) {
            break;
        }
        clock.sleep(delay);
        lookup.waited_ms += delay.as_millis() as u64;
    }
    lookup
}
/// Campaign reconciliation counts. Only every dispatched receipt matched is reconciled.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReconciliationSummary {
    /// Pending receipts, including records still unpublished by the provider.
    pub pending: usize,
    /// Terminal matched receipts.
    pub matched: usize,
    /// Proved error-only refusals settled without generation lookup.
    pub zero_cost_refused: usize,
    /// Operator settlements at full reservation, never provider reconciled.
    pub settled_conservatively: usize,
    /// Terminal billing, identity or revision mismatches.
    pub mismatch: usize,
    /// Aggregate state: pending, matched or mismatch.
    pub state: &'static str,
}
/// Read durable reconciliation state without credentials, dispatch or allowance.
pub fn reconciliation_status(
    ledger: &crate::budget_ledger::Ledger,
) -> std::result::Result<ReconciliationSummary, String> {
    let mut summary = ReconciliationSummary {
        pending: 0,
        matched: 0,
        zero_cost_refused: 0,
        settled_conservatively: 0,
        mismatch: 0,
        state: "pending",
    };
    for receipt in ledger
        .money_receipts()
        .map_err(|_| "openrouter_reconciliation_storage_unavailable")?
    {
        if receipt.outcome == "settled_conservatively" {
            summary.settled_conservatively += 1;
            continue;
        }
        if receipt.usage["openrouter_dispatched"] != true {
            continue;
        }
        match receipt.usage["reconciliation"]["state"].as_str() {
            Some("zero_cost_refused") => summary.zero_cost_refused += 1,
            Some("matched") => summary.matched += 1,
            Some("mismatch") => summary.mismatch += 1,
            _ => summary.pending += 1,
        }
    }
    summary.state = if summary.mismatch > 0 {
        "mismatch"
    } else if summary.settled_conservatively > 0 {
        "settled_conservatively"
    } else if summary.pending == 0 && summary.matched == 0 && summary.zero_cost_refused > 0 {
        "zero_cost_refused"
    } else if summary.pending == 0 && summary.matched > 0 {
        "matched"
    } else {
        "pending"
    };
    Ok(summary)
}
/// Later read-only reconciliation of pending dispatched receipts, without allowance.
/// Matched/mismatched receipts are immutable and skipped on subsequent invocations.
/// Unpublished/transient lookups remain pending; terminal failures stop spending.
/// GETs and backoff share a deadline capped at 30 seconds. Never dispatches completions.
pub fn reconcile(
    transport: &crate::judge_provider::transport::Transport<'_>,
    timeout: std::time::Duration,
) -> std::result::Result<ReconciliationSummary, String> {
    reconcile_pending(transport.ledger, transport.http, transport.keys, timeout)
}
/// Reconcile an existing campaign using only generation GETs and the fixed credential policy.
/// Requires neither a dispatch authorization nor a monetary allowance.
pub fn reconcile_pending(
    ledger: &crate::budget_ledger::Ledger,
    http: &dyn crate::judge_provider::transport::Http,
    keys: &crate::judge_provider::Keys,
    timeout: std::time::Duration,
) -> std::result::Result<ReconciliationSummary, String> {
    reconcile_pending_with_clock(
        ledger,
        http,
        keys,
        timeout,
        &WallClock(std::time::Instant::now()),
    )
}
#[cfg(test)]
fn reconcile_with_clock(
    transport: &crate::judge_provider::transport::Transport<'_>,
    timeout: std::time::Duration,
    clock: &impl ReconciliationClock,
) -> std::result::Result<ReconciliationSummary, String> {
    reconcile_pending_with_clock(
        transport.ledger,
        transport.http,
        transport.keys,
        timeout,
        clock,
    )
}
fn reconcile_pending_with_clock(
    ledger: &crate::budget_ledger::Ledger,
    http: &dyn crate::judge_provider::transport::Http,
    keys: &crate::judge_provider::Keys,
    timeout: std::time::Duration,
    clock: &impl ReconciliationClock,
) -> std::result::Result<ReconciliationSummary, String> {
    let deadline = clock.elapsed() + timeout.min(std::time::Duration::from_secs(30));
    let mut secret = None;
    for receipt in ledger
        .money_receipts()
        .map_err(|_| "openrouter_reconciliation_storage_unavailable")?
    {
        if receipt.outcome == "settled_conservatively"
            || receipt.usage["openrouter_dispatched"] != true
            || matches!(
                receipt.usage["reconciliation"]["state"].as_str(),
                Some("matched" | "mismatch" | "zero_cost_refused")
            )
        {
            continue;
        }
        let secret = match &secret {
            Some(secret) => secret,
            None => secret.insert(
                keys.openrouter()
                    .map_err(|_| "openrouter_credentials_unavailable")?,
            ),
        };
        let attempted_ms = crate::budget_ledger::now_ms();
        let lookup = lookup_generation(
            http,
            secret,
            receipt.usage["generation_id"].as_str(),
            deadline,
            clock,
        );
        ledger
            .record_openrouter_reconciliation(
                &receipt.id,
                lookup.result,
                lookup.attempts,
                lookup.waited_ms,
                attempted_ms,
            )
            .map_err(|_| "openrouter_reconciliation_storage_unavailable")?;
    }
    let summary = reconciliation_status(ledger)?;
    if summary.mismatch > 0 {
        Err("openrouter_reconciliation_failed".into())
    } else {
        Ok(summary)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod deferred_tests {
    use super::*;
    use crate::budget_ledger::{Ledger, MoneyReceipt, MoneyScope};
    use crate::judge_provider::{
        Keys,
        transport::{Http, HttpReply},
    };
    use std::{cell::Cell, time::Duration};

    #[derive(Default)]
    struct Clock(Cell<Duration>);
    impl ReconciliationClock for Clock {
        fn elapsed(&self) -> Duration {
            self.0.get()
        }
        fn sleep(&self, duration: Duration) {
            self.0.set(self.0.get() + duration);
        }
    }
    struct Recorded {
        ready: Cell<u32>,
        gets: Cell<u32>,
    }
    impl Http for Recorded {
        fn get(
            &self,
            url: &str,
            _: (&str, &str),
            _: Duration,
        ) -> std::result::Result<HttpReply, String> {
            self.gets.set(self.gets.get() + 1);
            let id = url
                .strip_prefix("https://openrouter.ai/api/v1/generation?id=")
                .unwrap();
            let ready = self.ready.get() >= if id == "gen-fixture-001" { 1 } else { 2 };
            let body = if ready {
                let mut value: Value = serde_json::from_slice(include_bytes!(
                    "../../tests/fixtures/assist-openrouter/generation-ready.json"
                ))
                .unwrap();
                value["data"]["id"] = json!(id);
                serde_json::to_vec(&value).unwrap()
            } else {
                include_bytes!("../../tests/fixtures/assist-openrouter/generation-not-ready.json")
                    .to_vec()
            };
            Ok(HttpReply {
                status: if ready { 200 } else { 404 },
                retry_after_secs: None,
                body,
            })
        }
        fn post(
            &self,
            _: &str,
            _: (&str, &str),
            _: &[u8],
            _: Duration,
        ) -> std::result::Result<HttpReply, String> {
            panic!("deferred reconciliation must never dispatch")
        }
    }
    #[test]
    fn g12_deferred_404_then_200_reconciliation_is_pending_and_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("openrouter.env"),
            "OPENROUTER_API_KEY=fixture-openrouter-key",
        )
        .unwrap();
        let keys = Keys::assist_fixture(temp.path().into());
        let ledger = Ledger::new(&temp.path().join("ledger"), true);
        for id in ["gen-fixture-001", "gen-fixture-002"] {
            ledger.reserve_money(&[MoneyScope { id: "fixture".into(), cap_nano_usd: 20_000_000 }], MoneyReceipt {
                id: id.into(), request_hash: Digest::of_bytes(id.as_bytes()), scopes: vec!["fixture".into()],
                reserved_nano_usd: 5_000_000, actual_nano_usd: None, outcome: "reserved".into(),
                usage: json!({"openrouter_dispatched":true,"requested_identity":{"model":"google/gemini-3.8-flash","revision":"absent"}}),
            }).unwrap();
            ledger
                .finish_money(id, Some(2_323_500), json!({"generation_id":id}), true)
                .unwrap();
        }
        assert_eq!(reconciliation_status(&ledger).unwrap().pending, 2);
        assert!(
            ledger
                .money_receipts()
                .unwrap()
                .iter()
                .all(|r| r.usage["reconciliation"]["state"] == "pending")
        );
        let original: Value = serde_json::from_slice(
            &std::fs::read(temp.path().join("ledger/campaign.json")).unwrap(),
        )
        .unwrap();
        let http = Recorded {
            ready: Cell::new(0),
            gets: Cell::new(0),
        };
        let run = || {
            reconcile_pending_with_clock(
                &ledger,
                &http,
                &keys,
                Duration::from_secs(30),
                &Clock::default(),
            )
            .unwrap()
        };
        assert_eq!(run().pending, 2);
        for r in ledger.money_receipts().unwrap() {
            let attempt = &r.usage["reconciliation"];
            assert_eq!(attempt["state"], "pending");
            assert_eq!(attempt["reason"], "openrouter_generation_not_ready");
            assert!(attempt["attempted_ms"].as_u64().is_some());
            assert_eq!(r.usage["qualification_eligible"], false);
        }
        http.ready.set(1);
        let partial = run();
        assert_eq!(
            (partial.pending, partial.matched, partial.state),
            (1, 1, "pending")
        );
        http.ready.set(2);
        let final_state = run();
        assert_eq!(
            (final_state.pending, final_state.matched, final_state.state),
            (0, 2, "matched")
        );
        let before = std::fs::read(temp.path().join("ledger/campaign.json")).unwrap();
        let gets = http.gets.get();
        // Terminal replay does not even read a credential file.
        let missing_keys = Keys::assist_fixture(temp.path().join("missing-keys"));
        assert_eq!(
            reconcile_pending(&ledger, &http, &missing_keys, Duration::from_secs(30)).unwrap(),
            final_state
        );
        assert_eq!(http.gets.get(), gets);
        assert_eq!(
            std::fs::read(temp.path().join("ledger/campaign.json")).unwrap(),
            before
        );
        let final_ledger: Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(
            original["money"]["counters"],
            final_ledger["money"]["counters"]
        );
        assert_eq!(original["scopes"], final_ledger["scopes"]);
        assert_eq!(final_ledger["money"]["stopped"], false);
        assert_eq!(
            ledger.money_receipts().unwrap()[0].usage["reconciliation"]["history"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod ceiling_tests {
    use super::*;
    #[test]
    fn ceiling_uses_provider_min_null_limit_credit_only_and_exact_rounding() {
        let key = br#"{"data":{"limit":2,"limit_remaining":0.99999999999999999,"usage":0}}"#;
        let credits = br#"{"data":{"total_credits":5,"total_usage":1}}"#;
        let snapshot = parse_ceiling(key, credits).unwrap();
        assert_eq!(snapshot.remaining, 999_999_999);
        assert_eq!(snapshot.hashes[0], Digest::of_bytes(key));
        assert_eq!(
            parse_ceiling(
                br#"{"data":{"limit":null,"limit_remaining":0,"usage":0}}"#,
                credits
            )
            .unwrap()
            .remaining,
            4_000_000_000
        );
        assert_eq!(
            parse_ceiling(key, br#"{"data":{"total_credits":0,"total_usage":1}}"#)
                .unwrap()
                .remaining,
            0
        );
        assert_eq!(
            parse_ceiling(
                key,
                br#"{"data":{"total_credits":0.000000001,"total_usage":0.0000000000001}}"#
            )
            .unwrap()
            .remaining,
            0
        );
        assert!(parse_ceiling(br#"{"data":{}}"#, br#"{"data":{}}"#).is_err());
        assert!(
            parse_ceiling(
                br#"{"data":{"limit":1,"limit_remaining":-1,"usage":0}}"#,
                credits
            )
            .is_err()
        );
        assert!(parse_ceiling(key, br#"{"data":{"total_credits":-1,"total_usage":0}}"#).is_err());
        assert!(
            parse_ceiling(
                br#"{"data":{"limit":"invalid","limit_remaining":1,"usage":0}}"#,
                credits
            )
            .is_err()
        );
        assert_eq!(decimal_amount("1e-9", false), Some(1));
        assert_eq!(decimal_amount("0.00000000001", true), Some(1));
        assert_eq!(decimal_amount("0.00000000001", false), Some(0));
        for invalid in ["NaN", "Infinity", "-1", "1e100", "1e-100", ""] {
            assert!(decimal_amount(invalid, false).is_none());
        }
    }
    #[test]
    fn g12_non_null_malformed_key_limit_never_falls_back_to_credits() {
        let credits = br#"{"data":{"total_credits":20,"total_usage":0}}"#;
        for key in [
            br#"{"data":{"limit":"5","limit_remaining":5,"usage":0}}"#.as_slice(),
            br#"{"data":{"limit":5,"limit_remaining":"5","usage":0}}"#,
            br#"{"data":{"limit":5,"limit_remaining":5,"usage":"0"}}"#,
            br#"{"data":{"limit":5,"usage":0}}"#,
            br#"{"data":{"limit":5,"limit_remaining":5}}"#,
            br#"{"data":{"limit":-5,"limit_remaining":5,"usage":0}}"#,
            br#"{"data":{"limit":5,"limit_remaining":-5,"usage":0}}"#,
            br#"{"data":{"limit":5,"limit_remaining":5,"usage":-1}}"#,
            br#"{"data":{"limit":1e100,"limit_remaining":5,"usage":0}}"#,
            br#"{"data":{"limit":5,"limit_remaining":5,"usage":1e100}}"#,
            br#"{"data":{"limit":5,"limit_remaining":"Infinity","usage":0}}"#,
        ] {
            assert_eq!(
                parse_ceiling(key, credits).unwrap_err(),
                "openrouter_ceiling_unavailable"
            );
        }
        assert_eq!(
            parse_ceiling(br#"{"data":{"limit":null,"usage":0}}"#, credits)
                .unwrap()
                .remaining,
            20_000_000_000
        );
    }
    #[test]
    fn permit_is_single_use_payload_bound_and_raw_network_remains_refused() {
        assert!(
            DispatchPermit::new(Digest::of_bytes(b"one"))
                .check(b"two")
                .is_err()
        );
        let permit = DispatchPermit {
            hash: Digest::of_bytes(b"one"),
            created: std::time::Instant::now() - std::time::Duration::from_secs(2),
        };
        assert!(permit.check(b"one").is_err());
        assert!(!accounting_url("https://example.org/api/v1/key"));
        assert!(!accounting_url(
            "https://openrouter.ai/api/v1/generation?id=x&other=y"
        ));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod outstanding_tests {
    use super::*;
    use crate::budget_ledger::{Ledger, MoneyReceipt, MoneyScope};
    #[test]
    fn g12_settled_unreported_spend_is_subtracted_before_dispatch() {
        for (key_usage, account_usage, admitted) in [
            (0, 0, false),
            (60_000_000, 0, false),
            (60_000_000, 60_000_000, true),
            (30_000_000, 30_000_000, false),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(temp.path(), true);
            let baseline = parse_ceiling(
                br#"{"data":{"limit":1,"limit_remaining":0.2,"usage":0}}"#,
                br#"{"data":{"total_credits":0.2,"total_usage":0}}"#,
            )
            .unwrap();
            ledger
                .openrouter_preflight(200_000_000, || Ok(baseline.clone()))
                .unwrap();
            let scopes = vec![MoneyScope {
                id: "smoke".into(),
                cap_nano_usd: 200_000_000,
            }];
            for (id, amount) in [("first", 60_000_000), ("next", 115_000_000)] {
                ledger
                    .reserve_money(
                        &scopes,
                        MoneyReceipt {
                            id: id.into(),
                            request_hash: Digest::of_bytes(id.as_bytes()),
                            scopes: vec!["smoke".into()],
                            reserved_nano_usd: amount,
                            actual_nano_usd: None,
                            outcome: "reserved".into(),
                            usage: Value::Null,
                        },
                    )
                    .unwrap();
                if id == "first" {
                    ledger
                        .openrouter_dispatch_check(Digest::of_bytes(b"first"), || {
                            Ok(baseline.clone())
                        })
                        .unwrap();
                    ledger
                        .finish_money(id, Some(amount), json!({}), true)
                        .unwrap();
                }
            }
            let mut fresh = baseline;
            // First call is settled; another consumer has removed the same $0.06
            // while provider usage can still be at baseline. Remaining is $0.14.
            fresh.remaining = 140_000_000;
            fresh.key_usage = Some(key_usage);
            fresh.account_usage = Some(account_usage);
            let result = ledger.openrouter_dispatch_check(Digest::of_bytes(b"next"), || Ok(fresh));
            assert_eq!(result.is_ok(), admitted);
            if !admitted {
                assert_eq!(result.err().unwrap(), "openrouter_remaining_exhausted");
                assert!(
                    ledger
                        .openrouter_preflight(200_000_000, || unreachable!())
                        .is_err()
                );
                assert!(
                    std::fs::read_to_string(temp.path().join("campaign.json"))
                        .unwrap()
                        .contains("openrouter_remaining_exhausted")
                );
            }
        }
    }
    #[test]
    fn fresh_ceiling_subtracts_all_outstanding_campaign_reservations() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), true);
        let baseline = parse_ceiling(
            br#"{"data":{"limit":1,"limit_remaining":1,"usage":0}}"#,
            br#"{"data":{"total_credits":1,"total_usage":0}}"#,
        )
        .unwrap();
        ledger
            .openrouter_preflight(200_000_000, || Ok(baseline.clone()))
            .unwrap();
        let scopes = vec![MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: 200_000_000,
        }];
        for id in ["first", "second"] {
            ledger
                .reserve_money(
                    &scopes,
                    MoneyReceipt {
                        id: id.into(),
                        request_hash: Digest::of_bytes(id.as_bytes()),
                        scopes: vec!["smoke".into()],
                        reserved_nano_usd: 80_000_000,
                        actual_nano_usd: None,
                        outcome: "reserved".into(),
                        usage: Value::Null,
                    },
                )
                .unwrap();
        }
        let mut fresh = baseline;
        fresh.remaining = 100_000_000;
        // Each individual reservation fits; their combined outstanding charge does not.
        assert!(
            ledger
                .openrouter_dispatch_check(Digest::of_bytes(b"second"), || Ok(fresh))
                .is_err()
        );
        assert!(
            ledger
                .money_receipts()
                .unwrap()
                .iter()
                .all(|r| r.actual_nano_usd.is_none() && r.reserved_nano_usd > 0)
        );
        assert!(
            std::fs::read_to_string(temp.path().join("campaign.json"))
                .unwrap()
                .contains("openrouter_remaining_exhausted")
        );
        assert!(
            ledger
                .openrouter_preflight(200_000_000, || unreachable!(
                    "stopped campaign must not fetch"
                ))
                .is_err()
        );
    }
}
