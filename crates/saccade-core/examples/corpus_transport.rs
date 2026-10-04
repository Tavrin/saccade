//! R12's single-attempt boundary. Every dispatch uses the R11 evaluation ledger.
use saccade_core::budget_ledger::{Caps, Ledger, Scope};
use saccade_core::evidence::canonical::Digest;
use saccade_core::judge_provider::Keys;
use saccade_core::judge_provider::transport::{
    Authorization, EgressSetting, Network, RootSetting, Transport, UserConfig,
};
use saccade_core::root_policy::RootPolicy;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dispatch {
    manifest: PathBuf,
    manifest_sha256: Digest,
    payload: PathBuf,
    payload_sha256: Digest,
    response: PathBuf,
    ledger: PathBuf,
    source_roots: Vec<String>,
    provider: String,
    model: String,
    batch_size: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 || args[1] != "--run" {
        return Err("usage: corpus_transport --run DISPATCH_JSON".into());
    }
    let input: Dispatch = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    if Digest::of_bytes(&std::fs::read(&input.manifest)?) != input.manifest_sha256 {
        return Err("frozen manifest changed".into());
    }
    let payload = std::fs::read(&input.payload)?;
    if Digest::of_bytes(&payload) != input.payload_sha256 {
        return Err("frozen payload changed".into());
    }
    // These startup permissions implement the owner's explicit §16 authorization.
    // No project/model/payload field supplies endpoint or credential authority.
    let paths: Vec<PathBuf> = input.source_roots.iter().map(PathBuf::from).collect();
    let mut roots = RootPolicy::new(&paths, None, false, &[])?;
    // Pacing is user-level only (~/.config/saccade/user.toml); absent entries use
    // the conservative built-in defaults. Nothing else is taken from that file.
    let pacing = UserConfig::load(&Keys::default_dir().join("user.toml"))?.pacing;
    let user = UserConfig {
        pacing,
        roots: paths
            .iter()
            .enumerate()
            .map(|(i, path)| RootSetting {
                id: format!("r12-source-{i}"),
                path: path.clone(),
                egress: EgressSetting::Allow,
            })
            .collect(),
        ..Default::default()
    };
    user.apply(&mut roots)?;
    let expanded = input
        .manifest
        .file_name()
        .is_some_and(|n| n == "moss-pilot-expanded.toml");
    if expanded
        && serde_json::to_value(input.manifest_sha256)?
            != json!("sha256:73fa53df42348eff4ac8821b38f18fa029222bcbc4ff43ff8c1533539dbeca86")
    {
        return Err("expanded manifest seal mismatch".into());
    }
    let authorization = Authorization {
        enabled: true,
        scopes: vec![Scope {
            id: if expanded {
                "r12q-expanded/1"
            } else {
                "r12-constructed-truth-run/1"
            }
            .into(),
            caps: if expanded {
                Caps {
                    total: 1123,
                    providers: BTreeMap::from([("jev".into(), 788), ("gemini".into(), 335)]),
                }
            } else {
                Caps {
                    total: 650,
                    providers: BTreeMap::from([("jev".into(), 400), ("gemini".into(), 250)]),
                }
            },
        }],
    };
    let ledger = Ledger::new(&input.ledger, true);
    let keys = Keys::new(None);
    let http = Network;
    let transport = Transport {
        user: &user,
        roots: &roots,
        authorization: &authorization,
        ledger: &ledger,
        keys: &keys,
        http: &http,
    };
    let start = Instant::now();
    let before = ledger.attempts()?.len();
    // One requested model per dispatch: 429/5xx retry the same model with
    // jittered exponential backoff (honouring provider waits) and user pacing;
    // the scheduler alone decides any fallback to another chain model.
    let private = input
        .response
        .parent()
        .ok_or("invalid response path")?
        .to_owned();
    let mut failures = Vec::new();
    let result = transport.execute_observed(
        &input.provider,
        std::slice::from_ref(&input.model),
        |_| Ok(payload.clone()),
        &input.source_roots,
        input.batch_size,
        Duration::from_secs(240),
        false,
        &mut |failed| failures.push(failed.clone()),
    );
    let mut failed_attempts = Vec::new();
    for (i, failed) in failures.iter().enumerate() {
        // Provider error bodies may echo request content: private folder only.
        let body_file = (!failed.body.is_empty()).then(|| private.join(format!("error-{i}.body")));
        if let Some(file) = &body_file {
            std::fs::write(file, &failed.body)?;
        }
        failed_attempts.push(
            json!({"status":failed.status,"retry_after_secs":failed.retry_after_secs,
            "reservation":failed.reservation,"error_body":body_file,
            "error_body_sha256":body_file.as_ref().map(|_| Digest::of_bytes(&failed.body))}),
        );
    }
    let output = match result {
        Ok(exchange) => {
            std::fs::write(&input.response, &exchange.body)?;
            json!({"outcome":"received", "response_sha256":Digest::of_bytes(&exchange.body),
                "model":exchange.model,"attempt_id":exchange.attempts.last(),
                "failed_attempts":failed_attempts})
        }
        Err(error) => {
            let rejected = failures.last().is_some_and(|f| {
                f.reservation.is_some()
                    && saccade_core::judge_provider::transport::request_rejected(f.status)
            });
            json!({"outcome":"unavailable","failure":if rejected {json!("rejected")} else {json!(error.class)},
                "reason":error.message,"retry_after_secs":error.retry_after_secs,"model":input.model,
                "failed_attempts":failed_attempts})
        }
    };
    let mut output = output;
    output["attempts"] = json!(ledger.attempts()?.len() - before);
    output["latency_ms"] = json!(start.elapsed().as_millis());
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
