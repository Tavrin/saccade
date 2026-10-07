//! Explicitly labelled local Jev allowance. No provider billing ceiling is implied.
use super::{Result, decode, require, schema::JEV};
use crate::evidence::canonical::Digest;
use serde_json::{Value, json};

/// All Jev monetary evidence carries this limitation.
pub const CEILING: &str = "local_allowance_not_provider_verified";
/// Frozen public tariff, not an observed debit.
pub const PRICE_ID: &str = "jev-input-42-nano-output-free/2026-10-07";
/// Public source for the frozen tariff.
pub const PRICE_SOURCE: &str = "https://docs.typesafe.ai/models";
/// Price expires 2026-11-07, requiring a reviewed refresh.
pub const EXPIRES_MS: u64 = 1_794_009_600_000;
/// Default one-dollar local campaign allowance.
pub const DEFAULT_ALLOWANCE: u64 = 1_000_000_000;
/// Absolute five-dollar local allowance maximum.
pub const MAX_ALLOWANCE: u64 = 5_000_000_000;
/// Fixed endpoint; no project-supplied URL is accepted.
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";

/// Human assertion only; this never verifies account settings.
pub struct LocalAllowance {
    /// Operator attests prepaid credits with automatic refill disabled.
    pub prepaid_no_refill_attested: bool,
    /// Durable campaign scope, shared by all workload calls and variants.
    pub campaign: String,
    /// Positive allowance, at most five dollars.
    pub nano_usd: u64,
}
impl LocalAllowance {
    /// Validate before any credential access or dispatch.
    pub fn validate(&self) -> Result<()> {
        require(
            self.prepaid_no_refill_attested,
            "Jev prepaid no-refill attestation required",
        )?;
        require(
            !self.campaign.is_empty()
                && self.campaign.len() <= 128
                && self
                    .campaign
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Jev campaign identity",
        )?;
        require(
            self.nano_usd > 0 && self.nano_usd <= MAX_ALLOWANCE,
            "Jev local allowance maximum",
        )
    }
}
/// Bound includes every serialized UTF-8 byte plus 4096 framing tokens.
/// This is a conservative local assumption, not a provider billing guarantee.
pub fn input_bound(payload: &[u8]) -> Result<u64> {
    let v: Value = decode(payload)?;
    require(
        v["model"] == JEV
            && v["state"].is_object()
            && v["questions"]
                .as_object()
                .is_some_and(|q| !q.is_empty() && q.len() <= 16),
        "Jev native payload",
    )?;
    let bound = (payload.len() as u64)
        .checked_add(4096)
        .ok_or(super::Error::Invalid("Jev payload overflow"))?;
    require(bound <= 32_000, "Jev oversized payload")?;
    Ok(bound)
}
/// Documented model ID is the available revision; optional stronger exposure is retained.
pub fn identity(value: &Value, revision: &str) -> Result<String> {
    require(
        value["model"] == JEV,
        "Jev model identity missing or mismatched",
    )?;
    let exposed = match value.get("modelVersion") {
        Some(v) => v
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(super::Error::Invalid("Jev malformed revision"))?,
        None => JEV,
    };
    require(exposed == revision, "Jev revision drift")?;
    Ok(exposed.into())
}
/// Strict documented native choice contract used by the live evaluation runner.
pub fn choice<'a>(value: &'a Value, choices: &[&str]) -> Result<&'a str> {
    identity(value, JEV)?;
    require(
        value["answers"]["q"]["type"] == "choice"
            && value["answers"]["q"]["confidence"].as_f64().is_some()
            && value["answers"]["q"]["probabilities"].is_object(),
        "Jev native choice fields",
    )?;
    let (normalised, _) = normalised_answer(value, choices)?;
    super::workflow::closed_choice(&normalised, choices)?;
    value["answers"]["q"]["choice"]
        .as_str()
        .ok_or(super::Error::Invalid("Jev choice"))
}
/// Normalise rounded distributions without modifying the raw response evidence.
/// Metadata retains the original sum and whether the answer was renormalised.
pub fn normalised_answer(value: &Value, choices: &[&str]) -> Result<(Value, Value)> {
    let p = value["answers"]["q"]["probabilities"]
        .as_object()
        .ok_or(super::Error::Invalid("Jev probabilities object"))?;
    require(
        p.len() == choices.len() && choices.iter().all(|k| p.contains_key(*k)),
        "Jev probability keys",
    )?;
    let mut sum = 0.0;
    for v in p.values() {
        let n = v
            .as_f64()
            .ok_or(super::Error::Invalid("Jev probability number"))?;
        require(
            n.is_finite() && (0.0..=1.0).contains(&n),
            "Jev probability range",
        )?;
        sum += n;
    }
    require((sum - 1.0).abs() <= 0.02 + 1e-12, "Jev probability sum")?;
    let mut normalised = value.clone();
    for v in normalised["answers"]["q"]["probabilities"]
        .as_object_mut()
        .ok_or(super::Error::Invalid("Jev probabilities object"))?
        .values_mut()
    {
        *v = json!(
            v.as_f64()
                .ok_or(super::Error::Invalid("Jev probability number"))?
                / sum
        );
    }
    super::workflow::closed_choice(&normalised, choices)?;
    let metadata = json!({"original_sum":sum,"renormalised":sum != 1.0,"probabilities":normalised["answers"]["q"]["probabilities"]});
    Ok((normalised, metadata))
}
/// Strict numeric Noul answer; no truthy strings or synthetic booleans.
pub fn noul(value: &Value) -> Result<f64> {
    identity(value, JEV)?;
    let a = &value["answers"]["q"];
    require(
        a.as_object().is_some_and(|o| o.len() == 2) && a["type"] == "noul",
        "Jev native Noul type",
    )?;
    a["noul"]
        .as_f64()
        .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
        .ok_or(super::Error::Invalid("Jev native Noul range"))
}
/// Missing usage retains the reservation. Present malformed usage is refused.
pub fn settlement(value: &Value, bound: u64) -> Result<(u64, bool)> {
    let Some(usage) = value.get("usage") else {
        return Ok((bound * 42, false));
    };
    require(usage.is_object(), "Jev malformed usage")?;
    let input = usage["input_tokens"]
        .as_u64()
        .ok_or(super::Error::Invalid("Jev malformed input usage"))?;
    require(
        usage["output_tokens"].as_u64().is_some(),
        "Jev malformed output usage",
    )?;
    Ok((
        input
            .checked_mul(42)
            .ok_or(super::Error::Invalid("Jev usage overflow"))?,
        input > bound,
    ))
}
/// Single-use permit constructed only after shared ledger reservation.
pub struct DispatchPermit {
    hash: Digest,
}
impl DispatchPermit {
    pub(super) fn reserved(payload: &[u8]) -> Self {
        Self {
            hash: Digest::of_bytes(payload),
        }
    }
    /// The HTTP boundary rejects changed bytes and stale pricing.
    pub fn check(self, payload: &[u8]) -> std::result::Result<(), String> {
        if self.hash != Digest::of_bytes(payload) || crate::budget_ledger::now_ms() >= EXPIRES_MS {
            return Err("jev_dispatch_permit_refused".into());
        }
        input_bound(payload)
            .map(|_| ())
            .map_err(|_| "jev_dispatch_permit_refused".into())
    }
}
/// Offline parser and monetary self-test, repeated before each authorized live call.
pub fn self_test() -> Result<Value> {
    let native = json!({"model":JEV,"answers":{"q":{"type":"choice","confidence":1.0,"choice":"vision","probabilities":{"vision":1.0,"insufficient":0.0}}},"usage":{"input_tokens":12,"output_tokens":4}});
    require(
        identity(&native, JEV)? == JEV
            && super::routing::answer(
                &serde_json::to_vec(&native).map_err(|_| super::Error::Storage)?,
                JEV,
            )? == super::routing::Decision::Vision,
        "Jev self-test native parser",
    )?;
    require(
        settlement(&native, 5000)? == (504, false),
        "Jev self-test usage",
    )?;
    let mut corrupt = native.clone();
    corrupt["model"] = json!("jev-latest");
    require(identity(&corrupt, JEV).is_err(), "Jev self-test identity")?;
    corrupt = native.clone();
    corrupt["usage"]["input_tokens"] = json!(-1);
    require(
        settlement(&corrupt, 5000).is_err(),
        "Jev self-test negative usage",
    )?;
    corrupt = native.clone();
    corrupt["answers"]["q"]["choice"] = json!("approve");
    require(
        super::routing::answer(
            &serde_json::to_vec(&corrupt).map_err(|_| super::Error::Storage)?,
            JEV,
        )
        .is_err(),
        "Jev self-test unknown choice",
    )?;
    for sum in [0.97, 0.98, 0.99, 1.0, 1.01, 1.02, 1.03] {
        let mut rounded = native.clone();
        rounded["answers"]["q"]["probabilities"] = json!({"vision":sum-0.1,"insufficient":0.1});
        require(
            choice(&rounded, &["vision", "insufficient"]).is_ok() == (0.98..=1.02).contains(&sum),
            "Jev self-test rounded probabilities",
        )?;
    }
    Ok(
        json!({"ceiling":CEILING,"native_parser":"pass","identity":"pass","negative_usage":"pass","unknown_choice":"pass","qualified":false}),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        assist::execution::{CacheKey, ENCODER, Executor},
        budget_ledger::{Caps, Ledger, MoneyScope, Scope},
        judge_provider::{Keys, transport::*},
        root_policy::RootPolicy,
    };
    use std::{
        cell::Cell,
        collections::BTreeMap,
        time::{Duration, Instant},
    };

    #[test]
    fn native_contract_and_conservative_admission() {
        assert!(self_test().is_ok());
        let request = serde_json::to_vec(
            &json!({"model":JEV,"state":{},"questions":{"q":{"type":"choice"}}}),
        )
        .unwrap();
        assert_eq!(input_bound(&request).unwrap(), request.len() as u64 + 4096);
        assert!(input_bound(&vec![b'x'; 32000]).is_err());
        assert_eq!(settlement(&json!({}), 5000).unwrap(), (210000, false));
        assert!(settlement(&json!({"usage":{}}), 5000).is_err());
        assert_eq!(
            settlement(
                &json!({"usage":{"input_tokens":5001,"output_tokens":0}}),
                5000
            )
            .unwrap(),
            (210042, true)
        );
        assert!(identity(&json!({"model":JEV}), "other").is_err());
        assert!(
            LocalAllowance {
                prepaid_no_refill_attested: true,
                campaign: "test".into(),
                nano_usd: MAX_ALLOWANCE + 1
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn dispatch_requires_attestation_and_reservation_and_retains_ambiguous_cost() {
        struct Fake<'a> {
            ledger: &'a Ledger,
            calls: Cell<u64>,
            body: Vec<u8>,
            status: u16,
        }
        impl Http for Fake<'_> {
            fn post(
                &self,
                url: &str,
                _header: (&str, &str),
                _payload: &[u8],
                _timeout: Duration,
            ) -> std::result::Result<HttpReply, String> {
                assert_eq!(url, ENDPOINT);
                let receipts = self.ledger.money_receipts().unwrap();
                assert_eq!(receipts.last().unwrap().outcome, "reserved");
                assert_eq!(receipts.last().unwrap().usage["ceiling"], CEILING);
                self.calls.set(self.calls.get() + 1);
                if self.status == 0 {
                    return Err("transport_timeout".into());
                }
                Ok(HttpReply {
                    status: self.status,
                    retry_after_secs: None,
                    body: self.body.clone(),
                })
            }
        }
        for (index, body, status, success, actual) in [
            (
                0,
                json!({"model":JEV,"answers":{"q":{"choice":"vision"}},"usage":{"input_tokens":12,"output_tokens":2}}),
                200,
                true,
                Some(504),
            ),
            (
                1,
                json!({"model":JEV,"answers":{"q":{"choice":"vision"}}}),
                200,
                true,
                None,
            ),
            (
                2,
                json!({"model":JEV,"usage":{"input_tokens":-1,"output_tokens":2}}),
                200,
                false,
                None,
            ),
            (
                3,
                json!({"model":"jev-latest","usage":{"input_tokens":12,"output_tokens":2}}),
                200,
                false,
                Some(504),
            ),
            (
                4,
                json!({"model":JEV,"usage":{"input_tokens":32001,"output_tokens":2}}),
                200,
                false,
                Some(1344042),
            ),
            (
                5,
                json!({"usage":{"input_tokens":12,"output_tokens":2}}),
                500,
                false,
                None,
            ),
            (6, json!({}), 0, false, None),
        ] {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(
                temp.path().join("jev.env"),
                "JEV_API_KEY=offline-fixture-key",
            )
            .unwrap();
            let keys = Keys::assist_fixture(temp.path().into());
            let user = UserConfig {
                roots: vec![RootSetting {
                    id: "fixture".into(),
                    path: temp.path().into(),
                    egress: EgressSetting::Allow,
                }],
                ..Default::default()
            };
            let mut roots = RootPolicy::new(&[temp.path().into()], None, false, &[]).unwrap();
            user.apply(&mut roots).unwrap();
            let auth = Authorization {
                enabled: true,
                scopes: vec![Scope {
                    id: "fixture".into(),
                    caps: Caps {
                        total: 8,
                        providers: BTreeMap::from([("jev".into(), 8)]),
                    },
                }],
            };
            let ledger = Ledger::new(&temp.path().join("ledger"), true);
            let fake = Fake {
                ledger: &ledger,
                calls: Cell::new(0),
                body: serde_json::to_vec(&body).unwrap(),
                status,
            };
            let transport = Transport {
                user: &user,
                roots: &roots,
                authorization: &auth,
                ledger: &ledger,
                keys: &keys,
                http: &fake,
            };
            let payload = serde_json::to_vec(
                &json!({"model":JEV,"state":{},"questions":{"q":{"type":"choice"}}}),
            )
            .unwrap();
            let key = CacheKey {
                evidence_hash: Digest::of_bytes(&payload),
                payload_hash: Digest::of_bytes(&payload),
                prompt_hash: Digest::of_bytes(b"prompt"),
                encoder_version: ENCODER.into(),
                provider: "jev".into(),
                model: JEV.into(),
                revision: JEV.into(),
                settings: json!({}),
                api_config_hash: Digest::of_bytes(b"fixture"),
                order: "route".into(),
            };
            let executor = Executor {
                transport: &transport,
                ledger: &ledger,
                money_scopes: vec![MoneyScope {
                    id: "fixture".into(),
                    cap_nano_usd: DEFAULT_ALLOWANCE,
                }],
                sources: vec![crate::paths::portable(temp.path())],
                deadline: Instant::now() + Duration::from_secs(30),
            };
            let mut allowance = LocalAllowance {
                prepaid_no_refill_attested: false,
                campaign: "fixture".into(),
                nano_usd: DEFAULT_ALLOWANCE,
            };
            assert!(executor.call_jev(&key, &payload, &allowance).is_err());
            assert_eq!(fake.calls.get(), 0);
            allowance.prepaid_no_refill_attested = true;
            allowance.nano_usd = 1;
            assert!(executor.call_jev(&key, &payload, &allowance).is_err());
            assert_eq!(fake.calls.get(), 0);
            allowance.campaign = "admitted".into();
            allowance.nano_usd = DEFAULT_ALLOWANCE;
            let result = executor.call_jev(&key, &payload, &allowance);
            assert_eq!(result.is_ok(), success, "fixture {index}");
            assert_eq!(fake.calls.get(), 1);
            let receipts = ledger.money_receipts().unwrap();
            let receipt = receipts.last().unwrap();
            assert_eq!(receipt.usage["ceiling"], CEILING);
            assert_eq!(receipt.usage["price_date"], "2026-10-07");
            let conservative = input_bound(&payload).unwrap() * 42;
            assert_eq!(
                receipt.actual_nano_usd,
                if index == 1 {
                    Some(conservative)
                } else {
                    actual
                }
            );
            if [2, 3, 4].contains(&index) {
                assert!(executor.call_jev(&key, &payload, &allowance).is_err());
                assert_eq!(fake.calls.get(), 1);
            }
        }
    }
}
