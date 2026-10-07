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
    if execute().is_err() {
        // No error bodies, keys, user-supplied paths or provider text in diagnostics.
        eprintln!("jev_campaign_refused_or_incomplete");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut opts = BTreeMap::new();
    let (mut run, mut attested) = (false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" if !run => run = true,
            "--jev-prepaid-no-refill-attested" if !attested => attested = true,
            "--requests" | "--out" | "--user-policy" | "--max-spend-usd" | "--campaign" => {
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
        || plan["source_hashes"] != selftest["source_hashes"]
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
    let out = PathBuf::from(opts.get("--out").ok_or("out required")?);
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
    std::fs::create_dir(&out)?; // Fresh campaign only. No implicit replay, resume or retry.
    let ledger = Ledger::new(&out.join("ledger"), true);
    let binary = std::fs::read(std::env::current_exe()?)?;
    ledger.bind_campaign(json!({"ceiling":jev::CEILING,"schema":"jev-local-campaign/1","requests_hash":Digest::of_bytes(&bytes),"allowance":cap,"campaign":allowance.campaign,"binary_hash":Digest::of_bytes(&binary),"selftest":selftest,"price_policy":jev::PRICE_ID}), false)?;
    assist::write(&out.join("preflight.json"), &summary)?;
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
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut results = Vec::new();
    let mut failed = false;
    for (index, (row, payload)) in rows.iter().zip(&prepared).enumerate() {
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
            deadline,
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
                if !answer_valid {
                    failed = true;
                }
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"answer_valid":answer_valid,"response":response,"response_raw":String::from_utf8(done.response)?,"provenance":done.provenance,"ceiling":jev::CEILING}));
            }
            Err(_) => {
                failed = true;
                results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"response":null,"ceiling":jev::CEILING,"code":"incomplete"}));
            }
        }
        assist::write(
            &out.join("results.json"),
            &json!({"ceiling":jev::CEILING,"rows":results,"money_receipts":ledger.money_receipts()?,"qualified":false,"incomplete":failed}),
        )?;
        if failed {
            break;
        } // Ambiguous billing, identity drift or timeout stops the campaign.
    }
    if failed {
        for (index, row) in rows.iter().enumerate().skip(results.len()) {
            results.push(json!({"index":index,"root":row.root,"workload":row.workload,"arm":row.arm,"order":row.order,"payload":row.payload,"response":null,"ceiling":jev::CEILING,"code":"not_run"}));
        }
        assist::write(
            &out.join("results.json"),
            &json!({"ceiling":jev::CEILING,"rows":results,"money_receipts":ledger.money_receipts()?,"qualified":false,"incomplete":true}),
        )?;
        return Err("incomplete campaign".into());
    }
    println!(
        "{}",
        json!({"ceiling":jev::CEILING,"completed":results.len(),"qualified":false})
    );
    Ok(())
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
