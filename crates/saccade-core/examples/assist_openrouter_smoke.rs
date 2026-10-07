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
// Runner accounting semantics changed; request/cache/prompt identities stay fixed.
const CAMPAIGN_SCHEMA: &str = "saccade-g12-campaign/3";
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
#[derive(Clone, Copy, serde::Serialize, PartialEq)]
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
#[cfg(test)]
fn root_outcomes(
    rows: &[Row],
    campaign_deadline: Instant,
    budget_bounded: bool,
    limits: Option<AnswerLimits>,
    now: impl FnMut() -> Instant,
    call: impl FnMut(usize, Instant) -> assist::Result<CallOutcome>,
) -> (Vec<Value>, bool) {
    continue_roots(
        rows,
        campaign_deadline,
        budget_bounded,
        limits,
        &[],
        &BTreeSet::new(),
        now,
        call,
        |_| Ok(()),
    )
}
#[allow(clippy::too_many_arguments)]
fn continue_roots(
    rows: &[Row],
    campaign_deadline: Instant,
    budget_bounded: bool,
    limits: Option<AnswerLimits>,
    prior: &[Value],
    settled: &BTreeSet<usize>,
    mut now: impl FnMut() -> Instant,
    mut call: impl FnMut(usize, Instant) -> assist::Result<CallOutcome>,
    mut checkpoint: impl FnMut(&[Value]) -> assist::Result<()>,
) -> (Vec<Value>, bool) {
    let mut failed = false;
    let mut stop = None;
    let (mut samples, mut invalid, mut consecutive) = (0usize, 0usize, 0usize);
    let mut outcomes = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let old = settled.contains(&index).then(|| &prior[index]);
        let mut reason = old.and_then(|o| o["answer_reason"].as_str());
        let code = if let Some(old) = old {
            old["code"]
                .as_str()
                .unwrap_or("assist_provider_execution_incomplete")
        } else if let Some(code) = stop {
            code
        } else {
            let started = now();
            if started >= campaign_deadline {
                stop = Some("not_run_deadline");
                "not_run_deadline"
            } else {
                let deadline = campaign_deadline.min(started + CALL_LIMIT);
                match call(index, deadline) {
                    Ok(CallOutcome::Completed) => "completed",
                    Ok(CallOutcome::InvalidAnswer(code)) => {
                        reason = Some(code);
                        "invalid_answer"
                    }
                    Err(assist::Error::Policy("money budget exhausted")) if budget_bounded => {
                        stop = Some("not_run_budget");
                        "not_run_budget"
                    }
                    Err(error) => {
                        failed = true;
                        stop = Some(if now() >= campaign_deadline {
                            "not_run_deadline"
                        } else {
                            "skipped_after_failure"
                        });
                        error.code()
                    }
                }
            }
        };
        let mut valve = None;
        if matches!(code, "completed" | "invalid_answer") {
            samples += 1;
            if code == "invalid_answer" {
                invalid += 1;
                consecutive += 1;
            } else {
                consecutive = 0;
            }
            if let Some(limits) = limits {
                valve = if consecutive > limits.max_consecutive {
                    Some("consecutive_invalid_answers")
                } else if samples >= limits.min_sample
                    && invalid * 100 > samples * usize::from(limits.max_percent)
                {
                    Some("invalid_answer_rate")
                } else {
                    None
                };
                if valve.is_some() {
                    failed = true;
                    stop = Some("not_run_answer_safety_valve");
                }
            }
        }
        let class = match code {
            "completed" => "valid_answer",
            "invalid_answer" => "answer_failure",
            "not_run_deadline"
            | "not_run_budget"
            | "not_run_answer_safety_valve"
            | "skipped_after_failure" => "not_run",
            _ => "campaign_failure",
        };
        let mut outcome = old.cloned().unwrap_or_else(|| {
            json!({"index":index,"root":row.root,"code":code,
            "failure_class":class,"answer_reason":reason,"safety_valve":valve})
        });
        if valve.is_some() {
            outcome["safety_valve"] = json!(valve);
        }
        outcomes.push(outcome);
        if checkpoint(&outcomes).is_err() {
            failed = true;
            stop = Some("skipped_after_failure");
            outcomes[index]["artifact_code"] = json!("assist_storage_unavailable");
        }
    }
    (outcomes, failed)
}
// Associate every attempt by its durable root marker, never by receipt ordinal.
fn attach_receipts(out: &Path, ledger: &Ledger, smoke: &mut Value) -> assist::Result<()> {
    let receipts = ledger
        .money_receipts()
        .map_err(|_| assist::Error::Storage)?;
    for receipt in &receipts {
        let index = receipt.usage["campaign_root_index"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(assist::Error::Storage)?;
        let outcome = smoke["root_outcomes"]
            .get_mut(index)
            .ok_or(assist::Error::Storage)?;
        outcome["execution_id"] = json!(receipt.id);
        outcome["response_identity"] = receipt.usage["response_identity"].clone();
        outcome["provider_reasoning_over_hint"] =
            receipt.usage["provider_reasoning_over_hint"].clone();
        outcome["reasoning_hint"] = receipt.usage["reasoning_hint"].clone();
        outcome["transport_failure"] = receipt.usage["transport_failure"].clone();
        outcome["http_error"] = receipt.usage["http_error"].clone();
        outcome["money_outcome"] = json!(receipt.outcome);
        outcome["actual_nano_usd"] = json!(receipt.actual_nano_usd);
        // Keep every monetary attempt, including reconciled incomplete dispatches.
        assist::write(&out.join(format!("money-{}.json", receipt.id)), receipt)?;
        let path = out.join(format!("receipt-{index}.json"));
        if !matches!(
            outcome["code"].as_str(),
            Some("completed" | "invalid_answer")
        ) {
            assist::write(&path, receipt)?;
        }
    }
    Ok(())
}
fn resume_roots(
    rows: &[Row],
    ledger: &Ledger,
    smoke: &Value,
) -> Result<BTreeSet<usize>, Box<dyn std::error::Error>> {
    let latest = resume_bindings(rows, ledger, smoke)?;
    let outcomes = smoke["root_outcomes"]
        .as_array()
        .ok_or("invalid resume outcomes")?;
    let mut settled: BTreeSet<_> = ledger
        .money_receipts()?
        .iter()
        .filter(|r| r.outcome == "settled_conservatively")
        .filter_map(|r| r.usage["campaign_root_index"].as_u64().map(|i| i as usize))
        .collect();
    for (index, outcome) in outcomes.iter().enumerate() {
        if settled.contains(&index) {
            continue;
        }
        if let Some(receipt) = latest.get(&index) {
            if receipt.actual_nano_usd.is_none() || receipt.outcome == "reserved" {
                return Err("resume requires reconciled incomplete cost".into());
            }
            if receipt.outcome == "settled_conservatively" {
                settled.insert(index);
                continue;
            }
            if receipt.outcome == "zero_cost_refused"
                || (receipt.outcome == "completed"
                    && matches!(
                        outcome["code"].as_str(),
                        Some("completed" | "invalid_answer")
                    ))
            {
                if outcome["execution_id"] != receipt.id {
                    return Err("resume receipt identity changed".into());
                }
                settled.insert(index);
            } else if receipt.outcome != "incomplete"
                || receipt.usage["reconciliation"]["state"] != "matched"
            {
                return Err(
                    "resume requires matched incomplete receipt or settled answer artifacts".into(),
                );
            }
        } else if outcome["execution_id"].is_string()
            || matches!(
                outcome["code"].as_str(),
                Some("completed" | "invalid_answer")
            )
        {
            return Err("resume missing settled receipt".into());
        }
    }
    Ok(settled)
}
// Validate all durable attempts before any operator settlement can mutate charges.
fn resume_bindings(
    rows: &[Row],
    ledger: &Ledger,
    smoke: &Value,
) -> Result<BTreeMap<usize, saccade_core::budget_ledger::MoneyReceipt>, Box<dyn std::error::Error>>
{
    let outcomes = smoke["root_outcomes"]
        .as_array()
        .ok_or("invalid resume outcomes")?;
    if outcomes.len() != rows.len()
        || outcomes
            .iter()
            .zip(rows)
            .enumerate()
            .any(|(i, (o, r))| o["index"] != i || o["root"] != r.root)
    {
        return Err("resume topology changed".into());
    }
    let receipts = ledger.money_receipts()?;
    let mut latest = BTreeMap::new();
    for receipt in &receipts {
        if receipt.usage["reconciliation"]["state"] == "mismatch"
            && receipt.outcome != "settled_conservatively"
            && !(receipt.actual_nano_usd.is_none()
                && receipt.outcome == "incomplete"
                && receipt.usage["generation_id"].is_null()
                && receipt.usage["reconciliation"]["generation"].is_null()
                && receipt.usage["reconciliation"]["reason"] == "openrouter_generation_missing")
        {
            return Err("openrouter_reconciliation_mismatch".into());
        }
        let index = receipt.usage["campaign_root_index"]
            .as_u64()
            .ok_or("resume receipt lacks root binding")? as usize;
        let row = rows.get(index).ok_or("resume receipt root changed")?;
        if receipt.request_hash != Digest::of_bytes(&serde_json::to_vec(&row.payload)?) {
            return Err("resume receipt payload changed".into());
        }
        latest.insert(index, receipt.clone());
    }
    Ok(latest)
}
fn settle_resume_unknown(
    rows: &[Row],
    ledger: &Ledger,
    smoke: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    resume_bindings(rows, ledger, smoke)?;
    ledger.settle_unknown_at_reservation()?;
    Ok(())
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
        outcome["transport_failure"] = receipt.usage["transport_failure"].clone();
        outcome["http_error"] = receipt.usage["http_error"].clone();
        outcome["money_outcome"] = json!(receipt.outcome);
        outcome["actual_nano_usd"] = json!(receipt.actual_nano_usd);
        outcome["unknown_cost_settlement"] = receipt.usage["unknown_cost_settlement"].clone();
        if receipt.outcome == "settled_conservatively" {
            if outcome["code"] != "not_run_transport_failure" {
                outcome["previous_code"] = outcome["code"].clone();
            }
            outcome["code"] = json!("not_run_transport_failure");
            outcome["failure_class"] = json!("not_run");
        }
        outcome["reconciliation"] = receipt.usage["reconciliation"].clone();
        outcome["revision_identity"] = receipt.usage["revision_identity"].clone();
        outcome["qualification_eligible"] = if outcome["code"] == "invalid_answer" {
            json!(false)
        } else {
            receipt.usage["qualification_eligible"].clone()
        };
        let path = out.join(format!("receipt-{index}.json"));
        let mut exported: Value = assist::decode(&assist::read_bytes(&path, 32 * 1024 * 1024)?)?;
        if exported.get("reserved_nano_usd").is_some() {
            exported = serde_json::to_value(receipt)?;
        }
        if receipt.usage["campaign_root_index"].is_u64() {
            assist::write(&out.join(format!("money-{}.json", receipt.id)), receipt)?;
        }
        exported["unknown_cost_settlement"] = receipt.usage["unknown_cost_settlement"].clone();
        exported["transport_failure"] = receipt.usage["transport_failure"].clone();
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
fn campaign_lock(out: &Path) -> Result<std::fs::File, Box<dyn std::error::Error>> {
    if !out.is_dir() || std::fs::symlink_metadata(out)?.file_type().is_symlink() {
        return Err("invalid campaign directory".into());
    }
    let lock_path = out.join("runner.lock");
    if std::fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("campaign lock symlink".into());
    }
    let run_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    fs2::FileExt::try_lock_exclusive(&run_lock).map_err(|_| "campaign already running")?;
    Ok(run_lock)
}
fn reconcile_only(
    out: &Path,
    http: &dyn saccade_core::judge_provider::transport::Http,
    keys: &Keys,
) -> Result<(), Box<dyn std::error::Error>> {
    let _run_lock = campaign_lock(out)?;
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
    let mut preflight_only = false;
    let mut settle_unknown = false;
    while let Some(arg) = args.next() {
        if arg == "--settle-unknown-at-reservation" {
            if settle_unknown {
                return Err("duplicate settlement flag".into());
            }
            settle_unknown = true;
            continue;
        }
        if arg == "--stage2" {
            stage2 = true;
            continue;
        }
        if arg == "--preflight-only" {
            preflight_only = true;
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
            "--resume",
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
    if settle_unknown
        && (!options.contains_key("--resume")
            || validate_only
            || preflight_only
            || options.contains_key("--reconcile-only"))
    {
        return Err("settlement requires resume execution".into());
    }
    if let Some(out) = options.get("--reconcile-only") {
        if options.len() != 1
            || above_25
            || stage2
            || validate_only
            || preflight_only
            || budget_bounded
        {
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
    let request_bytes = assist::read_bytes(&path, 32 * 1024 * 1024)?;
    let requests_hash = Digest::of_bytes(&request_bytes);
    let rows: Vec<Row> = assist::decode(&request_bytes)?;
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
    require_scorer_selftest()?;
    if stage2 {
        for row in &rows {
            let (data, _) = stage2_request(row)?;
            if data["prompt_epoch"] != "g12-pilot/4"
                || data["prompt_policy"] != "assist-openrouter-task-evidence/4"
            {
                return Err("paid stage2 run requires task-evidence policy epoch 4".into());
            }
        }
    }
    if options.contains_key("--resume") && options.contains_key("--out") {
        return Err("resume and out are exclusive".into());
    }
    let resume = options.contains_key("--resume");
    let out = PathBuf::from(if resume {
        required("--resume")?
    } else {
        required("--out")?
    });
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
            evidence_hash: requests_hash.clone(),
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
    // This boundary precedes all campaign writes, credential reads and HTTP.
    if preflight_only {
        println!(
            "{}",
            json!({"code":"smoke_offline_preflight_passed", "requests":count,
            "requests_hash":requests_hash, "qualified":false})
        );
        return Ok(());
    }
    if !resume {
        std::fs::create_dir(&out)?;
    }
    if !out.is_dir() || std::fs::symlink_metadata(&out)?.file_type().is_symlink() {
        return Err("invalid campaign directory".into());
    }
    let _run_lock = campaign_lock(&out)?;
    let ledger = Ledger::new(&out.join("ledger"), true);
    let identity = json!({"schema":CAMPAIGN_SCHEMA,"requests_hash":requests_hash.clone(),
        "keys":prepared.iter().map(|(k,_)| assist::digest(k)).collect::<assist::Result<Vec<_>>>()?,"allowance_nano_usd":cap,
        "stage2":stage2,"budget_bounded":budget_bounded,"answer_failure_policy":stage2.then_some(answer_limits),
        "executor_call_cap_seconds":120,"prompt_epoch":stage2.then_some("g12-pilot/4"),
        "prompt_policy":stage2.then_some("assist-openrouter-task-evidence/4")});
    ledger.bind_campaign_with_settlement(identity, resume, settle_unknown)?;
    let mut smoke = if resume {
        assist::decode::<Value>(&assist::read_bytes(
            &out.join("smoke.json"),
            32 * 1024 * 1024,
        )?)?
    } else {
        json!({"roots":count,"root_outcomes":rows.iter().enumerate().map(|(i,r)| json!({"index":i,"root":r.root,"code":"not_run"})).collect::<Vec<_>>(),
            "allowance_nano_usd":cap,"budget_bounded":budget_bounded,"campaign_seconds":campaign_duration(stage2).as_secs(),
            "dispatch_failed":false,"answer_failure_policy":stage2.then_some(answer_limits),"qualified":false})
    };
    smoke
        .as_object_mut()
        .ok_or("invalid smoke outcomes")?
        .remove("refusal");
    let settled = if resume {
        if settle_unknown {
            settle_resume_unknown(&rows, &ledger, &smoke)?;
        }
        let settled = resume_roots(&rows, &ledger, &smoke)?;
        attach_receipts(&out, &ledger, &mut smoke)?;
        export_reconciliation(&out, &ledger, &mut smoke)?;
        settled
    } else {
        BTreeSet::new()
    };
    let prior = smoke["root_outcomes"]
        .as_array()
        .ok_or("invalid outcomes")?
        .clone();
    assist::write(&out.join("smoke.json"), &smoke)?;
    let auth = Authorization {
        enabled: true,
        scopes: vec![Scope {
            id: "smoke".into(),
            caps: Caps {
                total: if stage2 { 1000 } else { 10 },
                providers: BTreeMap::from([("openrouter".into(), if stage2 { 1000 } else { 10 })]),
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
    let (outcomes, failed) = continue_roots(
        &rows,
        campaign_deadline,
        budget_bounded,
        stage2.then_some(answer_limits),
        &prior,
        &settled,
        Instant::now,
        |index, deadline| {
            ledger
                .begin_campaign_root(index, prepared[index].0.payload_hash.clone())
                .map_err(|_| assist::Error::Storage)?;
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
        |prefix| {
            for (i, outcome) in prefix.iter().enumerate() {
                smoke["root_outcomes"][i] = outcome.clone();
            }
            attach_receipts(&out, &ledger, &mut smoke)?;
            export_reconciliation(&out, &ledger, &mut smoke).map_err(|_| assist::Error::Storage)
        },
    );
    smoke["root_outcomes"] = json!(outcomes);
    smoke["dispatch_failed"] = json!(failed);
    attach_receipts(&out, &ledger, &mut smoke)?;
    export_reconciliation(&out, &ledger, &mut smoke)?;
    if failed {
        if let Some(code) = smoke["root_outcomes"].as_array().and_then(|outcomes| {
            outcomes.iter().rev().find_map(|o| {
                let code = o["code"].as_str()?;
                (o["failure_class"] == "campaign_failure")
                    .then(|| code.replace([' ', '-', ';', ':'], "_").to_ascii_lowercase())
            })
        }) {
            // Codes originate from typed local execution errors, never provider text.
            smoke["refusal"] = json!({"code":code,"reason":code});
            assist::write(&out.join("smoke.json"), &smoke)?;
        }
        return Err("OpenRouter smoke failed; inspect sanitized campaign receipts".into());
    }
    Ok(())
}
fn refusal_code(error: &(dyn std::error::Error + 'static)) -> String {
    if error.downcast_ref::<std::num::ParseIntError>().is_some() {
        return "smoke_argument_integer_invalid".into();
    }
    if let Some(error) = error.downcast_ref::<std::io::Error>() {
        return match error.kind() {
            std::io::ErrorKind::NotFound => "smoke_local_file_missing",
            std::io::ErrorKind::PermissionDenied => "smoke_local_permission_denied",
            std::io::ErrorKind::AlreadyExists => "smoke_output_already_exists",
            _ => "smoke_local_io_unavailable",
        }
        .into();
    }
    if error.downcast_ref::<serde_json::Error>().is_some() {
        return "smoke_json_invalid".into();
    }
    if error.downcast_ref::<saccade_core::Error>().is_some() {
        return "smoke_root_policy_invalid_or_unavailable".into();
    }
    let reason = if let Some(error) = error.downcast_ref::<assist::Error>() {
        error.code()
    } else {
        let text = error.to_string();
        if text.starts_with("budget ledger:") {
            return "smoke_ledger_unavailable".into();
        }
        return match text.as_str() {
            "openrouter_credentials_unavailable"
            | "openrouter_ceiling_unavailable"
            | "openrouter_allowance_exceeds_ceiling"
            | "openrouter_campaign_not_fresh"
            | "openrouter_allowance_changed"
            | "openrouter_reconciliation_storage_unavailable"
            | "openrouter_reconciliation_failed"
            | "openrouter_generation_not_ready"
            | "openrouter_generation_unavailable"
            | "openrouter_generation_invalid"
            | "paid run requires development scorer proof corpus"
            | "paid run requires frozen development scorer proof revision"
            | "offline scorer proof failed; paid run refused"
            | "paid stage2 run requires task-evidence policy epoch 4"
            | "campaign already running"
            | "cannot read user configuration"
            | "invalid user configuration"
            | "invalid user root policy"
            | "custom providers require a distinct ID, HTTPS endpoint and dedicated credentials"
            | "pacing requires provider or provider/model keys and positive limits"
            | "pricing requires provider/model and nonnegative finite USD per million token rates"
            | "egress_denied: incomplete source provenance"
            | "egress_denied: unknown source root"
            | "egress_denied: unavailable source root"
            | "egress_denied: unavailable policy root"
            | "egress_denied: source root denies export"
            | "egress_denied: unclassified or denied root"
            | "OpenRouter smoke failed; inspect sanitized campaign receipts"
            | "answer safety options require stage2"
            | "campaign lock symlink"
            | "campaign_plan_or_policy_changed"
            | "campaign_spending_stopped"
            | "cap must be positive and at most campaign parent 30 USD"
            | "cap precision exceeds nanodollars"
            | "duplicate settlement flag"
            | "invalid answer safety percent"
            | "invalid answer safety sample limit"
            | "invalid campaign directory"
            | "invalid outcomes"
            | "invalid resume outcomes"
            | "invalid reviewed smoke topology or unsupported batch arm"
            | "invalid smoke arguments"
            | "invalid smoke outcomes"
            | "invalid unknown cost settlement"
            | "missing argument value"
            | "missing campaign receipt"
            | "openrouter_generation_cost_mismatch"
            | "openrouter_generation_identity_mismatch"
            | "openrouter_generation_missing"
            | "openrouter_reconciliation_mismatch"
            | "reconcile-only accepts only an existing output directory"
            | "required smoke argument missing"
            | "resume and out are exclusive"
            | "resume missing settled receipt"
            | "resume receipt identity changed"
            | "resume receipt lacks root binding"
            | "resume receipt payload changed"
            | "resume receipt root changed"
            | "resume requires matched incomplete receipt or settled answer artifacts"
            | "resume requires reconciled incomplete cost"
            | "resume topology changed"
            | "settlement requires resume execution"
            | "smoke_reservation_overflow"
            | "smoke_reservations_exceed_cap"
            | "spend_cap_above_25_requires_explicit_flag"
            | "stage2 requires 1 through 1000 roots"
            | "smoke requires 1 through 10 roots"
            | "stage2 cap exceeds 5 USD" => {
                text.replace([' ', '-', ';', ':'], "_").to_ascii_lowercase()
            }
            _ => "smoke_preflight_invalid_or_unavailable".into(),
        };
    };
    reason
        .replace([' ', '-', ';', ':'], "_")
        .to_ascii_lowercase()
}
fn run_reported(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let result = run_with(args.clone());
    if let Err(error) = &result {
        let code = refusal_code(error.as_ref());
        eprintln!("OpenRouter smoke refusal: {code}");
        if args
            .iter()
            .any(|a| a == "--preflight-only" || a == "--validate-only")
        {
            return result;
        }
        if let Some(pair) = args.windows(2).find(|p| p[0] == "--out") {
            let out = Path::new(&pair[1]);
            if !out.exists() {
                std::fs::create_dir(out)?;
            }
        }
        if let Some(out) = args
            .windows(2)
            .find(|p| p[0] == "--resume" || p[0] == "--out" || p[0] == "--reconcile-only")
            .map(|p| Path::new(&p[1]))
            && out.is_dir()
            && let Ok(_lock) = campaign_lock(out)
        {
            let path = out.join("smoke.json");
            let existing = assist::read_bytes(&path, 32 * 1024 * 1024)
                .and_then(|b| assist::decode::<Value>(&b));
            if let Ok(mut smoke) = existing
                && smoke.is_object()
            {
                if error.to_string()
                    == "OpenRouter smoke failed; inspect sanitized campaign receipts"
                    && let Some(specific) = smoke["refusal"]["code"].as_str()
                    && specific.len() <= 128
                    && specific
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b == b'_')
                {
                    eprintln!("OpenRouter smoke refusal: {specific}");
                } else {
                    smoke["refusal"] = json!({"code":code,"reason":code});
                }
                smoke["qualified"] = json!(false);
                assist::write(&path, &smoke)?;
            } else if !path.exists() {
                assist::write(
                    &path,
                    &json!({"refusal":{"code":code,"reason":code},"qualified":false}),
                )?;
            }
        }
    }
    result
}
fn scorer_proof_inputs(
    corpus: Option<std::ffi::OsString>,
    revision: Option<std::ffi::OsString>,
) -> Result<(std::ffi::OsString, std::ffi::OsString), Box<dyn std::error::Error>> {
    let corpus = corpus
        .filter(|s| !s.is_empty())
        .ok_or("paid run requires development scorer proof corpus")?;
    let revision = revision
        .filter(|s| !s.is_empty())
        .ok_or("paid run requires frozen development scorer proof revision")?;
    Ok((corpus, revision))
}

// Every paid entry point, including direct binary invocation, proves the current
// scorer offline before opening credentials or creating a campaign directory.
fn require_scorer_selftest() -> Result<(), Box<dyn std::error::Error>> {
    let (corpus, revision) = scorer_proof_inputs(
        std::env::var_os("SACCADE_SCORER_DEV_CORPUS"),
        std::env::var_os("SACCADE_SCORER_SOURCE_REVISION"),
    )?;
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/assist/scorer_selftest.py");
    let status = std::process::Command::new("python3")
        .arg(script)
        .arg("--corpus")
        .arg(corpus)
        .arg("--source-revision")
        .arg(revision)
        .status()?;
    if !status.success() {
        return Err("offline scorer proof failed; paid run refused".into());
    }
    Ok(())
}

fn main() {
    if run_reported(std::env::args().skip(1).collect()).is_err() {
        std::process::exit(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paid_scorer_gate_requires_both_proof_inputs_without_credentials() {
        assert!(scorer_proof_inputs(None, Some("fixture".into())).is_err());
        assert!(scorer_proof_inputs(Some("fixture".into()), None).is_err());
        assert!(scorer_proof_inputs(Some("".into()), Some("fixture".into())).is_err());
        assert!(scorer_proof_inputs(Some("fixture".into()), Some("".into())).is_err());
    }

    #[test]
    fn g12_resume_genuine_cost_or_identity_mismatch_blocks_settlement() {
        use saccade_core::budget_ledger::MoneyReceipt;
        for drift in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(&temp.path().join("ledger"), true);
            let rows = vec![Row {
                root: "root".into(),
                model: "fixture".into(),
                revision: "absent".into(),
                payload: json!({}),
            }];
            ledger
                .bind_campaign(json!({"plan":"fixture"}), false)
                .unwrap();
            let hash = Digest::of_bytes(b"{}");
            ledger.begin_campaign_root(0, hash.clone()).unwrap();
            ledger.reserve_money(&[MoneyScope { id:"smoke".into(),cap_nano_usd:100 }],MoneyReceipt {
                id:"attempt".into(),request_hash:hash,scopes:vec!["smoke".into()],reserved_nano_usd:100,
                actual_nano_usd:None,outcome:"reserved".into(),usage:json!({"openrouter_dispatched":true,"requested_identity":{"model":"fixture","revision":"absent"}})
            }).unwrap();
            ledger
                .finish_money(
                    "attempt",
                    Some(10),
                    json!({"generation_id":"gen-fixture"}),
                    false,
                )
                .unwrap();
            ledger
                .record_openrouter_reconciliation(
                    "attempt",
                    Ok(openrouter::Generation {
                        cost_nano_usd: if drift { 10 } else { 90 },
                        response_hash: Digest::of_bytes(b"generation"),
                        model: if drift { "other" } else { "fixture" }.into(),
                        provider_name: "fixture".into(),
                    }),
                    1,
                    0,
                    1,
                )
                .unwrap();
            let smoke = json!({"root_outcomes":[{"index":0,"root":"root","code":"assist_provider_execution_incomplete"}]});
            let before = std::fs::read(temp.path().join("ledger/campaign.json")).unwrap();
            assert!(
                ledger
                    .bind_campaign_with_settlement(json!({"plan":"fixture"}), true, true)
                    .is_err()
            );
            assert!(settle_resume_unknown(&rows, &ledger, &smoke).is_err());
            assert!(resume_roots(&rows, &ledger, &smoke).is_err());
            assert_eq!(
                before,
                std::fs::read(temp.path().join("ledger/campaign.json")).unwrap()
            );
        }
    }
    #[test]
    fn g12_preflight_known_failures_are_sanitized_and_offline_failures_write_nothing() {
        for reason in [
            "paid run requires development scorer proof corpus",
            "paid run requires frozen development scorer proof revision",
            "offline scorer proof failed; paid run refused",
            "paid stage2 run requires task-evidence policy epoch 4",
            "egress_denied: source root denies export",
            "invalid user configuration",
        ] {
            let error: Box<dyn std::error::Error> = reason.into();
            assert_ne!(
                refusal_code(error.as_ref()),
                "smoke_preflight_invalid_or_unavailable"
            );
        }
        let error: Box<dyn std::error::Error> = "untrusted secret provider body".into();
        assert_eq!(
            refusal_code(error.as_ref()),
            "smoke_preflight_invalid_or_unavailable"
        );
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path().join("untouched");
        assert!(
            run_reported(vec![
                "--preflight-only".into(),
                "--out".into(),
                out.to_str().unwrap().into()
            ])
            .is_err()
        );
        assert!(!out.exists());
        assert_eq!(
            refusal_code(&"bad".parse::<usize>().unwrap_err()),
            "smoke_argument_integer_invalid"
        );
    }
    #[test]
    fn g12_resume_refusal_is_specific_persisted_and_sanitized() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("smoke.json");
        assist::write(&path, &json!({"root_outcomes":[],"marker":"preserved"})).unwrap();
        let error = run_reported(vec![
            "--resume".into(),
            temp.path().to_str().unwrap().into(),
            "--untrusted-secret".into(),
        ])
        .unwrap_err();
        assert_eq!(refusal_code(error.as_ref()), "invalid_smoke_arguments");
        let smoke: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(smoke["refusal"]["code"], "invalid_smoke_arguments");
        assert_eq!(smoke["marker"], "preserved");
        assert_eq!(
            refusal_code(&std::io::Error::other("secret provider body")),
            "smoke_local_io_unavailable"
        );
        assert_eq!(
            refusal_code(&assist::Error::Policy(
                "openrouter_allowance_exceeds_ceiling"
            )),
            "openrouter_allowance_exceeds_ceiling"
        );
        let error: Box<dyn std::error::Error> = "campaign_spending_stopped".into();
        assert_eq!(refusal_code(error.as_ref()), "campaign_spending_stopped");
    }
    #[test]
    fn g12_operator_settlement_retains_charge_skips_failed_root_and_survives_reopen() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();
        let ledger = Ledger::new(&out.join("ledger"), true);
        let rows: Vec<_> = (0..3)
            .map(|i| Row {
                root: format!("root-{i}"),
                model: "fixture".into(),
                revision: "absent".into(),
                payload: json!({"index":i}),
            })
            .collect();
        ledger
            .bind_campaign(json!({"plan":"fixture"}), false)
            .unwrap();
        for i in 0..2 {
            let hash = Digest::of_bytes(&serde_json::to_vec(&rows[i].payload).unwrap());
            ledger.begin_campaign_root(i, hash.clone()).unwrap();
            ledger
                .reserve_money(
                    &[MoneyScope {
                        id: "smoke".into(),
                        cap_nano_usd: 250,
                    }],
                    MoneyReceipt {
                        id: format!("attempt-{i}"),
                        request_hash: hash,
                        scopes: vec!["smoke".into()],
                        reserved_nano_usd: 100,
                        actual_nano_usd: None,
                        outcome: "reserved".into(),
                        usage: json!({"openrouter_dispatched":true}),
                    },
                )
                .unwrap();
        }
        ledger
            .finish_money(
                "attempt-0",
                None,
                json!({"transport_failure":"timeout"}),
                false,
            )
            .unwrap();
        ledger
            .record_openrouter_reconciliation(
                "attempt-0",
                Err("openrouter_generation_missing"),
                0,
                0,
                1,
            )
            .unwrap();
        assert_eq!(
            ledger
                .bind_campaign(json!({"plan":"fixture"}), true)
                .unwrap_err(),
            "openrouter_generation_missing"
        );
        assert!(
            ledger
                .bind_campaign_with_settlement(json!({"plan":"changed"}), true, true)
                .is_err()
        );
        ledger
            .bind_campaign_with_settlement(json!({"plan":"fixture"}), true, true)
            .unwrap();
        // Second receipt simulates a crash between dispatch/reservation and finish.
        let mut smoke = json!({"root_outcomes":rows.iter().enumerate().map(|(i,r)| json!({"index":i,"root":r.root,
            "code":"assist_provider_execution_incomplete"})).collect::<Vec<_>>()});
        attach_receipts(out, &ledger, &mut smoke).unwrap();
        assert!(resume_roots(&rows, &ledger, &smoke).is_err());
        let mut changed = smoke.clone();
        changed["root_outcomes"][0]["root"] = json!("wrong");
        assert!(settle_resume_unknown(&rows, &ledger, &changed).is_err());
        assert!(
            ledger.money_receipts().unwrap()[0]
                .actual_nano_usd
                .is_none()
        );
        let before: Value =
            serde_json::from_slice(&std::fs::read(out.join("ledger/campaign.json")).unwrap())
                .unwrap();
        settle_resume_unknown(&rows, &ledger, &smoke).unwrap();
        let receipts = ledger.money_receipts().unwrap();
        assert_eq!(
            receipts
                .iter()
                .map(|r| r.actual_nano_usd.unwrap())
                .sum::<u64>(),
            200
        );
        assert!(
            receipts
                .iter()
                .all(|r| r.outcome == "settled_conservatively"
                    && r.usage["qualification_eligible"] == false)
        );
        settle_resume_unknown(&rows, &ledger, &smoke).unwrap();
        assert_eq!(
            serde_json::to_value(ledger.money_receipts().unwrap()).unwrap(),
            serde_json::to_value(&receipts).unwrap()
        );
        ledger
            .record_openrouter_reconciliation(
                "attempt-0",
                Err("openrouter_generation_missing"),
                0,
                0,
                1,
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(ledger.money_receipts().unwrap()).unwrap(),
            serde_json::to_value(&receipts).unwrap()
        );
        let after: Value =
            serde_json::from_slice(&std::fs::read(out.join("ledger/campaign.json")).unwrap())
                .unwrap();
        assert_eq!(before["money"]["counters"], after["money"]["counters"]);
        assert_eq!(
            openrouter::reconciliation_status(&ledger)
                .unwrap()
                .settled_conservatively,
            2
        );
        assert_eq!(
            openrouter::reconciliation_status(&ledger).unwrap().state,
            "settled_conservatively"
        );
        // Recover even if the process crashed before exporting the settlement.
        let reopened = Ledger::new(&out.join("ledger"), true);
        assert_eq!(
            resume_roots(&rows, &reopened, &smoke).unwrap(),
            BTreeSet::from([0, 1])
        );
        export_reconciliation(out, &reopened, &mut smoke).unwrap();
        assert_eq!(
            smoke["root_outcomes"][0]["code"],
            "not_run_transport_failure"
        );
        assert_eq!(
            smoke["root_outcomes"][0]["previous_code"],
            "assist_provider_execution_incomplete"
        );
        let mut called = Vec::new();
        let (_, failed) = continue_roots(
            &rows,
            Instant::now() + Duration::from_secs(300),
            false,
            None,
            smoke["root_outcomes"].as_array().unwrap(),
            &resume_roots(&rows, &reopened, &smoke).unwrap(),
            Instant::now,
            |i, _| {
                called.push(i);
                Ok(CallOutcome::Completed)
            },
            |_| Ok(()),
        );
        assert!(!failed);
        assert_eq!(called, vec![2]);
        let hash = Digest::of_bytes(&serde_json::to_vec(&rows[2].payload).unwrap());
        reopened.begin_campaign_root(2, hash.clone()).unwrap();
        assert!(
            reopened
                .reserve_money(
                    &[MoneyScope {
                        id: "smoke".into(),
                        cap_nano_usd: 250
                    }],
                    MoneyReceipt {
                        id: "next".into(),
                        request_hash: hash,
                        scopes: vec!["smoke".into()],
                        reserved_nano_usd: 100,
                        actual_nano_usd: None,
                        outcome: "reserved".into(),
                        usage: json!({})
                    }
                )
                .is_err()
        );
        reopened.stop_spending().unwrap();
        assert!(reopened.settle_unknown_at_reservation().is_err());
    }
    #[test]
    fn g12_settlement_flag_requires_resume_before_policy_or_credentials() {
        for args in [
            vec!["--settle-unknown-at-reservation"],
            vec![
                "--settle-unknown-at-reservation",
                "--resume",
                "absent",
                "--validate-only",
            ],
            vec![
                "--settle-unknown-at-reservation",
                "--reconcile-only",
                "absent",
            ],
        ] {
            assert_eq!(
                run_with(args.into_iter().map(String::from))
                    .unwrap_err()
                    .to_string(),
                "settlement requires resume execution"
            );
        }
    }
    #[test]
    fn g12_resume_skips_settled_roots_blocks_unknown_and_preserves_attempt_mapping() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path();
        let ledger = Ledger::new(&out.join("ledger"), true);
        let rows: Vec<_> = (0..5)
            .map(|i| Row {
                root: format!("root-{i}"),
                model: "fixture".into(),
                revision: "absent".into(),
                payload: json!({"index":i}),
            })
            .collect();
        ledger
            .bind_campaign(json!({"plan":"fixture","policy":"epoch-2"}), false)
            .unwrap();
        let scopes = [MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: 1000,
        }];
        let reserve = |i: usize, id: &str| {
            let hash = Digest::of_bytes(&serde_json::to_vec(&rows[i].payload).unwrap());
            ledger.begin_campaign_root(i, hash.clone()).unwrap();
            ledger.reserve_money(&scopes,MoneyReceipt{id:id.into(),request_hash:hash,scopes:vec!["smoke".into()],
                reserved_nano_usd:100,actual_nano_usd:None,outcome:"reserved".into(),
                usage:json!({"openrouter_dispatched":true,"requested_identity":{"model":"fixture","revision":"absent"}})}).unwrap();
        };
        for i in 0..4 {
            let id = format!("attempt-{i}");
            reserve(i, &id);
            ledger.finish_money(&id,if i==3 {None} else {Some(if i==2 {0} else {10})},
                json!({"zero_cost_refused":i==2,"transport_failure":if i==3 {Some("timeout")} else {None}}),i<2).unwrap();
        }
        let mut smoke = json!({"root_outcomes":rows.iter().enumerate().map(|(i,r)| json!({"index":i,"root":r.root,
            "code":(["completed","invalid_answer","openrouter_http_zero_cost_refused","assist_provider_execution_incomplete","skipped_after_failure"][i]),
            "answer_reason":if i==1 {Some("geometry_bounds")} else {None},"execution_id":if i<4 {Some(format!("attempt-{i}"))} else {None}})).collect::<Vec<_>>()});
        assert!(resume_roots(&rows, &ledger, &smoke).is_err());
        ledger
            .record_openrouter_reconciliation(
                "attempt-3",
                Ok(openrouter::Generation {
                    cost_nano_usd: 20,
                    response_hash: Digest::of_bytes(b"proof"),
                    model: "fixture".into(),
                    provider_name: "fixture".into(),
                }),
                1,
                0,
                1,
            )
            .unwrap();
        let settled = resume_roots(&rows, &ledger, &smoke).unwrap();
        assert_eq!(settled, BTreeSet::from([0, 1, 2]));
        let prior = smoke["root_outcomes"].as_array().unwrap().clone();
        let mut called = Vec::new();
        let mut checkpoints = 0;
        let (outcomes, failed) = continue_roots(
            &rows,
            Instant::now() + Duration::from_secs(300),
            false,
            Some(AnswerLimits::default()),
            &prior,
            &settled,
            Instant::now,
            |i, _| {
                called.push(i);
                Ok(CallOutcome::Completed)
            },
            |_| {
                checkpoints += 1;
                Ok(())
            },
        );
        assert!(!failed);
        assert_eq!(called, vec![3, 4]);
        assert_eq!(checkpoints, 5);
        assert_eq!(outcomes[1]["answer_reason"], "geometry_bounds");
        reserve(3, "retry-3");
        ledger
            .finish_money("retry-3", Some(30), json!({}), true)
            .unwrap();
        reserve(4, "attempt-4");
        ledger
            .finish_money("attempt-4", Some(40), json!({}), true)
            .unwrap();
        smoke["root_outcomes"] = json!(outcomes);
        for i in [0, 1, 3, 4] {
            assist::write(
                &out.join(format!("receipt-{i}.json")),
                &json!({"provenance":"preserved"}),
            )
            .unwrap();
        }
        attach_receipts(out, &ledger, &mut smoke).unwrap();
        export_reconciliation(out, &ledger, &mut smoke).unwrap();
        assert_eq!(smoke["root_outcomes"][3]["execution_id"], "retry-3");
        assert_eq!(smoke["root_outcomes"][4]["execution_id"], "attempt-4");
        assert_eq!(smoke["root_outcomes"][1]["qualification_eligible"], false);
        assert!(out.join("money-attempt-3.json").exists());
        assert!(out.join("money-retry-3.json").exists());
        assert_eq!(
            ledger
                .money_receipts()
                .unwrap()
                .iter()
                .map(|r| r.actual_nano_usd.unwrap())
                .sum::<u64>(),
            110
        );
        assert_eq!(resume_roots(&rows, &ledger, &smoke).unwrap().len(), 5);
        let mut changed = smoke.clone();
        changed["root_outcomes"][0]["root"] = json!("changed");
        assert!(resume_roots(&rows, &ledger, &changed).is_err());
        assert_eq!(
            smoke["root_outcomes"][2]["money_outcome"],
            "zero_cost_refused"
        );
    }
    #[test]
    fn g12_resume_keeps_answer_safety_history_and_checkpoint_failures_stop() {
        let rows: Vec<_> = (0..8)
            .map(|i| Row {
                root: format!("root-{i}"),
                model: "fixture".into(),
                revision: "absent".into(),
                payload: json!({}),
            })
            .collect();
        let prior:Vec<_>=rows.iter().enumerate().map(|(i,r)|json!({"index":i,"root":r.root,"code":"invalid_answer","answer_reason":"geometry_bounds"})).collect();
        let mut called = Vec::new();
        let (outcomes, failed) = continue_roots(
            &rows,
            Instant::now() + Duration::from_secs(300),
            false,
            Some(AnswerLimits::default()),
            &prior,
            &BTreeSet::from([0, 1, 2, 3, 4, 5]),
            Instant::now,
            |i, _| {
                called.push(i);
                Ok(CallOutcome::Completed)
            },
            |_| Ok(()),
        );
        assert!(failed);
        assert!(called.is_empty());
        assert_eq!(outcomes[6]["code"], "not_run_answer_safety_valve");
        let mut calls = 0;
        let (outcomes, failed) = continue_roots(
            &rows,
            Instant::now() + Duration::from_secs(300),
            false,
            None,
            &[],
            &BTreeSet::new(),
            Instant::now,
            |_, _| {
                calls += 1;
                Ok(CallOutcome::Completed)
            },
            |_| Err(assist::Error::Storage),
        );
        assert!(failed);
        assert_eq!(calls, 1);
        assert_eq!(outcomes[1]["code"], "skipped_after_failure");
    }
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
    fn epoch3_exclusion_citations_preserve_closed_same_slot_protocol() {
        let hash = Digest::of_bytes(b"invented-epoch3");
        let row = Row {
            root: "synthetic".into(),
            model: "google/gemini-3.8-flash".into(),
            revision: "absent".into(),
            payload: json!({"messages":[{}, {"content":[{"text":json!({"request_hash":hash,
                "prompt_epoch":"g12-pilot/4", "views":[{"slot":"P1","regions":[
                    {"id":"P1:R0"},{"id":"P1:R1","exclusion_id":"synthetic-exclusion"}]}]}).to_string()}]}]}),
        };
        let answer = json!({"request_hash":hash,"outcome":"observed","observations":[{
            "slot":"P1","kind":"appearance","statement":"appearance:changed",
            "geometry":{"type":"box","pixels":[0.0,0.0,1.0,1.0]},"visibility":"visible",
            "evidence_refs":["P1:R0","P1:R1"],"uncertainty":0.0}]});
        let response = |a: &Value| {
            serde_json::to_vec(&json!({"id":"gen-synthetic",
            "model":row.model,"provider":"fixture","choices":[{"finish_reason":"stop","message":{"content":a.to_string()}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2,"cost":0.001}})).unwrap()
        };
        stage2_answer(&row, &response(&answer)).unwrap();
        for citation in ["synthetic-exclusion", "P2:R1", "P1:R2"] {
            let mut wrong = answer.clone();
            wrong["observations"][0]["evidence_refs"] = json!([citation]);
            assert!(stage2_answer(&row, &response(&wrong)).is_err());
        }
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
    fn g12_resume_over_hint_settled_answer_continues_at_next_root() {
        use saccade_core::budget_ledger::MoneyReceipt;
        let body = include_bytes!("../tests/fixtures/assist-openrouter/call7-reasoning-hint.json");
        for valid in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let out = temp.path();
            let ledger = Ledger::new(&out.join("ledger"), true);
            let rows = [cross_citation_row("call7"), cross_citation_row("next")];
            let identity = json!({"schema":CAMPAIGN_SCHEMA,"plan":"epoch-2-fixture"});
            ledger.bind_campaign(identity.clone(), false).unwrap();
            let hash = Digest::of_bytes(&serde_json::to_vec(&rows[0].payload).unwrap());
            ledger.begin_campaign_root(0, hash.clone()).unwrap();
            ledger
                .reserve_money(
                    &[MoneyScope {
                        id: "smoke".into(),
                        cap_nano_usd: 5_000_000_000,
                    }],
                    MoneyReceipt {
                        id: "call7".into(),
                        request_hash: hash,
                        scopes: vec!["smoke".into()],
                        reserved_nano_usd: 26_055_750,
                        actual_nano_usd: None,
                        outcome: "reserved".into(),
                        usage: json!({}),
                    },
                )
                .unwrap();
            ledger.finish_money("call7", Some(10_862_250), json!({"bound_breach":false,
                "provider_reasoning_over_hint":true,"reasoning_hint":{"requested_tokens":1024,"observed_tokens":1638},
                "input_bound":14261,"output_bound":4096}), true).unwrap();
            let mut response: Value = assist::decode(body).unwrap();
            if !valid {
                response["choices"][0]["message"]["content"] = json!("{}");
            }
            let result = record_response(
                &rows[0],
                &serde_json::to_vec(&response).unwrap(),
                &json!({"execution_id":"call7"}),
                out,
                0,
                true,
            )
            .unwrap();
            assert_eq!(
                result,
                if valid {
                    CallOutcome::Completed
                } else {
                    CallOutcome::InvalidAnswer("closed_schema")
                }
            );
            let mut smoke = json!({"root_outcomes":[{"index":0,"root":"call7","code":if valid {"completed"} else {"invalid_answer"},
                "answer_reason":if valid {Value::Null} else {json!("closed_schema")}},
                {"index":1,"root":"next","code":"not_run"}]});
            attach_receipts(out, &ledger, &mut smoke).unwrap();
            export_reconciliation(out, &ledger, &mut smoke).unwrap();
            assert_eq!(
                smoke["root_outcomes"][0]["provider_reasoning_over_hint"],
                true
            );
            assert_eq!(
                smoke["root_outcomes"][0]["reasoning_hint"],
                json!({"requested_tokens":1024,"observed_tokens":1638})
            );
            ledger.bind_campaign(identity, true).unwrap();
            assert!(
                ledger
                    .bind_campaign(
                        json!({"schema":"saccade-g12-campaign/2","plan":"epoch-2-fixture"}),
                        true
                    )
                    .is_err()
            );
            let settled = resume_roots(&rows, &ledger, &smoke).unwrap();
            assert_eq!(settled, BTreeSet::from([0]));
            let mut calls = Vec::new();
            let (outcomes, failed) = continue_roots(
                &rows,
                Instant::now() + CALL_LIMIT,
                false,
                Some(AnswerLimits::default()),
                smoke["root_outcomes"].as_array().unwrap(),
                &settled,
                Instant::now,
                |i, _| {
                    calls.push(i);
                    Ok(CallOutcome::Completed)
                },
                |_| Ok(()),
            );
            assert!(!failed);
            assert_eq!(calls, vec![1]);
            assert_eq!(
                outcomes[0]["code"],
                if valid { "completed" } else { "invalid_answer" }
            );
            assert_eq!(
                ledger.money_receipts().unwrap()[0].actual_nano_usd,
                Some(10_862_250)
            );
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
