//! Explicit OpenRouter chat-completions dialect. Live dispatch requires fresh provider accounting.
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
            gets: Cell<u32>,
            scenario: usize,
        }
        impl Http for Fake<'_> {
            fn get(
                &self,
                url: &str,
                _: (&str, &str),
                _: Duration,
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
                    body = json!({"data":{"id":"gen-fixture-001","total_cost":if self.scenario == 7 {0.001} else if self.scenario == 10 {0.0} else {0.0001875}}});
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
        for index in 0..11 {
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
            if index == 10 {
                value["usage"]["cost"] = json!(0);
            }
            if index == 2 {
                value["usage"]["prompt_tokens"] = json!(16001);
                value["usage"]["total_tokens"] = json!(16031);
            }
            let fake = Fake {
                ledger: &ledger,
                calls: Cell::new(0),
                body: serde_json::to_vec(&value).unwrap(),
                gets: Cell::new(0),
                scenario: index,
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
                &json!({"model":"openai/fixture-model","max_tokens":4096,"messages":[{"role":"user","content":"fixture"}],"temperature":0,"response_format":{"type":"json_object"},"provider":{"allow_fallbacks":false,"require_parameters":true},"usage":{"include":true}}),
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
            let campaign =
                std::fs::read_to_string(temp.path().join("ledger/campaign.json")).unwrap();
            assert!(!campaign.contains("fixture-openrouter-key"));
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
            assert_eq!(fake.calls.get(), 1);
            let reconciliation = reconcile(&transport, Duration::from_secs(1));
            assert_eq!(reconciliation.is_ok(), ![1, 7].contains(&index));
            assert_eq!(
                ledger.money_receipts().unwrap()[0].usage["reconciliation"]["matches"],
                json!(![1, 7].contains(&index))
            );

            if index == 2 {
                assert!(result.is_err());
                assert_eq!(receipt.actual_nano_usd, Some(187_500));
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
                    if index == 1 {
                        None
                    } else if index == 10 {
                        Some(0)
                    } else {
                        Some(187_500)
                    }
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
    let v: Value = decode(payload)?;
    require(
        v.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "model",
                    "messages",
                    "temperature",
                    "max_tokens",
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
            && v["response_format"] == json!({"type":"json_object"})
            && v["provider"] == json!({"allow_fallbacks":false,"require_parameters":true})
            && v["usage"] == json!({"include":true})
            && v["max_tokens"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= super::execution::OUTPUT_LIMIT),
        "OpenRouter closed request shape",
    )
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
    let auth = format!("Bearer {}", secret.expose());
    let reply = http
        .get(url, ("Authorization", &auth), timeout)
        .map_err(|_| "openrouter_accounting_unavailable")?;
    if !(200..300).contains(&reply.status)
        || reply.body.len() > 256 * 1024
        || secret.reflected(&reply.body)
    {
        return Err("openrouter_accounting_rejected".into());
    }
    Ok(reply.body)
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
    let key_remaining = if !k["data"]["limit"].is_null()
        && body_amount(key, &["data", "limit"], false).is_some()
        && key_usage.is_some()
    {
        body_amount(key, &["data", "limit_remaining"], false)
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
/// Reconcile every dispatched monetary receipt, including failed/unknown calls.
/// Unavailable or mismatched generations stop spending and remain recorded failures.
pub fn reconcile(
    transport: &crate::judge_provider::transport::Transport<'_>,
    timeout: std::time::Duration,
) -> std::result::Result<(), String> {
    let secret = transport
        .keys
        .openrouter()
        .map_err(|_| "openrouter_credentials_unavailable")?;
    let mut failed = false;
    for receipt in transport.ledger.money_receipts()? {
        if receipt.usage["openrouter_dispatched"] != true {
            continue;
        }
        let id = receipt.usage["generation_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256);
        let result = id
            .ok_or("openrouter_generation_missing".to_string())
            .and_then(|id| {
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
                let body = accounting(
                    transport.http,
                    &secret,
                    &format!("https://openrouter.ai/api/v1/generation?id={encoded}"),
                    timeout,
                )?;
                let value: Value =
                    serde_json::from_slice(&body).map_err(|_| "openrouter_generation_invalid")?;
                if value["data"]["id"] != id {
                    return Err("openrouter_generation_identity".into());
                }
                let cost = body_amount(&body, &["data", "total_cost"], true)
                    .ok_or("openrouter_generation_cost_unknown")?;
                Ok((cost, Digest::of_bytes(&body)))
            });
        let matches = result.as_ref().is_ok_and(|(cost, _)| {
            receipt
                .actual_nano_usd
                .is_some_and(|actual| actual.abs_diff(*cost) <= RECONCILIATION_TOLERANCE)
        });
        transport
            .ledger
            .record_openrouter_reconciliation(&receipt.id, result.ok(), matches)?;
        failed |= !matches;
    }
    if failed {
        Err("openrouter_reconciliation_failed".into())
    } else {
        Ok(())
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
                br#"{}"#
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
