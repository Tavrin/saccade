//! Bounded transport smoke over an explicitly reviewed request file; no qualification.
use saccade_core::{
    assist::{
        self,
        execution::{CacheKey, ENCODER, Executor},
        openrouter,
    },
    budget_ledger::{Caps, Ledger, MoneyScope, Scope},
    evidence::canonical::Digest,
    judge_provider::{
        Keys,
        transport::{Authorization, Network, Transport, UserConfig},
    },
    root_policy::RootPolicy,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    root: String,
    model: String,
    revision: String,
    payload: Value,
}
fn allowance(text: &str, above_25: bool) -> Result<u64, &'static str> {
    let cap = openrouter::decimal_amount(text, false)
        .filter(|n| *n > 0 && *n <= 30_000_000_000)
        .ok_or("cap must be positive and at most campaign parent 30 USD")?;
    if openrouter::decimal_amount(text, true) != Some(cap) {
        return Err("cap precision exceeds nanodollars");
    }
    if cap > 25_000_000_000 && !above_25 {
        return Err("spend_cap_above_25_requires_explicit_flag");
    }
    Ok(cap)
}
// All roots must fit at their worst-case pinned price before credentials,
// accounting, artifact creation or the first dispatch can occur.
fn validate_rows(rows: &[Row], count: usize, cap: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut ids = BTreeSet::new();
    if rows.len() != count
        || rows.iter().any(|r| {
            r.root.is_empty()
                || !ids.insert(&r.root)
                || r.model.ends_with(":batch")
                || r.revision.is_empty()
        })
    {
        return Err("invalid reviewed smoke topology or unsupported batch arm".into());
    }
    let mut total = 0u64;
    for row in rows {
        let bytes = serde_json::to_vec(&row.payload)?;
        total = total
            .checked_add(openrouter::admission(&bytes, &row.model)?.reservation)
            .ok_or("smoke_reservation_overflow")?;
    }
    if total > cap {
        return Err("smoke_reservations_exceed_cap".into());
    }
    Ok(())
}
// Preserve the existing stop-on-first-failure policy and account for every root.
fn root_outcomes(
    rows: &[Row],
    mut call: impl FnMut(usize) -> assist::Result<()>,
) -> (Vec<Value>, bool) {
    let mut failed = false;
    let outcomes = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let code = if failed {
                "skipped_after_failure"
            } else {
                match call(index) {
                    Ok(()) => "completed",
                    Err(error) => {
                        failed = true;
                        error.code()
                    }
                }
            };
            json!({"index":index,"root":row.root,"code":code})
        })
        .collect();
    (outcomes, failed)
}
fn run_with(args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let mut options = BTreeMap::new();
    let mut above_25 = false;
    while let Some(arg) = args.next() {
        if arg == "--allow-spend-above-25-usd" {
            above_25 = true;
            continue;
        }
        if ![
            "--requests",
            "--roots",
            "--max-spend-usd",
            "--out",
            "--user-policy",
        ]
        .contains(&arg.as_str())
            || options.contains_key(&arg)
        {
            return Err("invalid smoke arguments".into());
        }
        options.insert(arg, args.next().ok_or("missing argument value")?);
    }
    let required = |name: &str| options.get(name).ok_or("required smoke argument missing");
    let cap = allowance(required("--max-spend-usd")?, above_25)?;
    let count: usize = required("--roots")?.parse()?;
    if count == 0 || count > 10 {
        return Err("smoke requires 1 through 10 roots".into());
    }
    let path = PathBuf::from(required("--requests")?);
    let rows: Vec<Row> = assist::decode(&assist::read_bytes(&path, 32 * 1024 * 1024)?)?;
    validate_rows(&rows, count, cap)?;
    let user = UserConfig::load(&PathBuf::from(required("--user-policy")?))?;
    let mut roots = RootPolicy::new(
        &user
            .roots
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        None,
        false,
        &[],
    )?;
    user.apply(&mut roots)?;
    let source = std::fs::canonicalize(&path)?.to_string_lossy().into_owned();
    user.authorize(std::slice::from_ref(&source), &roots)?;
    let mut prepared = Vec::new();
    for row in &rows {
        let bytes = serde_json::to_vec(&row.payload)?;
        let key = CacheKey {
            evidence_hash: Digest::of_bytes(&std::fs::read(&path)?),
            payload_hash: Digest::of_bytes(&bytes),
            prompt_hash: Digest::of_bytes(&bytes),
            encoder_version: ENCODER.into(),
            provider: "openrouter".into(),
            model: row.model.clone(),
            revision: row.revision.clone(),
            settings: row.payload.clone(),
            api_config_hash: assist::digest(&user)?,
            order: "single".into(),
        };
        key.validate()?;
        openrouter::validate_request(&bytes, &row.model)?;
        prepared.push((key, bytes));
    }
    let out = PathBuf::from(required("--out")?);
    std::fs::create_dir(&out)?;
    let ledger = Ledger::new(&out.join("ledger"), true);
    let auth = Authorization {
        enabled: true,
        scopes: vec![Scope {
            id: "smoke".into(),
            caps: Caps {
                total: count as u64,
                providers: BTreeMap::from([("openrouter".into(), count as u64)]),
            },
        }],
    };
    let keys = Keys::new(Some(Keys::default_dir()));
    let network = Network;
    let transport = Transport {
        user: &user,
        roots: &roots,
        authorization: &auth,
        ledger: &ledger,
        keys: &keys,
        http: &network,
    };
    let executor = Executor {
        transport: &transport,
        ledger: &ledger,
        money_scopes: vec![MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: cap,
        }],
        sources: vec![source],
        deadline: Instant::now() + Duration::from_secs(300),
    };
    let (mut outcomes, mut failed) = root_outcomes(&rows, |index| {
        let (key, payload) = &prepared[index];
        match executor.call(key, payload) {
            Ok(completed) => {
                // Response has passed dispatch-secret reflection checks. No model qualification implied.
                std::fs::write(
                    out.join(format!("response-{index}.json")),
                    &completed.response,
                )
                .map_err(|_| assist::Error::Storage)?;
                std::fs::write(
                    out.join(format!("receipt-{index}.json")),
                    serde_json::to_vec(&completed.provenance)
                        .map_err(|_| assist::Error::Storage)?,
                )
                .map_err(|_| assist::Error::Storage)?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    });
    let reconciliation = openrouter::reconcile(&transport, Duration::from_secs(30));
    // OpenRouter has one money reservation per call, no auxiliary token counts
    // or retries. A fresh output ledger and stop-on-error keep this root order.
    let (receipts, receipt_code) = match ledger.money_receipts() {
        Ok(receipts) => (receipts, None),
        Err(_) => {
            failed = true;
            (Vec::new(), Some("assist_storage_unavailable"))
        }
    };
    for (index, receipt) in receipts.iter().enumerate() {
        outcomes[index]["execution_id"] = json!(receipt.id);
        outcomes[index]["response_identity"] = receipt.usage["response_identity"].clone();
        if outcomes[index]["code"] != "completed" {
            let bytes = serde_json::to_vec(receipt).map_err(|_| "smoke_receipt_encoding_failed")?;
            if std::fs::write(out.join(format!("receipt-{index}.json")), bytes).is_err() {
                outcomes[index]["artifact_code"] = json!("assist_storage_unavailable");
                failed = true;
            }
        }
    }
    std::fs::write(
        out.join("smoke.json"),
        serde_json::to_vec_pretty(
            &json!({"roots":count,"root_outcomes":outcomes,"receipt_code":receipt_code,"allowance_nano_usd":cap,"dispatch_failed":failed,"reconciliation":reconciliation.as_ref().err(),"qualified":false}),
        )?,
    )?;
    if failed || reconciliation.is_err() {
        return Err("OpenRouter smoke failed; inspect sanitized campaign receipts".into());
    }
    Ok(())
}
fn main() {
    if run_with(std::env::args().skip(1)).is_err() {
        eprintln!("OpenRouter smoke refused or failed; inspect campaign receipts when created");
        std::process::exit(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn g12_smoke_records_static_root_codes_and_stops_after_first_failure() {
        let rows: Vec<_> = (0..3)
            .map(|i| Row {
                root: format!("fixture-{i}"),
                model: "fixture/model".into(),
                revision: "fixture".into(),
                payload: Value::Null,
            })
            .collect();
        for error in [
            assist::Error::Invalid("provider revision drift quarantined"),
            assist::Error::Policy("deadline limit"),
            assist::Error::Storage,
            assist::Error::Provider,
        ] {
            let expected = error.code();
            let mut error = Some(error);
            let mut calls = 0;
            let (outcomes, failed) = root_outcomes(&rows, |index| {
                calls += 1;
                if index == 0 {
                    Ok(())
                } else {
                    Err(error.take().unwrap())
                }
            });
            assert!(failed);
            assert_eq!(calls, 2);
            let encoded = serde_json::to_vec(&json!({"root_outcomes":outcomes})).unwrap();
            let smoke: Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(smoke["root_outcomes"][0]["code"], "completed");
            assert_eq!(smoke["root_outcomes"][1]["code"], expected);
            assert_eq!(smoke["root_outcomes"][2]["code"], "skipped_after_failure");
            assert_eq!(smoke["root_outcomes"][1]["root"], "fixture-1");
        }
        let (outcomes, failed) = root_outcomes(&rows, |_| Ok(()));
        assert!(!failed);
        assert!(outcomes.iter().all(|o| o["code"] == "completed"));
    }
    #[test]
    fn g12_smoke_total_reservations_must_fit_before_policy_or_dispatch() {
        let temp = tempfile::tempdir().unwrap();
        let model = "google/gemini-3.8-flash";
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let payload: Value = assist::decode(
            &openrouter::request(&serde_json::to_vec(&source).unwrap(), model).unwrap(),
        )
        .unwrap();
        let rows: Vec<_> = (0..10).map(|i| json!({"root":format!("root-{i}"),"model":model,"revision":"fixture","payload":payload})).collect();
        let requests = temp.path().join("requests.json");
        std::fs::write(&requests, serde_json::to_vec(&rows).unwrap()).unwrap();
        let out = temp.path().join("out");
        let args = vec![
            "--requests".into(),
            requests.to_string_lossy().into_owned(),
            "--roots".into(),
            "10".into(),
            "--max-spend-usd".into(),
            "0.1".into(),
            "--out".into(),
            out.to_string_lossy().into_owned(),
            "--user-policy".into(),
            temp.path()
                .join("absent-user.toml")
                .to_string_lossy()
                .into_owned(),
        ];
        // Each root fits $0.10, but all ten worst-case reservations do not.
        assert_eq!(
            run_with(args).unwrap_err().to_string(),
            "smoke_reservations_exceed_cap"
        );
        assert!(!out.exists());
    }
    #[test]
    fn smoke_admission_refuses_unpriced_models_oversized_inputs_and_exact_cap_shortfall() {
        let model = "google/gemini-3.8-flash";
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let payload: Value = assist::decode(
            &openrouter::request(&serde_json::to_vec(&source).unwrap(), model).unwrap(),
        )
        .unwrap();
        let mut rows = vec![Row {
            root: "fixture".into(),
            model: model.into(),
            revision: "fixture".into(),
            payload,
        }];
        let cost = openrouter::admission(&serde_json::to_vec(&rows[0].payload).unwrap(), model)
            .unwrap()
            .reservation;
        assert!(validate_rows(&rows, 1, cost).is_ok());
        assert!(validate_rows(&rows, 1, cost - 1).is_err());
        rows[0].model = "unpriced/model".into();
        rows[0].payload["model"] = json!("unpriced/model");
        assert!(validate_rows(&rows, 1, u64::MAX).is_err());
        rows[0].model = model.into();
        rows[0].payload["model"] = json!(model);
        rows[0].payload["messages"][1]["content"][0]["text"] = json!("x".repeat(16000));
        assert!(validate_rows(&rows, 1, u64::MAX).is_err());
    }
    #[test]
    fn caps_above_25_require_separate_flag_without_raising_campaign_parent() {
        assert_eq!(allowance("1", false), Ok(1_000_000_000));
        assert_eq!(allowance("25", false), Ok(25_000_000_000));
        assert!(allowance("25.000000001", false).is_err());
        assert!(allowance("25.0000000001", false).is_err());
        assert_eq!(allowance("30", true), Ok(30_000_000_000));
        assert!(allowance("30.000000001", true).is_err());
        assert!(allowance("NaN", true).is_err());
    }
}
