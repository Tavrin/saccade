//! Explicit OpenRouter chat-completions dialect. Live billing is disabled pending verified limits.
use super::{Result, decode, require, workflow::WireAnswer};
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
/// Billing source: OpenRouter passes provider prices through without markup.
pub const PRICE_VERSION: &str = "openrouter-recorded-prices/2026-10-06-v1";
/// Fixed API dialect; credentials can never be redirected by a project.
pub const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// Build chat messages from the same anonymous image extraction packet.
pub fn request(gemini: &[u8], model: &str) -> Result<Vec<u8>> {
    require(
        !model.is_empty()
            && model.len() <= 128
            && model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b)),
        "OpenRouter model ID",
    )?;
    let source: Value = decode(gemini)?;
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
    crate::evidence::canonical::bytes(&json!({"model":model,"messages":[{"role":"system","content":source["systemInstruction"]["parts"][0]["text"]},{"role":"user","content":content}],"temperature":0,"max_tokens":4096,"response_format":{"type":"json_object"},"provider":{"allow_fallbacks":false,"require_parameters":true},"usage":{"include":true}})).map_err(|_|super::Error::Invalid("OpenRouter payload"))
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
    let revision = value["system_fingerprint"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(super::Error::Invalid("OpenRouter returned revision"))?;
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
            revision: revision.into(),
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
        for field in ["usage", "provider", "system_fingerprint", "id"] {
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
    fn chat_request_has_chat_messages_bounded_output_and_pinned_routing() {
        let source = json!({"systemInstruction":{"parts":[{"text":"untrusted data"}]},"contents":[{"parts":[{"text":"P1"},{"inline_data":{"data":"fixture-base64"}}]}]});
        let bytes = request(
            &serde_json::to_vec(&source).unwrap(),
            "openai/fixture-model",
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
    use crate::assist::{
        execution::{CacheKey, ENCODER, Executor},
        schema::Usage,
    };
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
        struct Fake<'a> {
            ledger: &'a Ledger,
            calls: Cell<u32>,
            body: Vec<u8>,
        }
        impl Http for Fake<'_> {
            fn post(
                &self,
                url: &str,
                _: (&str, &str),
                _: &[u8],
                _: Duration,
            ) -> std::result::Result<HttpReply, String> {
                assert_eq!(url, ENDPOINT);
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
                Ok(HttpReply {
                    status: 200,
                    retry_after_secs: None,
                    body: self.body.clone(),
                })
            }
        }
        for index in 0..3 {
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
            if index == 1 {
                value["usage"] = json!({"cost":0});
            }
            if index == 2 {
                value["usage"]["prompt_tokens"] = json!(16001);
                value["usage"]["total_tokens"] = json!(16031);
            }
            let fake = Fake {
                ledger: &ledger,
                calls: Cell::new(0),
                body: serde_json::to_vec(&value).unwrap(),
            };
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
                deadline: Instant::now() + Duration::from_secs(1),
            };
            let payload = serde_json::to_vec(
                &json!({"model":"openai/fixture-model","max_tokens":4096,"messages":[]}),
            )
            .unwrap();
            let key = CacheKey {
                evidence_hash: Digest::of_bytes(b"fixture-request"),
                payload_hash: Digest::of_bytes(&payload),
                prompt_hash: Digest::of_bytes(b"fixture"),
                encoder_version: ENCODER.into(),
                provider: "openrouter".into(),
                model: "openai/fixture-model".into(),
                revision: "fixture-revision-1".into(),
                settings: json!({}),
                api_config_hash: Digest::of_bytes(b"user"),
                order: "single".into(),
            };
            let result = executor.call(&key, &payload);
            let receipts = ledger.money_receipts().unwrap();
            let receipt = &receipts[0];
            assert_eq!(fake.calls.get(), 1);
            if index == 2 {
                assert!(result.is_err());
                assert_eq!(receipt.actual_nano_usd, Some(12_113_250));
                assert_eq!(receipt.outcome, "usage_limit_exceeded");
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
                    if index == 0 { Some(187_500) } else { None }
                );
                if index == 1 {
                    assert!(result.provenance.cost_usd.is_none());
                    assert!(receipt.reserved_nano_usd > 0);
                    assert_ne!(result.provenance.usage, Usage::default());
                }
            }
        }
    }
}
