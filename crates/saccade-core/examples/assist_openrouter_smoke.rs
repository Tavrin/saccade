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
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
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
    let mut failed = false;
    let writes = (|| -> Result<(), Box<dyn std::error::Error>> {
        for (index, (key, payload)) in prepared.iter().enumerate() {
            match executor.call(key, payload) {
                Ok(completed) => {
                    // Response has passed dispatch-secret reflection checks. No model qualification implied.
                    std::fs::write(
                        out.join(format!("response-{index}.json")),
                        &completed.response,
                    )?;
                    std::fs::write(
                        out.join(format!("receipt-{index}.json")),
                        serde_json::to_vec(&completed.provenance)?,
                    )?;
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        Ok(())
    })();
    failed |= writes.is_err();
    let reconciliation = openrouter::reconcile(&transport, Duration::from_secs(30));
    std::fs::write(
        out.join("smoke.json"),
        serde_json::to_vec_pretty(
            &json!({"roots":count,"allowance_nano_usd":cap,"dispatch_failed":failed,"reconciliation":reconciliation.as_ref().err(),"qualified":false}),
        )?,
    )?;
    if failed || reconciliation.is_err() {
        return Err("OpenRouter smoke failed; inspect sanitized campaign receipts".into());
    }
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!("OpenRouter smoke refused or failed; inspect campaign receipts when created");
        std::process::exit(4);
    }
}

#[cfg(test)]
mod tests {
    use super::allowance;
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
