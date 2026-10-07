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
    path::{Path, PathBuf},
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
// Always validate every request offline. By default the whole schedule must
// fit before credentials/accounting; budget-bounded mode delegates admission
// to the unchanged per-call reservation and external ceiling checks.
fn validate_rows(
    rows: &[Row],
    count: usize,
    cap: u64,
    budget_bounded: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
    if !budget_bounded && total > cap {
        return Err("smoke_reservations_exceed_cap".into());
    }
    Ok(())
}
const CALL_LIMIT: Duration = Duration::from_secs(300);
fn campaign_duration(stage2: bool) -> Duration {
    Duration::from_secs(if stage2 { 21600 } else { 300 })
}
// Only the post-settlement answer validator can produce InvalidAnswer.
#[derive(Debug, PartialEq)]
enum CallOutcome {
    Completed,
    InvalidAnswer(&'static str),
}
#[derive(Clone, Copy, serde::Serialize)]
struct AnswerLimits {
    max_consecutive: usize,
    max_percent: u8,
    min_sample: usize,
}
impl Default for AnswerLimits {
    fn default() -> Self {
        Self {
            max_consecutive: 5,
            max_percent: 50,
            min_sample: 20,
        }
    }
}
// Clock injection keeps deadline and safety-valve fixtures free of real waits.
fn root_outcomes(
    rows: &[Row],
    campaign_deadline: Instant,
    budget_bounded: bool,
    limits: Option<AnswerLimits>,
    mut now: impl FnMut() -> Instant,
    mut call: impl FnMut(usize, Instant) -> assist::Result<CallOutcome>,
) -> (Vec<Value>, bool) {
    let mut failed = false;
    let mut stop = None;
    let (mut samples, mut invalid, mut consecutive) = (0usize, 0usize, 0usize);
    let outcomes = rows.iter().enumerate().map(|(index, row)| {
        let mut reason = None;
        let mut valve = None;
        let code = if let Some(code) = stop { code } else {
            let started = now();
            if started >= campaign_deadline {
                stop = Some("not_run_deadline");
                "not_run_deadline"
            } else {
                let deadline = campaign_deadline.min(started + CALL_LIMIT);
                match call(index, deadline) {
                    Ok(CallOutcome::Completed) => {
                        samples += 1;
                        consecutive = 0;
                        "completed"
                    }
                    Ok(CallOutcome::InvalidAnswer(code)) => {
                        samples += 1;
                        invalid += 1;
                        consecutive += 1;
                        reason = Some(code);
                        "invalid_answer"
                    }
                    Err(assist::Error::Policy("money budget exhausted")) if budget_bounded => {
                        stop = Some("not_run_budget");
                        "not_run_budget"
                    }
                    Err(error) => {
                        failed = true;
                        stop = Some(if now() >= campaign_deadline { "not_run_deadline" } else { "skipped_after_failure" });
                        error.code()
                    }
                }
            }
        };
        if matches!(code, "completed" | "invalid_answer") && let Some(limits) = limits {
                valve = if consecutive > limits.max_consecutive {
                    Some("consecutive_invalid_answers")
                } else if samples >= limits.min_sample && invalid * 100 > samples * usize::from(limits.max_percent) {
                    Some("invalid_answer_rate")
                } else { None };
                if valve.is_some() {
                    failed = true;
                    stop = Some("not_run_answer_safety_valve");
                }
        }
        let class = match code {
            "completed" => "valid_answer",
            "invalid_answer" => "answer_failure",
            "not_run_deadline" | "not_run_budget" | "not_run_answer_safety_valve" | "skipped_after_failure" => "not_run",
            _ => "campaign_failure",
        };
        json!({"index":index,"root":row.root,"code":code,"failure_class":class,"answer_reason":reason,"safety_valve":valve})
    }).collect();
    (outcomes, failed)
}
// Refresh exported receipts from the authoritative ledger without replacing provenance.
fn export_reconciliation(
    out: &Path,
    ledger: &Ledger,
    smoke: &mut Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipts = ledger.money_receipts()?;
    let outcomes = smoke["root_outcomes"]
        .as_array_mut()
        .ok_or("invalid smoke outcomes")?;
    for (index, outcome) in outcomes.iter_mut().enumerate() {
        let Some(id) = outcome["execution_id"].as_str() else {
            continue;
        };
        let receipt = receipts
            .iter()
            .find(|r| r.id == id)
            .ok_or("missing campaign receipt")?;
        outcome["http_error"] = receipt.usage["http_error"].clone();
        outcome["money_outcome"] = json!(receipt.outcome);
        outcome["actual_nano_usd"] = json!(receipt.actual_nano_usd);
        outcome["reconciliation"] = receipt.usage["reconciliation"].clone();
        outcome["revision_identity"] = receipt.usage["revision_identity"].clone();
        outcome["qualification_eligible"] = if outcome["code"] == "invalid_answer" {
            json!(false)
        } else {
            receipt.usage["qualification_eligible"].clone()
        };
        let path = out.join(format!("receipt-{index}.json"));
        let mut exported: Value = assist::decode(&assist::read_bytes(&path, 32 * 1024 * 1024)?)?;
        exported["reconciliation"] = receipt.usage["reconciliation"].clone();
        exported["revision_identity"] = receipt.usage["revision_identity"].clone();
        exported["qualification_eligible"] = outcome["qualification_eligible"].clone();
        exported["failure_class"] = outcome["failure_class"].clone();
        exported["answer_reason"] = outcome["answer_reason"].clone();
        assist::write(&path, &exported)?;
    }
    smoke["reconciliation"] = json!(openrouter::reconciliation_status(ledger)?);
    smoke["qualified"] = json!(false);
    assist::write(&out.join("smoke.json"), smoke)?;
    Ok(())
}
fn reconcile_only(
    out: &Path,
    http: &dyn saccade_core::judge_provider::transport::Http,
    keys: &Keys,
) -> Result<(), Box<dyn std::error::Error>> {
    // Refuse a missing campaign before the ledger can create any state.
    assist::read_bytes(&out.join("ledger/campaign.json"), 32 * 1024 * 1024)?;
    let mut smoke: Value = assist::decode(&assist::read_bytes(
        &out.join("smoke.json"),
        32 * 1024 * 1024,
    )?)?;
    let ledger = Ledger::new(&out.join("ledger"), true);
    let result = openrouter::reconcile_pending(&ledger, http, keys, Duration::from_secs(30));
    export_reconciliation(out, &ledger, &mut smoke)?;
    result?;
    Ok(())
}
// Stage-2 transport collection validates the existing closed answer protocol.
// Scoring/qualification remains a separate operation over independent roots.
fn stage2_request(row: &Row) -> assist::Result<(Value, Digest)> {
    let text = row.payload["messages"][1]["content"][0]["text"]
        .as_str()
        .ok_or(assist::Error::Invalid("stage2 request data"))?;
    let data: Value = assist::decode(text.as_bytes())?;
    let hash: Digest = serde_json::from_value(data["request_hash"].clone())
        .map_err(|_| assist::Error::Invalid("stage2 request hash"))?;
    Ok((data, hash))
}
fn stage2_answer(row: &Row, response: &[u8]) -> assist::Result<Value> {
    let (data, hash) = stage2_request(row)?;
    let (answer, _) = openrouter::reply(response, &row.model, &hash)?;
    let value = serde_json::to_value(answer).map_err(|_| assist::Error::Storage)?;
    for observation in value["observations"]
        .as_array()
        .ok_or(assist::Error::Invalid("stage2 observations"))?
    {
        let view = data["views"]
            .as_array()
            .and_then(|views| views.iter().find(|v| v["slot"] == observation["slot"]))
            .ok_or(assist::Error::Invalid("stage2 observation slot"))?;
        let geometry = &observation["geometry"];
        let coords = geometry["pixels"]
            .as_array()
            .ok_or(assist::Error::Invalid("stage2 geometry"))?;
        let expected = match geometry["type"].as_str() {
            Some("box") => 4,
            Some("point") => 2,
            _ => 0,
        };
        if expected == 0
            || coords.len() != expected
            || coords.iter().any(|c| {
                c.as_f64()
                    .is_none_or(|n| !n.is_finite() || !(0.0..=1.0).contains(&n))
            })
        {
            return Err(assist::Error::Invalid("stage2 normalized geometry"));
        }
        if expected == 4
            && (coords[2].as_f64() == Some(0.0)
                || coords[3].as_f64() == Some(0.0)
                || coords[0]
                    .as_f64()
                    .zip(coords[2].as_f64())
                    .is_none_or(|(x, w)| x + w > 1.0)
                || coords[1]
                    .as_f64()
                    .zip(coords[3].as_f64())
                    .is_none_or(|(y, h)| y + h > 1.0))
        {
            return Err(assist::Error::Invalid("stage2 box bounds"));
        }
        let refs = observation["evidence_refs"]
            .as_array()
            .ok_or(assist::Error::Invalid("stage2 citations"))?;
        if refs.is_empty()
            || refs.iter().any(|r| {
                !view["regions"]
                    .as_array()
                    .is_some_and(|regions| regions.iter().any(|region| region["id"] == *r))
            })
        {
            return Err(assist::Error::Invalid("stage2 citation identity"));
        }
        let statement = observation["statement"].as_str().unwrap_or("");
        let supported = match observation["kind"].as_str() {
            Some("text") => statement
                .strip_prefix("text:")
                .is_some_and(|text| !text.is_empty()),
            Some("presence") => matches!(statement, "presence:present" | "presence:absent"),
            Some("clipping") => matches!(statement, "clipping:clipped" | "clipping:contained"),
            Some("overlap") => matches!(statement, "overlap:overlap" | "overlap:separate"),
            Some("appearance") => {
                matches!(statement, "appearance:changed" | "appearance:unchanged")
            }
            _ => false,
        };
        if !supported {
            return Err(assist::Error::Invalid("stage2 unsupported statement"));
        }
        if observation["uncertainty"]
            .as_f64()
            .is_none_or(|n| !(0.0..=1.0).contains(&n))
        {
            return Err(assist::Error::Invalid("stage2 uncertainty"));
        }
    }
    Ok(value)
}
fn record_response(
    row: &Row,
    response: &[u8],
    receipt: &impl serde::Serialize,
    out: &Path,
    index: usize,
    stage2: bool,
) -> assist::Result<CallOutcome> {
    std::fs::write(out.join(format!("response-{index}.json")), response)
        .map_err(|_| assist::Error::Storage)?;
    assist::write(&out.join(format!("receipt-{index}.json")), receipt)?;
    if stage2 {
        stage2_outcome(row, response, out, index)
    } else {
        Ok(CallOutcome::Completed)
    }
}
fn stage2_outcome(
    row: &Row,
    response: &[u8],
    out: &Path,
    index: usize,
) -> assist::Result<CallOutcome> {
    // Local request/schema failures are campaign errors, even when their generic
    // schema diagnostic matches an answer diagnostic. Keep them outside the match.
    stage2_request(row)?;
    // Check outer transport/accounting before inspecting model content. A bad
    // answer must never mask unknown money or incomplete execution.
    let v: Value = assist::decode(response)?;
    if v["choices"][0]["finish_reason"] == "length" {
        return Err(assist::Error::Invalid("truncated_output"));
    }
    if v["model"] != row.model
        || v["choices"].as_array().is_none_or(|a| a.len() != 1)
        || v["choices"][0]["finish_reason"] != "stop"
        || !v["choices"][0]["message"]["refusal"].is_null()
        || v["id"].as_str().is_none_or(str::is_empty)
        || v["provider"].as_str().is_none_or(str::is_empty)
    {
        return Err(assist::Error::Invalid(
            "OpenRouter incomplete, refused or misrouted answer",
        ));
    }
    let u = &v["usage"];
    if u["prompt_tokens"]
        .as_u64()
        .zip(u["completion_tokens"].as_u64())
        .is_none_or(|(a, b)| a.checked_add(b) != u["total_tokens"].as_u64())
        || openrouter::body_amount(response, &["usage", "cost"], true).is_none()
        || u.get("currency").is_some_and(|c| c != "USD")
    {
        return Err(assist::Error::Invalid(
            "OpenRouter inconsistent or unknown billed usage",
        ));
    }
    match stage2_answer(row, response) {
        Ok(answer) => {
            assist::write(&out.join(format!("answer-{index}.json")), &answer)?;
            Ok(CallOutcome::Completed)
        }
        Err(assist::Error::Invalid(reason)) => {
            let code = match reason {
                "stage2 citation identity" => "citation_identity",
                "stage2 observation slot" => "observation_slot",
                "stage2 normalized geometry" | "stage2 geometry" => "normalized_geometry",
                "stage2 box bounds" => "geometry_bounds",
                "stage2 uncertainty" => "uncertainty_range",
                "stage2 unsupported statement" => "unsupported_statement",
                "closed schema or JSON violation" => "closed_schema",
                "OpenRouter request-bound answer" => "request_bound_answer",
                "OpenRouter content" => "answer_content",
                _ => return Err(assist::Error::Invalid(reason)),
            };
            Ok(CallOutcome::InvalidAnswer(code))
        }
        Err(error) => Err(error),
    }
}
fn run_with(args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let mut options = BTreeMap::new();
    let mut above_25 = false;
    let mut stage2 = false;
    let mut budget_bounded = false;
    let mut validate_only = false;
    while let Some(arg) = args.next() {
        if arg == "--stage2" {
            stage2 = true;
            continue;
        }
        if arg == "--validate-only" {
            validate_only = true;
            continue;
        }
        if arg == "--budget-bounded" {
            budget_bounded = true;
            continue;
        }
        if arg == "--allow-spend-above-25-usd" {
            above_25 = true;
            continue;
        }
        if ![
            "--reconcile-only",
            "--requests",
            "--roots",
            "--max-spend-usd",
            "--out",
            "--user-policy",
            "--max-consecutive-invalid-answers",
            "--max-invalid-answer-percent",
            "--invalid-answer-min-sample",
        ]
        .contains(&arg.as_str())
            || options.contains_key(&arg)
        {
            return Err("invalid smoke arguments".into());
        }
        options.insert(arg, args.next().ok_or("missing argument value")?);
    }
    if let Some(out) = options.get("--reconcile-only") {
        if options.len() != 1 || above_25 || stage2 || validate_only || budget_bounded {
            return Err("reconcile-only accepts only an existing output directory".into());
        }
        return reconcile_only(
            Path::new(out),
            &Network,
            &Keys::new(Some(Keys::default_dir())),
        );
    }
    let required = |name: &str| options.get(name).ok_or("required smoke argument missing");
    let cap = allowance(required("--max-spend-usd")?, above_25)?;
    let count: usize = required("--roots")?.parse()?;
    if stage2 && cap > 5_000_000_000 {
        return Err("stage2 cap exceeds 5 USD".into());
    }
    if count == 0 || count > if stage2 { 1000 } else { 10 } {
        return Err(if stage2 {
            "stage2 requires 1 through 1000 roots"
        } else {
            "smoke requires 1 through 10 roots"
        }
        .into());
    }
    let mut answer_limits = AnswerLimits::default();
    for (name, value) in [
        (
            "--max-consecutive-invalid-answers",
            &mut answer_limits.max_consecutive,
        ),
        ("--invalid-answer-min-sample", &mut answer_limits.min_sample),
    ] {
        if let Some(text) = options.get(name) {
            *value = text.parse()?;
        }
        if *value == 0 || *value > 1000 {
            return Err("invalid answer safety sample limit".into());
        }
    }
    if let Some(text) = options.get("--max-invalid-answer-percent") {
        answer_limits.max_percent = text.parse()?;
    }
    if answer_limits.max_percent == 0 || answer_limits.max_percent > 100 {
        return Err("invalid answer safety percent".into());
    }
    if !stage2
        && options
            .keys()
            .any(|k| k.contains("invalid-answer") || k.contains("invalid-answers"))
    {
        return Err("answer safety options require stage2".into());
    }
    let path = PathBuf::from(required("--requests")?);
    let rows: Vec<Row> = assist::decode(&assist::read_bytes(&path, 32 * 1024 * 1024)?)?;
    validate_rows(&rows, count, cap, budget_bounded)?;
    if validate_only {
        let reservations: Vec<_> = rows
            .iter()
            .map(|row| {
                openrouter::admission(&serde_json::to_vec(&row.payload)?, &row.model)
                    .map(|a| a.reservation)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })
            })
            .collect::<Result<_, _>>()?;
        println!("{}", serde_json::to_string(&reservations)?);
        return Ok(());
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
    let campaign_deadline = Instant::now() + campaign_duration(stage2);
    let (mut outcomes, mut failed) = root_outcomes(
        &rows,
        campaign_deadline,
        budget_bounded,
        stage2.then_some(answer_limits),
        Instant::now,
        |index, deadline| {
            let executor = Executor {
                transport: &transport,
                ledger: &ledger,
                money_scopes: vec![MoneyScope {
                    id: "smoke".into(),
                    cap_nano_usd: cap,
                }],
                sources: vec![source.clone()],
                deadline,
            };
            let (key, payload) = &prepared[index];
            match executor.call(key, payload) {
                Ok(completed) => {
                    // Executor has settled money, checked identity and secret reflections.
                    record_response(
                        &rows[index],
                        &completed.response,
                        &completed.provenance,
                        &out,
                        index,
                        stage2,
                    )
                }
                Err(error) => Err(error),
            }
        },
    );
    // Generation publication is delayed: dispatch ends pending with no lookup.
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
        outcomes[index]["http_error"] = receipt.usage["http_error"].clone();
        outcomes[index]["money_outcome"] = json!(receipt.outcome);
        outcomes[index]["actual_nano_usd"] = json!(receipt.actual_nano_usd);
        if outcomes[index]["code"] != "completed" && outcomes[index]["code"] != "invalid_answer" {
            let bytes = serde_json::to_vec(receipt).map_err(|_| "smoke_receipt_encoding_failed")?;
            if std::fs::write(out.join(format!("receipt-{index}.json")), bytes).is_err() {
                outcomes[index]["artifact_code"] = json!("assist_storage_unavailable");
                failed = true;
            }
        }
    }
    let mut smoke = json!({"roots":count,"root_outcomes":outcomes,"receipt_code":receipt_code,"allowance_nano_usd":cap,"budget_bounded":budget_bounded,"campaign_seconds":campaign_duration(stage2).as_secs(),"dispatch_failed":failed,"answer_failure_policy":stage2.then_some(answer_limits),"qualified":false});
    export_reconciliation(&out, &ledger, &mut smoke)?;
    if failed {
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
    fn g12_smoke_exports_http_refusal_classification_and_zero_cost_root_outcome() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();
        let ledger = Ledger::new(&out.join("ledger"), true);
        let error = openrouter::http_error(
            Some(400),
            include_bytes!("../tests/fixtures/assist-openrouter/schema-http-400.json"),
        )
        .unwrap();
        ledger
            .reserve_money(
                &[MoneyScope {
                    id: "fixture".into(),
                    cap_nano_usd: 5_000_000,
                }],
                MoneyReceipt {
                    id: "fixture".into(),
                    request_hash: Digest::of_bytes(b"fixture"),
                    scopes: vec!["fixture".into()],
                    reserved_nano_usd: 5_000_000,
                    actual_nano_usd: None,
                    outcome: "reserved".into(),
                    usage: json!({"openrouter_dispatched":true}),
                },
            )
            .unwrap();
        ledger
            .finish_money(
                "fixture",
                Some(0),
                json!({"http_error":error,"zero_cost_refused":true}),
                false,
            )
            .unwrap();
        assist::write(
            &out.join("receipt-0.json"),
            &ledger.money_receipts().unwrap()[0],
        )
        .unwrap();
        let mut smoke = json!({"root_outcomes":[{"execution_id":"fixture","code":"openrouter_http_zero_cost_refused"}],"qualified":false});
        export_reconciliation(out, &ledger, &mut smoke).unwrap();
        let before = std::fs::read(out.join("ledger/campaign.json")).unwrap();
        // Terminal zero settlement needs neither a credential read nor a GET.
        run_with(vec![
            "--reconcile-only".into(),
            out.to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert_eq!(
            before,
            std::fs::read(out.join("ledger/campaign.json")).unwrap()
        );
        let recorded: Value =
            assist::decode(&std::fs::read(out.join("smoke.json")).unwrap()).unwrap();
        assert_eq!(recorded["root_outcomes"][0]["http_error"], json!(error));
        assert_eq!(recorded["root_outcomes"][0]["actual_nano_usd"], 0);
        assert_eq!(
            recorded["root_outcomes"][0]["money_outcome"],
            "zero_cost_refused"
        );
        assert_eq!(recorded["reconciliation"]["zero_cost_refused"], 1);
        assert_eq!(recorded["reconciliation"]["matched"], 0);
        assert_eq!(recorded["qualified"], false);
        assert_eq!(
            recorded["root_outcomes"][0]["qualification_eligible"],
            false
        );
        let exported: Value =
            assist::decode(&std::fs::read(out.join("receipt-0.json")).unwrap()).unwrap();
        assert_eq!(exported["usage"]["http_error"], json!(error));
    }
    #[test]
    fn g12_reconcile_only_needs_no_allowance_and_preserves_exported_provenance() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();
        let ledger = Ledger::new(&out.join("ledger"), true);
        ledger.reserve_money(&[MoneyScope { id: "fixture".into(), cap_nano_usd: 5_000_000 }], MoneyReceipt {
            id: "fixture".into(), request_hash: Digest::of_bytes(b"fixture"), scopes: vec!["fixture".into()],
            reserved_nano_usd: 5_000_000, actual_nano_usd: None, outcome: "reserved".into(),
            usage: json!({"openrouter_dispatched":true,"requested_identity":{"model":"google/gemini-3.8-flash","revision":"absent"}}),
        }).unwrap();
        ledger
            .finish_money(
                "fixture",
                Some(2_323_500),
                json!({"generation_id":"gen-fixture-001"}),
                true,
            )
            .unwrap();
        assist::write(
            &out.join("receipt-0.json"),
            &json!({"execution_id":"fixture","response_hash":"preserved"}),
        )
        .unwrap();
        let mut smoke = json!({"root_outcomes":[{"execution_id":"fixture","code":"completed"}],"qualified":false});
        export_reconciliation(out, &ledger, &mut smoke).unwrap();
        assert_eq!(smoke["reconciliation"]["state"], "pending");
        ledger
            .record_openrouter_reconciliation(
                "fixture",
                Ok(openrouter::Generation {
                    cost_nano_usd: 2_323_500,
                    response_hash: Digest::of_bytes(b"generation"),
                    model: "google/gemini-3.8-flash-20260902".into(),
                    provider_name: "Google AI Studio".into(),
                }),
                1,
                0,
                1000,
            )
            .unwrap();
        // Already matched: no network or credential access is needed to refresh artifacts.
        run_with(vec![
            "--reconcile-only".into(),
            out.to_string_lossy().into_owned(),
        ])
        .unwrap();
        let receipt: Value = assist::decode(
            &assist::read_bytes(&out.join("receipt-0.json"), 32 * 1024 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["response_hash"], "preserved");
        assert_eq!(
            receipt["revision_identity"]["dated_model"],
            "google/gemini-3.8-flash-20260902"
        );
        assert_eq!(receipt["reconciliation"]["state"], "matched");
        let smoke: Value =
            assist::decode(&assist::read_bytes(&out.join("smoke.json"), 32 * 1024 * 1024).unwrap())
                .unwrap();
        assert_eq!(smoke["reconciliation"]["state"], "matched");
        assert_eq!(smoke["qualified"], false);
        assert!(
            run_with(vec![
                "--reconcile-only".into(),
                out.to_string_lossy().into_owned(),
                "--max-spend-usd".into(),
                "1".into()
            ])
            .is_err()
        );
        let absent = out.join("absent");
        assert!(
            run_with(vec![
                "--reconcile-only".into(),
                absent.to_string_lossy().into_owned()
            ])
            .is_err()
        );
        assert!(!absent.exists());
    }
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
            assist::Error::Policy("openrouter_preflight_refused"),
            assist::Error::Invalid("OpenRouter inconsistent or unknown billed usage"),
            assist::Error::Invalid("openrouter_http_zero_cost_refused"),
        ] {
            let expected = error.code();
            let mut error = Some(error);
            let mut calls = 0;
            let (outcomes, failed) = root_outcomes(
                &rows,
                Instant::now() + campaign_duration(false),
                false,
                Some(AnswerLimits::default()),
                Instant::now,
                |index, _| {
                    calls += 1;
                    if index == 0 {
                        Ok(CallOutcome::Completed)
                    } else {
                        Err(error.take().unwrap())
                    }
                },
            );
            assert!(failed);
            assert_eq!(calls, 2);
            let encoded = serde_json::to_vec(&json!({"root_outcomes":outcomes})).unwrap();
            let smoke: Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(smoke["root_outcomes"][0]["code"], "completed");
            assert_eq!(smoke["root_outcomes"][1]["code"], expected);
            assert_eq!(smoke["root_outcomes"][2]["code"], "skipped_after_failure");
            assert_eq!(smoke["root_outcomes"][1]["root"], "fixture-1");
        }
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + campaign_duration(false),
            false,
            None,
            Instant::now,
            |_, _| Ok(CallOutcome::Completed),
        );
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
        assert!(validate_rows(&rows, 1, cost, false).is_ok());
        assert!(validate_rows(&rows, 1, cost - 1, false).is_err());
        rows[0].model = "unpriced/model".into();
        rows[0].payload["model"] = json!("unpriced/model");
        assert!(validate_rows(&rows, 1, u64::MAX, false).is_err());
        rows[0].model = model.into();
        rows[0].payload["model"] = json!(model);
        rows[0].payload["messages"][1]["content"][0]["text"] = json!("x".repeat(16000));
        assert!(validate_rows(&rows, 1, u64::MAX, false).is_err());
    }
    #[test]
    fn g12_recorded_length_response_is_truncated_in_root_outcomes() {
        let response = include_bytes!("../tests/fixtures/assist-openrouter/truncated-pilot.json");
        let row = Row {
            root: "fixture".into(),
            model: "google/gemini-3.8-flash".into(),
            revision: "absent".into(),
            payload: json!({"messages":[{}, {"content":[{"text":
                json!({"request_hash":Digest::of_bytes(b"fixture")}).to_string()}]}]}),
        };
        let rows = [row];
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + campaign_duration(true),
            false,
            None,
            Instant::now,
            |_, _| stage2_answer(&rows[0], response).map(|_| CallOutcome::Completed),
        );
        assert!(failed);
        assert_eq!(outcomes[0]["code"], "truncated_output");
        let mut refusal: Value = serde_json::from_slice(response).unwrap();
        refusal["choices"][0]["finish_reason"] = json!("stop");
        refusal["choices"][0]["message"]["refusal"] = json!("fixture refusal");
        assert_ne!(
            stage2_answer(&rows[0], &serde_json::to_vec(&refusal).unwrap())
                .unwrap_err()
                .code(),
            "truncated_output"
        );
        refusal["model"] = json!("other/model");
        assert_ne!(
            stage2_answer(&rows[0], &serde_json::to_vec(&refusal).unwrap())
                .unwrap_err()
                .code(),
            "truncated_output"
        );
    }
    #[test]
    fn stage2_mechanics_refuse_unbound_citations_and_geometry() {
        let hash = Digest::of_bytes(b"fixture");
        let row = Row {
            root: "fixture".into(),
            model: "google/gemini-3.8-flash".into(),
            revision: "absent".into(),
            payload: json!({"messages":[{}, {"content":[{"text":json!({"request_hash":hash,"views":[{"slot":"P1","regions":[{"id":"P1:R0"}]}]}).to_string()}]}]}),
        };
        let answer = json!({"request_hash":hash,"outcome":"observed","observations":[{"slot":"P1","kind":"presence","statement":"presence:present","geometry":{"type":"box","pixels":[0.0,0.0,1.0,1.0]},"visibility":"visible","evidence_refs":["P1:R0"],"uncertainty":0.0}]});
        let response = |a: &Value| {
            serde_json::to_vec(&json!({"id":"gen-fixture","model":row.model,"provider":"fixture","choices":[{"finish_reason":"stop","message":{"content":a.to_string()}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2,"cost":0.001}})).unwrap()
        };
        assert!(stage2_answer(&row, &response(&answer)).is_ok());
        let mut bad = answer.clone();
        bad["observations"][0]["evidence_refs"] = json!(["invented"]);
        assert!(stage2_answer(&row, &response(&bad)).is_err());
        let mut bad = answer.clone();
        bad["observations"][0]["geometry"]["pixels"] = json!([0.5, 0.0, 1.0, 1.0]);
        assert!(stage2_answer(&row, &response(&bad)).is_err());
        let mut bad = answer;
        bad["request_hash"] = json!(Digest::of_bytes(b"other"));
        assert!(stage2_answer(&row, &response(&bad)).is_err());
    }
    #[test]
    fn stage2_schedule_validation_is_offline_and_keeps_five_dollar_cap() {
        let temp = tempfile::tempdir().unwrap();
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let payload: Value = assist::decode(
            &openrouter::request(
                &serde_json::to_vec(&source).unwrap(),
                "google/gemini-3.8-flash",
            )
            .unwrap(),
        )
        .unwrap();
        let rows: Vec<_> = (0..11).map(|i| json!({"root":format!("fixture-{i}"),"model":"google/gemini-3.8-flash","revision":"google/gemini-3.8-flash-20260902","payload":payload})).collect();
        let path = temp.path().join("requests.json");
        std::fs::write(&path, serde_json::to_vec(&rows).unwrap()).unwrap();
        let args = vec![
            "--stage2".into(),
            "--validate-only".into(),
            "--requests".into(),
            path.to_string_lossy().into_owned(),
            "--roots".into(),
            "11".into(),
            "--max-spend-usd".into(),
            "5".into(),
        ];
        run_with(args.clone()).unwrap();
        let mut bounded = args.clone();
        *bounded.last_mut().unwrap() = "0.001".into();
        assert!(run_with(bounded.clone()).is_err());
        bounded.push("--budget-bounded".into());
        run_with(bounded).unwrap();
        let mut over = args.clone();
        *over.last_mut().unwrap() = "5.000000001".into();
        assert!(run_with(over.clone()).is_err());
        over.push("--budget-bounded".into());
        assert!(run_with(over).is_err());
        assert!(run_with(args.into_iter().filter(|a| a != "--stage2")).is_err());
        assert!(!temp.path().join("ledger").exists());
    }
    #[test]
    fn g12_stage2_per_root_deadlines_survive_pacing_and_stop_campaign() {
        use std::cell::Cell;
        let started = Instant::now();
        let clock = Cell::new(started);
        let campaign_deadline = started + campaign_duration(true);
        let rows: Vec<_> = (0..300)
            .map(|i| Row {
                root: format!("paced-{i}"),
                model: "fixture/model".into(),
                revision: "fixture".into(),
                payload: Value::Null,
            })
            .collect();
        let mut calls = 0;
        let (outcomes, failed) = root_outcomes(
            &rows,
            campaign_deadline,
            false,
            None,
            || clock.get(),
            |_, deadline| {
                // Fixture executor enforces the unchanged 300s admission guard.
                // Passing the old six-hour deadline here fails on the first root.
                let remaining = deadline.saturating_duration_since(clock.get());
                if remaining.is_zero() || remaining > CALL_LIMIT {
                    return Err(assist::Error::Policy("deadline limit"));
                }
                assert!(deadline <= campaign_deadline);
                calls += 1;
                // Simulated transport/pacing, with no sockets or real sleeps.
                clock.set(clock.get() + Duration::from_secs(90));
                Ok(CallOutcome::Completed)
            },
        );
        assert!(!failed);
        assert_eq!(calls, 240);
        assert!(clock.get().duration_since(started) > CALL_LIMIT);
        assert_eq!(clock.get(), campaign_deadline);
        assert_eq!(outcomes.len(), rows.len());
        assert!(outcomes[..calls].iter().all(|o| o["code"] == "completed"));
        assert!(
            outcomes[calls..]
                .iter()
                .all(|o| o["code"] == "not_run_deadline")
        );
        // An already expired campaign never reaches the fixture executor.
        let (outcomes, failed) = root_outcomes(
            &rows,
            started,
            false,
            None,
            || started,
            |_, _| panic!("expired campaign dispatched"),
        );
        assert!(!failed);
        assert!(outcomes.iter().all(|o| o["code"] == "not_run_deadline"));
        // If a call fails as the campaign expires, its failure stays visible
        // and the remaining schedule is unavailable due to the deadline.
        clock.set(started);
        let (outcomes, failed) = root_outcomes(
            &rows,
            campaign_deadline,
            false,
            None,
            || clock.get(),
            |_, _| {
                clock.set(campaign_deadline);
                Err(assist::Error::Provider)
            },
        );
        assert!(failed);
        assert_eq!(outcomes[0]["code"], "assist_provider_execution_incomplete");
        assert!(
            outcomes[1..]
                .iter()
                .all(|o| o["code"] == "not_run_deadline")
        );
    }
    #[test]
    fn g12_budget_bounded_schedule_reserves_until_next_shortfall_and_retains_tail() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let model = "google/gemini-3.8-flash";
        let source = json!({"systemInstruction":{"parts":[{"text":"fixture"}]},"contents":[{"parts":[{"text":"fixture"}]}]});
        let payload: Value = assist::decode(
            &openrouter::request(&serde_json::to_vec(&source).unwrap(), model).unwrap(),
        )
        .unwrap();
        let rows: Vec<_> = (0..10)
            .map(|i| Row {
                root: format!("budget-{i}"),
                model: model.into(),
                revision: "absent".into(),
                payload: payload.clone(),
            })
            .collect();
        let reservation = openrouter::admission(&serde_json::to_vec(&payload).unwrap(), model)
            .unwrap()
            .reservation;
        // Half a reservation settles after each call; actual usage, not the
        // full schedule or a guessed average, determines subsequent admission.
        let actual = reservation / 2;
        let cap = reservation + 2 * actual;
        assert!(validate_rows(&rows, rows.len(), cap, false).is_err());
        validate_rows(&rows, rows.len(), cap, true).unwrap();
        assert!(validate_rows(&rows, rows.len() - 1, cap, true).is_err());
        let ledger = Ledger::new(&temp.path().join("ledger"), true);
        let scopes = [MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: cap,
        }];
        let mut dispatches = 0;
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + campaign_duration(true),
            true,
            None,
            Instant::now,
            |index, _| {
                let id = format!("budget-{index}");
                ledger
                    .reserve_money(
                        &scopes,
                        MoneyReceipt {
                            id: id.clone(),
                            request_hash: Digest::of_bytes(id.as_bytes()),
                            scopes: vec!["smoke".into()],
                            reserved_nano_usd: reservation,
                            actual_nano_usd: None,
                            outcome: "reserved".into(),
                            usage: Value::Null,
                        },
                    )
                    .map_err(|reason| {
                        assert_eq!(reason, "money_budget_exhausted");
                        assist::Error::Policy("money budget exhausted")
                    })?;
                dispatches += 1;
                ledger
                    .finish_money(&id, Some(actual), json!({"fixture":true}), true)
                    .unwrap();
                Ok(CallOutcome::Completed)
            },
        );
        assert!(!failed);
        assert_eq!(dispatches, 3);
        assert_eq!(ledger.money_receipts().unwrap().len(), dispatches);
        assert_eq!(outcomes.len(), rows.len());
        assert!(
            outcomes[..dispatches]
                .iter()
                .all(|o| o["code"] == "completed")
        );
        assert!(
            outcomes[dispatches..]
                .iter()
                .all(|o| o["code"] == "not_run_budget")
        );
        // Security/accounting failures are never relabelled as clean budget stops.
        for error in [
            assist::Error::Policy("money reservation refused"),
            assist::Error::Policy("openrouter_preflight_refused"),
            assist::Error::Invalid("provider usage exceeded reservation"),
        ] {
            let code = error.code();
            let mut error = Some(error);
            let (outcomes, failed) = root_outcomes(
                &rows,
                Instant::now() + campaign_duration(true),
                true,
                Some(AnswerLimits::default()),
                Instant::now,
                |_, _| Err(error.take().unwrap()),
            );
            assert!(failed);
            assert_eq!(outcomes[0]["code"], code);
            assert!(
                outcomes[1..]
                    .iter()
                    .all(|o| o["code"] == "skipped_after_failure")
            );
        }
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + campaign_duration(true),
            false,
            None,
            Instant::now,
            |_, _| Err(assist::Error::Policy("money budget exhausted")),
        );
        assert!(failed);
        assert_eq!(outcomes[0]["code"], "money budget exhausted");
        assert_eq!(outcomes[1]["code"], "skipped_after_failure");
    }
    fn cross_citation_row(root: &str) -> Row {
        Row {
            root: root.into(),
            model: "google/gemini-3.8-flash".into(),
            revision: "absent".into(),
            payload: json!({"messages":[{}, {"content":[{"text":json!({
                "request_hash":"sha256:752179b2327daedbe5ed07aa3bf09389b7cb4b729c9cc3296fd56dcbdf7fe664",
                "views":[{"slot":"P1","regions":[{"id":"P1:R0"}]},{"slot":"P2","regions":[{"id":"P2:R0"}]}]
            }).to_string()}]}]}),
        }
    }
    #[test]
    fn g12_recorded_cross_citation_is_invalid_and_next_root_dispatches() {
        let temp = tempfile::tempdir().unwrap();
        let response =
            include_bytes!("../tests/fixtures/assist-openrouter/cross-citation-pilot.json");
        let rows = [cross_citation_row("first"), cross_citation_row("next")];
        assert_eq!(
            stage2_answer(&rows[0], response).unwrap_err().code(),
            "stage2 citation identity"
        );
        let mut dispatches = 0;
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + CALL_LIMIT,
            false,
            Some(AnswerLimits::default()),
            Instant::now,
            |index, _| {
                dispatches += 1;
                // A settled fixture receipt is retained beside the rejected response.
                record_response(
                    &rows[index],
                    response,
                    &json!({"execution_id":format!("fixture-{index}"),"actual_nano_usd":3583500}),
                    temp.path(),
                    index,
                    true,
                )
            },
        );
        assert!(!failed);
        assert_eq!(dispatches, 2);
        for (index, outcome) in outcomes.iter().enumerate() {
            assert_eq!(outcome["code"], "invalid_answer");
            assert_eq!(outcome["failure_class"], "answer_failure");
            assert_eq!(outcome["answer_reason"], "citation_identity");
            assert_eq!(
                std::fs::read(temp.path().join(format!("response-{index}.json"))).unwrap(),
                response
            );
            let receipt: Value = assist::decode(
                &std::fs::read(temp.path().join(format!("receipt-{index}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(receipt["actual_nano_usd"], 3583500);
            assert!(!temp.path().join(format!("answer-{index}.json")).exists());
        }
        // Storage and unknown money remain campaign errors even with this bad answer.
        assert!(matches!(
            record_response(
                &rows[0],
                response,
                &json!({}),
                &temp.path().join("absent"),
                0,
                true
            ),
            Err(assist::Error::Storage)
        ));
        let mut unknown: Value = assist::decode(response).unwrap();
        let mut local = cross_citation_row("malformed-request");
        local.payload["messages"][1]["content"][0]["text"] = json!("malformed local JSON");
        assert!(matches!(
            stage2_outcome(&local, response, temp.path(), 0),
            Err(assist::Error::Invalid("closed schema or JSON violation"))
        ));
        unknown["usage"]["cost"] = Value::Null;
        assert_eq!(
            stage2_outcome(
                &rows[0],
                &serde_json::to_vec(&unknown).unwrap(),
                temp.path(),
                0
            )
            .unwrap_err()
            .code(),
            "OpenRouter inconsistent or unknown billed usage"
        );
    }
    #[test]
    fn g12_answer_safety_valves_trip_strictly_above_limits_and_reset_streaks() {
        let rows: Vec<_> = (0..30)
            .map(|i| cross_citation_row(&format!("valve-{i}")))
            .collect();
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + CALL_LIMIT,
            false,
            Some(AnswerLimits::default()),
            Instant::now,
            |_, _| Ok(CallOutcome::InvalidAnswer("citation_identity")),
        );
        assert!(failed);
        assert_eq!(outcomes[4]["safety_valve"], Value::Null);
        assert_eq!(outcomes[5]["code"], "invalid_answer");
        assert_eq!(outcomes[5]["safety_valve"], "consecutive_invalid_answers");
        assert!(
            outcomes[6..]
                .iter()
                .all(|o| o["code"] == "not_run_answer_safety_valve")
        );
        let limits = AnswerLimits {
            max_consecutive: 30,
            max_percent: 50,
            min_sample: 20,
        };
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + CALL_LIMIT,
            false,
            Some(limits),
            Instant::now,
            |i, _| {
                Ok(if i % 2 == 0 || i == 20 {
                    CallOutcome::InvalidAnswer("citation_identity")
                } else {
                    CallOutcome::Completed
                })
            },
        );
        assert!(failed);
        assert_eq!(outcomes[19]["safety_valve"], Value::Null); // exactly 50%
        assert_eq!(outcomes[20]["safety_valve"], "invalid_answer_rate");
        assert_eq!(outcomes[21]["code"], "not_run_answer_safety_valve");
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + CALL_LIMIT,
            false,
            Some(AnswerLimits::default()),
            Instant::now,
            |i, _| {
                Ok(if i % 2 == 1 {
                    CallOutcome::InvalidAnswer("citation_identity")
                } else {
                    CallOutcome::Completed
                })
            },
        );
        assert!(!failed); // streaks reset; every prefix is at most 50%
        assert!(outcomes.iter().all(|o| o["safety_valve"].is_null()));
        // Reaching the minimum on a valid answer still checks the accumulated rate.
        let (outcomes, failed) = root_outcomes(
            &rows,
            Instant::now() + CALL_LIMIT,
            false,
            Some(limits),
            Instant::now,
            |i, _| {
                Ok(if i == 19 {
                    CallOutcome::Completed
                } else {
                    CallOutcome::InvalidAnswer("citation_identity")
                })
            },
        );
        assert!(failed);
        assert_eq!(outcomes[18]["safety_valve"], Value::Null);
        assert_eq!(outcomes[19]["code"], "completed");
        assert_eq!(outcomes[19]["safety_valve"], "invalid_answer_rate");
        assert_eq!(outcomes[20]["code"], "not_run_answer_safety_valve");
    }
    #[test]
    fn g12_invalid_answer_reconciliation_preserves_failure_and_provenance() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(&temp.path().join("ledger"), true);
        ledger.reserve_money(&[MoneyScope { id: "fixture".into(), cap_nano_usd: 5_000_000 }], MoneyReceipt {
            id: "fixture".into(), request_hash: Digest::of_bytes(b"fixture"), scopes: vec!["fixture".into()],
            reserved_nano_usd: 5_000_000, actual_nano_usd: None, outcome: "reserved".into(),
            usage: json!({"openrouter_dispatched":true,"requested_identity":{"model":"google/gemini-3.8-flash","revision":"absent"}}),
        }).unwrap();
        ledger
            .finish_money(
                "fixture",
                Some(3_583_500),
                json!({"generation_id":"gen-fixture"}),
                true,
            )
            .unwrap();
        assist::write(
            &temp.path().join("receipt-0.json"),
            &json!({"execution_id":"fixture","response_hash":"preserved"}),
        )
        .unwrap();
        let mut smoke = json!({"root_outcomes":[{"execution_id":"fixture","code":"invalid_answer","failure_class":"answer_failure","answer_reason":"citation_identity"}]});
        export_reconciliation(temp.path(), &ledger, &mut smoke).unwrap();
        ledger
            .record_openrouter_reconciliation(
                "fixture",
                Ok(openrouter::Generation {
                    cost_nano_usd: 3_583_500,
                    response_hash: Digest::of_bytes(b"generation"),
                    model: "google/gemini-3.8-flash-20260902".into(),
                    provider_name: "Google AI Studio".into(),
                }),
                1,
                0,
                1000,
            )
            .unwrap();
        export_reconciliation(temp.path(), &ledger, &mut smoke).unwrap();
        assert_eq!(smoke["root_outcomes"][0]["code"], "invalid_answer");
        assert_eq!(smoke["root_outcomes"][0]["qualification_eligible"], false);
        assert_eq!(smoke["root_outcomes"][0]["actual_nano_usd"], 3_583_500);
        assert_eq!(
            smoke["root_outcomes"][0]["reconciliation"]["state"],
            "matched"
        );
        let exported: Value =
            assist::decode(&std::fs::read(temp.path().join("receipt-0.json")).unwrap()).unwrap();
        assert_eq!(exported["response_hash"], "preserved");
        assert_eq!(exported["answer_reason"], "citation_identity");
        assert_eq!(exported["qualification_eligible"], false);
    }
    #[test]
    fn g12_answer_safety_options_validate_offline() {
        let temp = tempfile::tempdir().unwrap();
        let args = vec![
            "--stage2".into(),
            "--validate-only".into(),
            "--roots".into(),
            "1".into(),
            "--max-spend-usd".into(),
            "5".into(),
            "--requests".into(),
            temp.path().join("absent").to_string_lossy().into_owned(),
        ];
        for (name, value) in [
            ("--max-consecutive-invalid-answers", "0"),
            ("--max-consecutive-invalid-answers", "1001"),
            ("--max-invalid-answer-percent", "101"),
            ("--max-invalid-answer-percent", "0"),
            ("--invalid-answer-min-sample", "0"),
        ] {
            let mut invalid = args.clone();
            invalid.extend([name.into(), value.into()]);
            assert!(
                run_with(invalid)
                    .unwrap_err()
                    .to_string()
                    .starts_with("invalid answer safety")
            );
        }
        let mut invalid: Vec<_> = args.into_iter().filter(|a| a != "--stage2").collect();
        invalid.extend(["--max-invalid-answer-percent".into(), "50".into()]);
        assert_eq!(
            run_with(invalid).unwrap_err().to_string(),
            "answer safety options require stage2"
        );
        assert!(!temp.path().join("ledger").exists());
    }
    #[test]
    fn g12_post_settlement_protocol_reasons_stay_refused() {
        let temp = tempfile::tempdir().unwrap();
        let row = cross_citation_row("protocol");
        let response: Value = assist::decode(include_bytes!(
            "../tests/fixtures/assist-openrouter/cross-citation-pilot.json"
        ))
        .unwrap();
        let mut answer: Value = assist::decode(
            response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        answer["observations"].as_array_mut().unwrap().pop();
        for (pointer, value, expected) in [
            ("/observations/0/slot", json!("P2"), "citation_identity"),
            (
                "/observations/0/geometry/pixels",
                json!([0.5, 0.0, 1.0, 1.0]),
                "geometry_bounds",
            ),
            (
                "/observations/0/geometry/pixels",
                json!([2.0, 0.0, 0.1, 0.1]),
                "closed_schema",
            ),
            ("/observations/0/uncertainty", json!(2.0), "closed_schema"),
            ("/observations/0/slot", json!("P3"), "closed_schema"),
            (
                "/observations/0/statement",
                json!("caused by a broken engine"),
                "unsupported_statement",
            ),
            (
                "/request_hash",
                json!(Digest::of_bytes(b"other")),
                "request_bound_answer",
            ),
        ] {
            let mut bad = answer.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            let mut envelope = response.clone();
            envelope["choices"][0]["message"]["content"] = json!(bad.to_string());
            assert_eq!(
                stage2_outcome(
                    &row,
                    &serde_json::to_vec(&envelope).unwrap(),
                    temp.path(),
                    0
                )
                .unwrap(),
                CallOutcome::InvalidAnswer(expected)
            );
            assert!(!temp.path().join("answer-0.json").exists());
        }
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
