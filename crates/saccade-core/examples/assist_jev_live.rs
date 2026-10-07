//! Explicit bounded Jev development runner. Default mode is offline preflight.
use saccade_core::{
    assist::{
        self,
        execution::{CacheKey, ENCODER, Executor},
        jev,
        schema::JEV,
    },
    budget_ledger::{Caps, Ledger, MoneyScope, Scope},
    evidence::canonical::Digest,
    judge_provider::{
        Keys,
        transport::{Authorization, Network, Transport, UserConfig},
    },
    root_policy::RootPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Row {
    root: String,
    workload: String,
    arm: String,
    order: String,
    payload: Value,
}
fn main() {
    if let Err(error) = execute() {
        // No error bodies, keys, user-supplied paths or provider text in diagnostics.
        let code = diagnostic_code(error.as_ref());
        eprintln!("{code}");
        std::process::exit(1);
    }
}
fn diagnostic_code(error: &(dyn std::error::Error + 'static)) -> &'static str {
    error
        .downcast_ref::<assist::Error>()
        .map(assist::Error::code)
        .unwrap_or("jev_campaign_refused_or_incomplete")
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut opts = BTreeMap::new();
    let (mut run, mut attested, mut continue_unknown) = (false, false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" if !run => run = true,
            "--continue-on-unknown-cost-at-reservation" if !continue_unknown => {
                continue_unknown = true
            }
            "--jev-prepaid-no-refill-attested" if !attested => attested = true,
            "--requests" | "--out" | "--resume" | "--user-policy" | "--max-spend-usd"
            | "--campaign" => {
                if opts
                    .insert(arg, args.next().ok_or("missing option")?)
                    .is_some()
                {
                    return Err("duplicate option".into());
                }
            }
            _ => return Err("unknown option".into()),
        }
    }
    let resume = opts.contains_key("--resume");
    if resume && (opts.contains_key("--out") || !run) {
        return Err("resume requires exclusive run output".into());
    }
    let path = PathBuf::from(opts.get("--requests").ok_or("requests required")?);
    let bytes = assist::read_bytes(&path, 8 * 1024 * 1024)?;
    let rows: Vec<Row> = assist::decode(&bytes)?;
    if rows.is_empty() || rows.len() > 1000 {
        return Err("request count".into());
    }
    let cap = match opts.get("--max-spend-usd") {
        Some(v) => {
            let cap = assist::openrouter::decimal_amount(v, false)
                .filter(|n| *n > 0 && *n <= jev::MAX_ALLOWANCE)
                .ok_or("allowance")?;
            if assist::openrouter::decimal_amount(v, true) != Some(cap) {
                return Err("allowance precision".into());
            }
            cap
        }
        None => jev::DEFAULT_ALLOWANCE,
    };
    let campaign = opts
        .get("--campaign")
        .cloned()
        .unwrap_or_else(|| Digest::of_bytes(&bytes).as_str()[7..].into());
    let allowance = jev::LocalAllowance {
        prepaid_no_refill_attested: attested,
        campaign,
        nano_usd: cap,
    };
    if run {
        allowance.validate()?;
    }
    jev::self_test()?;
    // Execute the actual frozen scorer self-test from this source tree, never an
    // arbitrary operator-supplied success receipt. It runs before keys or HTTP.
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/assist/jev_eval.py");
    let check = std::process::Command::new("python3")
        .arg(script)
        .arg("--self-test")
        .output()?;
    if !check.status.success() {
        return Err("scorer self-test failed".into());
    }
    let selftest: Value = assist::decode(&check.stdout)?;
    if selftest["status"] != "pass" {
        return Err("scorer gate".into());
    }
    let plan_path = path.parent().ok_or("plan parent")?.join("plan.json");
    let plan: Value = assist::decode(&assist::read_bytes(&plan_path, 256 * 1024)?)?;
    if plan["plan_hash"].as_str() != Some(&assist::digest(&rows)?.as_str()[7..])
        || (!resume && plan["source_hashes"] != selftest["source_hashes"])
        || plan["ceiling"] != jev::CEILING
        || plan["encoder"] != ENCODER
        || plan["split"] != "development"
    {
        return Err("frozen plan identity changed".into());
    }
    for (name, bytes) in compiled_sources() {
        if selftest["source_hashes"][name].as_str() != Some(&Digest::of_bytes(bytes).as_str()[7..])
        {
            return Err("compiled source identity changed".into());
        }
    }
    let mut prepared = Vec::new();
    let mut ids = BTreeSet::new();
    let mut reserved = 0u64;
    for row in &rows {
        if !["routing", "classification", "cause", "priority", "support"]
            .contains(&row.workload.as_str())
            || !["compact_jev", "enriched_jev"].contains(&row.arm.as_str())
            || !["forward", "reverse"].contains(&row.order.as_str())
            || !ids.insert((&row.root, &row.workload, &row.arm, &row.order))
        {
            return Err("request topology".into());
        }
        let payload = saccade_core::evidence::canonical::bytes(&row.payload)?;
        reserved = reserved
            .checked_add(jev::input_bound(&payload)? * 42)
            .ok_or("overflow")?;
        prepared.push(payload);
    }
    if reserved > cap {
        return Err("schedule exceeds allowance".into());
    }
    let summary = json!({"ceiling":jev::CEILING,"requests":rows.len(),"reservation_nano_usd":reserved,"allowance_nano_usd":cap,"selftest":selftest,"qualified":false,"run":run});
    if !run {
        println!("{}", summary);
        return Ok(());
    }
    let out = PathBuf::from(
        opts.get(if resume { "--resume" } else { "--out" })
            .ok_or("out required")?,
    );
    let user = UserConfig::load(&PathBuf::from(
        opts.get("--user-policy").ok_or("user policy required")?,
    ))?;
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
    if !resume {
        std::fs::create_dir(&out)?;
    }
    if !out.is_dir() || std::fs::symlink_metadata(&out)?.file_type().is_symlink() {
        return Err("invalid campaign directory".into());
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(out.join("runner.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)?;
    let ledger = Ledger::new(&out.join("ledger"), true);
    let binary = std::fs::read(std::env::current_exe()?)?;
    let identity = json!({"ceiling":jev::CEILING,"schema":"jev-local-campaign/1","requests_hash":Digest::of_bytes(&bytes),"allowance":cap,"campaign":allowance.campaign,"binary_hash":Digest::of_bytes(&binary),"selftest":selftest,"price_policy":jev::PRICE_ID});
    let mut prior = Vec::new();
    let mut settled = BTreeSet::new();
    if resume {
        let old = ledger
            .campaign_identity()?
            .ok_or("missing campaign identity")?;
        validate_resume_identity(&old, &identity, &plan)?;
        ledger.bind_campaign(old, true)?;
        let artifact: Value = assist::decode(&assist::read_bytes(
            &out.join("results.json"),
            32 * 1024 * 1024,
        )?)?;
        prior = artifact["rows"].as_array().ok_or("resume rows")?.clone();
        let bindings = validate_resume_rows(&rows, &prepared, &ledger, &prior)?;
        ledger.bind_legacy_jev_roots(&bindings)?;
        for (_, index, _) in &bindings {
            if continue_unknown {
                ledger.settle_continuation_root(*index)?;
            }
            let receipt = ledger
                .money_receipts()?
                .into_iter()
                .find(|r| r.usage["campaign_root_index"] == *index)
                .ok_or("receipt binding")?;
            if receipt.outcome == "settled_conservatively" {
                prior[*index]["code"] = json!("not_run_transport_failure");
                prior[*index]["response"] = Value::Null;
                prior[*index]["answer_valid"] = json!(false);
            } else if receipt.outcome != "completed" || receipt.actual_nano_usd.is_none() {
                return Err("resume requires settled unavailable roots".into());
            }
            if receipt.outcome == "completed" {
                let choices = rows[*index].payload["questions"]["q"]["criteria"]
                    .as_object()
                    .ok_or("criteria")?
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                prior[*index]["answer_valid"] =
                    json!(jev::choice(&prior[*index]["response"], &choices).is_ok());
                if prior[*index]["answer_valid"] != true
                    && receipt.usage["billing_basis"] != "returned_usage_times_pinned_tariff"
                {
                    return Err("resume requires known answer cost".into());
                }
                prior[*index]["answer"] = json!(
                    jev::normalised_answer(&prior[*index]["response"], &choices)
                        .ok()
                        .map(|(_, metadata)| metadata)
                );
            }
            prior[*index]["execution_id"] = json!(receipt.id);
            prior[*index]["transport_failure"] = receipt.usage["transport_failure"].clone();
            prior[*index]["actual_nano_usd"] = json!(receipt.actual_nano_usd);
            prior[*index]["money_outcome"] = json!(receipt.outcome);
            settled.insert(*index);
        }
        // Record the new binary separately; preserve the original frozen campaign.
        assist::write(
            &out.join("resume-preflight.json"),
            &json!({"identity":identity,"answer_failure_policy":{"max_consecutive":5,"max_percent":50,"min_sample":20},"transport_failure_policy":continue_unknown.then(assist::continuation::History::policy),"qualified":false}),
        )?;
    } else {
        ledger.bind_campaign(identity, false)?;
    }
    if !resume {
        assist::write(&out.join("preflight.json"), &summary)?;
    }
    let keys = Keys::new(Some(Keys::default_dir()));
    let network = Network;
    let auth = Authorization {
        enabled: true,
        scopes: vec![Scope {
            id: "jev-campaign".into(),
            caps: Caps {
                total: rows.len() as u64,
                providers: BTreeMap::from([("jev".into(), rows.len() as u64)]),
            },
        }],
    };
    let transport = Transport {
        user: &user,
        roots: &roots,
        authorization: &auth,
        ledger: &ledger,
        keys: &keys,
        http: &network,
    };
    let deadline = Instant::now() + Duration::from_secs(if continue_unknown { 21600 } else { 300 });
    let mut results = Vec::new();
    let mut failed = false;
    let mut stop_code: Option<&'static str> = None;
    let mut history = assist::continuation::History::default();
    let mut answers = AnswerHistory::default();
    for (index, (row, payload)) in rows.iter().zip(&prepared).enumerate() {
        if settled.contains(&index) {
            let code = if prior[index]["code"] == "not_run_transport_failure" {
                "not_run_transport_failure"
            } else if prior[index]["answer_valid"] == true {
                "completed"
            } else {
                "invalid_answer"
            };
            results.push(prior[index].clone());
            if let Some(valve) = history.observe(code).or_else(|| answers.observe(code)) {
                results[index]["safety_valve"] = json!(valve);
                stop_code = Some(valve);
                failed = true;
                break;
            }
            continue;
        }
        if Instant::now() >= deadline {
            stop_code = Some("campaign_deadline");
            failed = true;
            break;
        }
        ledger.begin_campaign_root(index, Digest::of_bytes(payload))?;
        let key = CacheKey {
            evidence_hash: Digest::of_bytes(&bytes),
            payload_hash: Digest::of_bytes(payload),
            prompt_hash: assist::digest(&row.payload["questions"])?,
            encoder_version: ENCODER.into(),
            provider: "jev".into(),
            model: JEV.into(),
            revision: JEV.into(),
            settings: json!({"workload":row.workload,"arm":row.arm,"order":row.order}),
            api_config_hash: assist::digest(&user)?,
            order: "single".into(),
        };
        let executor = Executor {
            transport: &transport,
            ledger: &ledger,
            money_scopes: vec![MoneyScope {
                id: "jev-development".into(),
                cap_nano_usd: cap,
            }],
            sources: vec![source.clone()],
            deadline: deadline.min(Instant::now() + Duration::from_secs(300)),
        };
        match executor.call_jev(&key, payload, &allowance) {
            Ok(done) => {
                let response: Value = assist::decode(&done.response)?;
                let choices: Vec<&str> = row.payload["questions"]["q"]["criteria"]
                    .as_object()
                    .ok_or("criteria")?
                    .keys()
                    .map(String::as_str)
                    .collect();
                let answer_valid = jev::choice(&response, &choices).is_ok();
                if !answer_valid && response.get("usage").is_none() {
                    stop_code = Some("unknown_answer_cost");
                    failed = true;
                }
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"answer_valid":answer_valid,"code":if answer_valid {"completed"} else {"invalid_answer"},"answer":jev::normalised_answer(&response, &choices).ok().map(|(_, metadata)| metadata),"response":response,"response_raw":String::from_utf8(done.response)?,"provenance":done.provenance,"ceiling":jev::CEILING}));
            }
            Err(assist::Error::Provider)
                if continue_unknown && ledger.settle_continuation_root(index)? =>
            {
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"response":null,"answer_valid":false,"ceiling":jev::CEILING,"code":"not_run_transport_failure"}));
            }
            Err(error) => {
                stop_code = Some(error.code());
                failed = true;
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"response":null,"ceiling":jev::CEILING,"code":"incomplete"}));
            }
        }
        if let Some(receipt) = ledger
            .money_receipts()?
            .iter()
            .rev()
            .find(|r| r.usage["campaign_root_index"] == index)
        {
            results[index]["execution_id"] = json!(receipt.id);
            results[index]["transport_failure"] = receipt.usage["transport_failure"].clone();
            results[index]["actual_nano_usd"] = json!(receipt.actual_nano_usd);
            results[index]["money_outcome"] = json!(receipt.outcome);
        }
        let code = if failed {
            "incomplete"
        } else if results[index]["code"] == "not_run_transport_failure" {
            "not_run_transport_failure"
        } else if results[index]["answer_valid"] == true {
            "completed"
        } else {
            "invalid_answer"
        };
        if let Some(valve) = history.observe(code).or_else(|| answers.observe(code)) {
            results[index]["safety_valve"] = json!(valve);
            stop_code = Some(valve);
            failed = true;
        }
        assist::write(
            &out.join("results.json"),
            &json!({"ceiling":jev::CEILING,"rows":results,"money_receipts":ledger.money_receipts()?,"qualified":false,"incomplete":failed,"stop_code":stop_code,"answer_failure_policy":{"max_consecutive":5,"max_percent":50,"min_sample":20},"transport_failure_policy":continue_unknown.then(assist::continuation::History::policy)}),
        )?;
        if failed {
            break;
        } // Ambiguous billing, identity drift or timeout stops the campaign.
    }
    if failed {
        for (index, row) in rows.iter().enumerate().skip(results.len()) {
            if settled.contains(&index) {
                results.push(prior[index].clone());
            } else {
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"response":null,"ceiling":jev::CEILING,"code":"not_run"}));
            }
        }
        assist::write(
            &out.join("results.json"),
            &json!({"ceiling":jev::CEILING,"rows":results,"money_receipts":ledger.money_receipts()?,"qualified":false,"incomplete":true,"stop_code":stop_code,"answer_failure_policy":{"max_consecutive":5,"max_percent":50,"min_sample":20},"transport_failure_policy":continue_unknown.then(assist::continuation::History::policy)}),
        )?;
        return Err(assist::Error::Policy(
            stop_code.unwrap_or("jev_campaign_refused_or_incomplete"),
        )
        .into());
    }
    assist::write(
        &out.join("results.json"),
        &json!({"ceiling":jev::CEILING,"rows":results,"money_receipts":ledger.money_receipts()?,"qualified":false,"incomplete":false,"stop_code":null,"answer_failure_policy":{"max_consecutive":5,"max_percent":50,"min_sample":20},"transport_failure_policy":continue_unknown.then(assist::continuation::History::policy)}),
    )?;
    println!(
        "{}",
        json!({"ceiling":jev::CEILING,"completed":results.iter().filter(|r| r["answer_valid"] == true).count(),"qualified":false})
    );
    Ok(())
}

#[derive(Default)]
struct AnswerHistory {
    samples: usize,
    invalid: usize,
    consecutive: usize,
}
impl AnswerHistory {
    fn observe(&mut self, code: &str) -> Option<&'static str> {
        if !["completed", "invalid_answer"].contains(&code) {
            return None;
        }
        self.samples += 1;
        if code == "invalid_answer" {
            self.invalid += 1;
            self.consecutive += 1;
        } else {
            self.consecutive = 0;
        }
        if self.consecutive > 5 {
            Some("consecutive_invalid_answers")
        } else if self.samples >= 20 && self.invalid * 100 > self.samples * 50 {
            Some("invalid_answer_rate")
        } else {
            None
        }
    }
}

// This authorised continuation upgrade includes rounded-probability parser/scorer
// sources. Schema, tariff, evidence and campaign settings remain identical.
fn validate_resume_identity(
    old: &Value,
    new: &Value,
    plan: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut expected = new.clone();
    expected["binary_hash"] = old["binary_hash"].clone();
    expected["selftest"] = old["selftest"].clone();
    if *old != expected || plan["source_hashes"] != old["selftest"]["source_hashes"] {
        return Err("resume campaign identity changed".into());
    }
    let allowed = [
        "scripts/assist/jev_eval.py",
        "scripts/assist/test_jev.py",
        "crates/saccade-core/src/assist/jev.rs",
        "crates/saccade-core/examples/assist_jev_live.rs",
        "crates/saccade-core/src/assist/execution.rs",
        "crates/saccade-core/src/judge_provider/transport.rs",
        "crates/saccade-core/src/assist/money.rs",
    ];
    for (name, hash) in old["selftest"]["source_hashes"]
        .as_object()
        .ok_or("resume source identity")?
    {
        if !allowed.contains(&name.as_str()) && new["selftest"]["source_hashes"][name] != *hash {
            return Err("resume frozen source changed".into());
        }
    }
    Ok(())
}
type RootBinding = (String, usize, Digest);
fn validate_resume_rows(
    rows: &[Row],
    prepared: &[Vec<u8>],
    ledger: &Ledger,
    prior: &[Value],
) -> Result<Vec<RootBinding>, Box<dyn std::error::Error>> {
    if prior.len() != rows.len() {
        return Err("resume topology changed".into());
    }
    for (i, (row, old)) in rows.iter().zip(prior).enumerate() {
        if old["index"] != i
            || old["root"] != row.root
            || old["workload"] != row.workload
            || old["arm"] != row.arm
            || old["order"] != row.order
            || old["payload"] != row.payload
        {
            return Err("resume topology changed".into());
        }
    }
    let receipts = ledger.money_receipts()?;
    let mut bindings = Vec::new();
    let mut seen = BTreeSet::new();
    for (ordinal, receipt) in receipts.iter().enumerate() {
        let index = receipt.usage["campaign_root_index"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(ordinal);
        if index >= rows.len()
            || !seen.insert(index)
            || receipt.request_hash != Digest::of_bytes(&prepared[index])
        {
            return Err("resume receipt identity changed".into());
        }
        let old = &prior[index];
        if receipt.outcome == "completed" {
            let raw = old["response_raw"]
                .as_str()
                .ok_or("resume response missing")?;
            if old["provenance"]["execution_id"] != receipt.id
                || old["provenance"]["request_hash"] != serde_json::to_value(&receipt.request_hash)?
                || old["provenance"]["response_hash"]
                    != serde_json::to_value(Digest::of_bytes(raw.as_bytes()))?
                || receipt.usage["response_hash"] != old["provenance"]["response_hash"]
                || assist::decode::<Value>(raw.as_bytes())? != old["response"]
            {
                return Err("resume response identity changed".into());
            }
        } else if !matches!(
            receipt.outcome.as_str(),
            "incomplete" | "reserved" | "settled_conservatively"
        ) || !old["response"].is_null()
        {
            return Err("resume receipt outcome changed".into());
        }
        bindings.push((receipt.id.clone(), index, receipt.request_hash.clone()));
    }
    for (index, old) in prior.iter().enumerate() {
        if !seen.contains(&index) && (old["code"] != "not_run" || !old["response"].is_null()) {
            return Err("resume missing receipt".into());
        }
    }
    Ok(bindings)
}

fn compiled_sources() -> [(&'static str, &'static [u8]); 12] {
    [
        (
            "crates/saccade-core/src/assist/schema.rs",
            include_bytes!("../src/assist/schema.rs"),
        ),
        (
            "crates/saccade-core/src/budget_ledger.rs",
            include_bytes!("../src/budget_ledger.rs"),
        ),
        (
            "crates/saccade-core/src/assist/price.rs",
            include_bytes!("../src/assist/price.rs"),
        ),
        (
            "scripts/assist/jev_eval.py",
            include_bytes!("../../../scripts/assist/jev_eval.py"),
        ),
        (
            "scripts/assist/test_jev.py",
            include_bytes!("../../../scripts/assist/test_jev.py"),
        ),
        (
            "crates/saccade-core/src/assist/jev.rs",
            include_bytes!("../src/assist/jev.rs"),
        ),
        (
            "crates/saccade-core/src/assist/execution.rs",
            include_bytes!("../src/assist/execution.rs"),
        ),
        (
            "crates/saccade-core/src/assist/workflow.rs",
            include_bytes!("../src/assist/workflow.rs"),
        ),
        (
            "crates/saccade-core/src/assist/routing.rs",
            include_bytes!("../src/assist/routing.rs"),
        ),
        (
            "crates/saccade-core/examples/assist_jev_live.rs",
            include_bytes!("assist_jev_live.rs"),
        ),
        (
            "crates/saccade-core/src/judge_provider/transport.rs",
            include_bytes!("../src/judge_provider/transport.rs"),
        ),
        (
            "crates/saccade-core/src/assist/money.rs",
            include_bytes!("../src/assist/money.rs"),
        ),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn answer_failures_continue_until_strict_g12_limits() {
        let mut h = AnswerHistory::default();
        for _ in 0..5 {
            assert_eq!(h.observe("invalid_answer"), None);
        }
        assert_eq!(
            h.observe("invalid_answer"),
            Some("consecutive_invalid_answers")
        );
        let mut h = AnswerHistory::default();
        for _ in 0..10 {
            assert_eq!(h.observe("invalid_answer"), None);
            assert_eq!(h.observe("completed"), None);
        }
        assert_eq!(h.observe("not_run"), None);
        assert_eq!(h.observe("invalid_answer"), Some("invalid_answer_rate"));
    }
    #[test]
    fn rounded_answers_revalidate_without_changing_raw_evidence() {
        for sum in [0.97, 0.98, 0.99, 1.0, 1.01, 1.02, 1.03] {
            let response = json!({"model":JEV,"answers":{"q":{"type":"choice","confidence":0.9,"choice":"vision","probabilities":{"vision":sum-0.1,"insufficient":0.1}}}});
            let original = response.clone();
            let valid = (0.98..=1.02).contains(&sum);
            assert_eq!(
                jev::choice(&response, &["vision", "insufficient"]).is_ok(),
                valid
            );
            if valid {
                let (_, metadata) =
                    jev::normalised_answer(&response, &["vision", "insufficient"]).unwrap();
                assert!((metadata["original_sum"].as_f64().unwrap() - sum).abs() < 1e-12);
            }
            assert_eq!(response, original);
        }
    }
    #[test]
    fn stable_stop_diagnostics_do_not_echo_untrusted_errors() {
        for code in [
            "invalid_answer",
            "campaign_deadline",
            "transport_failure_rate",
            "consecutive_transport_failures",
            "assist_provider_execution_incomplete",
        ] {
            assert_eq!(diagnostic_code(&assist::Error::Policy(code)), code);
        }
        let error = std::io::Error::other("untrusted provider body or path");
        assert_eq!(
            diagnostic_code(&error),
            "jev_campaign_refused_or_incomplete"
        );
    }
    #[test]
    #[ignore = "operator-supplied saved campaign; copies only, no provider or keys"]
    fn offline_saved_resume_fixture() {
        let requests = PathBuf::from(std::env::var_os("SACCADE_RESUME_REQUESTS").unwrap());
        let out = PathBuf::from(std::env::var_os("SACCADE_RESUME_OUTPUT").unwrap());
        let rows: Vec<Row> =
            assist::decode(&assist::read_bytes(&requests, 8 * 1024 * 1024).unwrap()).unwrap();
        let prior: Value = assist::decode(
            &assist::read_bytes(&out.join("results.json"), 32 * 1024 * 1024).unwrap(),
        )
        .unwrap();
        let prepared = rows
            .iter()
            .map(|r| saccade_core::evidence::canonical::bytes(&r.payload).unwrap())
            .collect::<Vec<_>>();
        let temp = tempfile::tempdir().unwrap();
        std::fs::copy(
            out.join("ledger/campaign.json"),
            temp.path().join("campaign.json"),
        )
        .unwrap();
        let ledger = Ledger::new(temp.path(), true);
        let old = ledger.campaign_identity().unwrap().unwrap();
        let plan: Value = assist::decode(
            &assist::read_bytes(&requests.parent().unwrap().join("plan.json"), 256 * 1024).unwrap(),
        )
        .unwrap();
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/assist/jev_eval.py");
        let result = std::process::Command::new("python3")
            .arg(script)
            .arg("--self-test")
            .output()
            .unwrap();
        assert!(result.status.success());
        let mut new = old.clone();
        new["binary_hash"] = json!("fixture-new-binary");
        new["selftest"] = assist::decode(&result.stdout).unwrap();
        validate_resume_identity(&old, &new, &plan).unwrap();
        let bindings =
            validate_resume_rows(&rows, &prepared, &ledger, prior["rows"].as_array().unwrap())
                .unwrap();
        ledger.bind_legacy_jev_roots(&bindings).unwrap();
        let before = ledger.money_receipts().unwrap();
        for (_, index, _) in &bindings {
            ledger.settle_continuation_root(*index).unwrap();
        }
        let after = ledger.money_receipts().unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(
            after
                .iter()
                .filter(|r| r.outcome == "settled_conservatively")
                .count(),
            1
        );
        assert!(after.iter().all(|r| r.actual_nano_usd.is_some()));
        assert_eq!(
            before.iter().filter(|r| r.outcome == "completed").count(),
            prior["rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| !r["response"].is_null())
                .count()
        );
        for (row, saved) in rows.iter().zip(prior["rows"].as_array().unwrap()) {
            if saved["response"].is_null() {
                continue;
            }
            let choices = row.payload["questions"]["q"]["criteria"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            assert!(jev::choice(&saved["response"], &choices).is_ok());
        }
    }
    #[test]
    fn resume_identity_preserves_tariff_scorer_and_schedule_across_runner_upgrade() {
        let old = json!({"schema":"jev-local-campaign/1","requests_hash":"requests","allowance":100,
            "binary_hash":"old","selftest":{"status":"pass","source_hashes":{"scorer":"same","crates/saccade-core/examples/assist_jev_live.rs":"old"}}});
        let mut new = old.clone();
        new["binary_hash"] = json!("new");
        new["selftest"]["source_hashes"]["crates/saccade-core/examples/assist_jev_live.rs"] =
            json!("new");
        let plan = json!({"source_hashes":old["selftest"]["source_hashes"]});
        validate_resume_identity(&old, &new, &plan).unwrap();
        new["allowance"] = json!(101);
        assert!(validate_resume_identity(&old, &new, &plan).is_err());
        new["allowance"] = json!(100);
        new["selftest"]["source_hashes"]["scorer"] = json!("changed");
        assert!(validate_resume_identity(&old, &new, &plan).is_err());
    }
    #[test]
    fn legacy_incomplete_resume_binds_charges_and_skips_once() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), true);
        ledger
            .bind_campaign(json!({"schema":"jev-local-campaign/1"}), false)
            .unwrap();
        let rows = vec![Row {
            root: "fixture".into(),
            workload: "routing".into(),
            arm: "compact_jev".into(),
            order: "forward".into(),
            payload: json!({"fixture":true}),
        }];
        let payload = saccade_core::evidence::canonical::bytes(&rows[0].payload).unwrap();
        ledger
            .reserve_money(
                &[MoneyScope {
                    id: "fixture".into(),
                    cap_nano_usd: 100,
                }],
                saccade_core::budget_ledger::MoneyReceipt {
                    id: "failed".into(),
                    request_hash: Digest::of_bytes(&payload),
                    scopes: vec!["fixture".into()],
                    reserved_nano_usd: 100,
                    actual_nano_usd: None,
                    outcome: "reserved".into(),
                    usage: json!({}),
                },
            )
            .unwrap();
        ledger
            .finish_money("failed", Some(0), json!({"not_dispatched":true}), false)
            .unwrap();
        // Historical class was null before this change.
        let mut prior = vec![
            json!({"index":0,"root":"fixture","workload":"routing","arm":"compact_jev","order":"forward","payload":rows[0].payload,"code":"incomplete","response":null}),
        ];
        let prepared = vec![payload];
        let bindings = validate_resume_rows(&rows, &prepared, &ledger, &prior).unwrap();
        ledger.bind_legacy_jev_roots(&bindings).unwrap();
        // Newly classified generic local errors fail closed, unlike pacing failures.
        assert!(!ledger.settle_continuation_root(0).unwrap());
        prior[0]["payload"] = json!({"changed":true});
        assert!(validate_resume_rows(&rows, &prepared, &ledger, &prior).is_err());
    }
}
