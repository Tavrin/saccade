//! Monetary reservations under the existing ledger lock. Crashes retain charges.
#[cfg(feature = "assist")]
use super::Ledger;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Independent monetary ceiling, expressed in integer nanodollars.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(feature = "assist")]
pub struct MoneyScope {
    /// Entry, run, repository/day or evaluation/epoch identity.
    pub id: String,
    /// Allowance can only decrease on reopening.
    pub cap_nano_usd: u64,
}
/// A durable pre-dispatch charge and optional reconciled receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoneyReceipt {
    /// Locally generated dispatch identity.
    pub id: String,
    /// Exact request body, without credentials or body data.
    pub request_hash: crate::evidence::canonical::Digest,
    /// Scopes charged atomically.
    pub scopes: Vec<String>,
    /// Conservative allowance consumed before dispatch.
    pub reserved_nano_usd: u64,
    /// Known cost; null retains the entire allowance, never zero.
    pub actual_nano_usd: Option<u64>,
    /// reserved, completed, incomplete or cost_limit_exceeded.
    pub outcome: String,
    /// Full token accounting, even when an answer was malformed.
    pub usage: UsageValue,
}
/// Usage metadata stored without raw provider text.
pub type UsageValue = serde_json::Value;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct MoneyState {
    counters: BTreeMap<String, (u64, u64)>,
    #[serde(default)]
    stopped: bool,
    #[serde(default)]
    campaign_identity: Option<UsageValue>,
    #[serde(default)]
    campaign_root: Option<(usize, crate::evidence::canonical::Digest)>,
    receipts: Vec<MoneyReceipt>,
    #[serde(default)]
    openrouter: Option<UsageValue>,
    #[serde(default)]
    ceiling_events: Vec<UsageValue>,
}
#[cfg(feature = "assist")]
fn missing_generation(receipt: &MoneyReceipt) -> bool {
    receipt.actual_nano_usd.is_none()
        && receipt.outcome == "incomplete"
        && receipt.usage["generation_id"].is_null()
        && receipt.usage["reconciliation"]["state"] == "mismatch"
        && receipt.usage["reconciliation"]["reason"] == "openrouter_generation_missing"
        && receipt.usage["reconciliation"]["generation"].is_null()
}
#[cfg(feature = "assist")]
fn recoverable_missing_generation(state: &MoneyState) -> bool {
    state.receipts.iter().any(missing_generation)
        && state.ceiling_events.iter().all(|e| e["refusal"].is_null())
        && state.receipts.iter().all(|r| {
            r.outcome != "cost_limit_exceeded"
                && r.usage["bound_breach"] != true
                && r.usage["identity_error"].is_null()
                && (r.usage["reconciliation"]["state"] != "mismatch"
                    || r.outcome == "settled_conservatively"
                    || missing_generation(r))
        })
}
#[cfg(feature = "assist")]
impl Ledger {
    fn campaign(&self) -> Self {
        Self {
            dir: self.dir.clone(),
            namespace: "campaign".into(),
        }
    }
    /// Freeze the complete runner identity under the monetary lock. Legacy ledgers
    /// without this binding cannot be resumed or silently assigned a new policy.
    pub fn bind_campaign(&self, identity: UsageValue, resume: bool) -> Result<(), String> {
        self.bind_campaign_with_settlement(identity, resume, false)
    }
    /// Permit binding validation for an explicitly requested missing-generation settlement.
    /// Spending remains stopped until the atomic settlement succeeds.
    pub fn bind_campaign_with_settlement(
        &self,
        identity: UsageValue,
        resume: bool,
        settle: bool,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            if state.money.stopped
                && !(resume && settle && recoverable_missing_generation(&state.money))
            {
                let reason = state
                    .money
                    .receipts
                    .iter()
                    .find_map(|r| {
                        if r.usage["reconciliation"]["state"] != "mismatch" {
                            return None;
                        }
                        Some(match r.usage["reconciliation"]["reason"].as_str() {
                            Some("openrouter_generation_missing") => {
                                "openrouter_generation_missing"
                            }
                            Some("openrouter_generation_cost_mismatch") => {
                                "openrouter_generation_cost_mismatch"
                            }
                            Some("provider revision drift quarantined") => {
                                "openrouter_generation_identity_mismatch"
                            }
                            _ => "openrouter_reconciliation_mismatch",
                        })
                    })
                    .unwrap_or("campaign_spending_stopped");
                return Err(reason.into());
            }
            match &state.money.campaign_identity {
                Some(old) if resume && *old == identity => Ok(()),
                None if !resume && state.money.receipts.is_empty() => {
                    state.money.campaign_identity = Some(identity);
                    Ok(())
                }
                _ => Err("campaign_plan_or_policy_changed".into()),
            }
        })
    }
    /// Read the frozen identity without changing the campaign.
    pub fn campaign_identity(&self) -> Result<Option<UsageValue>, String> {
        self.campaign()
            .transaction(|state| Ok(state.money.campaign_identity.clone()))
    }
    /// Add durable root markers to a validated legacy Jev prefix, atomically.
    /// Every receipt must be supplied exactly once and retain its request hash.
    pub fn bind_legacy_jev_roots(
        &self,
        bindings: &[(String, usize, crate::evidence::canonical::Digest)],
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            if state
                .money
                .campaign_identity
                .as_ref()
                .is_none_or(|v| v["schema"] != "jev-local-campaign/1")
                || bindings.len() != state.money.receipts.len()
            {
                return Err("legacy Jev bindings rejected".into());
            }
            for (receipt, (id, index, hash)) in state.money.receipts.iter().zip(bindings) {
                if receipt.id != *id
                    || receipt.request_hash != *hash
                    || receipt.usage["campaign_root_index"]
                        .as_u64()
                        .is_some_and(|n| n != *index as u64)
                {
                    return Err("legacy Jev bindings rejected".into());
                }
            }
            for (receipt, (_, index, _)) in state.money.receipts.iter_mut().zip(bindings) {
                receipt.usage["campaign_root_index"] = serde_json::json!(index);
            }
            Ok(())
        })
    }
    /// Settle one unavailable root at its full reservation, retaining the charge.
    /// Proven pre-dispatch pacing failures also consume the full reservation in
    /// continuation mode. Identity, accounting and authorization failures cannot continue.
    pub fn settle_continuation_root(&self, index: usize) -> Result<bool, String> {
        self.campaign().transaction(|state| {
            if state.money.stopped {
                return Err("campaign_spending_stopped".into());
            }
            let Some(receipt) = state
                .money
                .receipts
                .iter_mut()
                .rev()
                .find(|r| r.usage["campaign_root_index"].as_u64() == Some(index as u64))
            else {
                return Ok(false);
            };
            if receipt.outcome == "settled_conservatively" {
                return Ok(true);
            }
            let class = receipt.usage["transport_failure"].as_str();
            let pacing = receipt.usage["not_dispatched"] == true
                && receipt.actual_nano_usd == Some(0)
                && (class == Some("client_deadline") || class.is_none());
            if !matches!(receipt.outcome.as_str(), "incomplete" | "reserved")
                || receipt.reserved_nano_usd == 0
                || receipt.usage["bound_breach"] == true
                || receipt.usage["continuation_eligible"] == false
                || !receipt.usage["identity_error"].is_null()
                || receipt.usage["reconciliation"]["state"] == "mismatch"
                || !(pacing
                    || (receipt.actual_nano_usd.is_none()
                        && receipt.usage["not_dispatched"] != true))
            {
                return Ok(false);
            }
            let previous = receipt.actual_nano_usd;
            let charge = previous.unwrap_or(receipt.reserved_nano_usd);
            if charge > receipt.reserved_nano_usd {
                return Err("invalid unknown cost settlement".into());
            }
            for scope in &receipt.scopes {
                let counter = state
                    .money
                    .counters
                    .get_mut(scope)
                    .ok_or("missing money counter")?;
                counter.1 = counter
                    .1
                    .checked_add(receipt.reserved_nano_usd - charge)
                    .ok_or("money accounting overflow")?;
            }
            if receipt.usage["transport_failure"].is_null() {
                receipt.usage["transport_failure"] = serde_json::json!("other");
                receipt.usage["transport_error_kind"] = serde_json::json!("legacy_unclassified");
            }
            receipt.usage["unknown_cost_settlement"] = serde_json::json!({
                "method":"operator_full_reservation", "conservative":true,"reconciled":false,
                "previous_actual_nano_usd":previous,"previous_charge_nano_usd":charge,
                "previous_outcome":receipt.outcome,"charge_nano_usd":receipt.reserved_nano_usd,
                "settled_ms":crate::budget_ledger::now_ms()});
            receipt.actual_nano_usd = Some(receipt.reserved_nano_usd);
            receipt.outcome = "settled_conservatively".into();
            receipt.usage["qualification_eligible"] = serde_json::json!(false);
            Ok(true)
        })
    }
    /// Bind the next root before reserving so a crash cannot lose receipt-to-root identity.
    pub fn begin_campaign_root(
        &self,
        index: usize,
        hash: crate::evidence::canonical::Digest,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            if state.money.campaign_identity.is_none() {
                return Err("campaign_identity_required".into());
            }
            state.money.campaign_root = Some((index, hash));
            Ok(())
        })
    }
    /// Durably stop all modes and epochs in this campaign after a demonstrated breach.
    pub fn stop_spending(&self) -> Result<(), String> {
        self.campaign().transaction(|state| {
            state.money.stopped = true;
            state.money.ceiling_events.push(
                serde_json::json!({"phase":"operator_stop","refusal":"campaign_spending_stopped"}),
            );
            Ok(())
        })
    }
    /// Reserve every monetary scope under the same lock as request accounting.
    /// Money is reserved first; a later request refusal conservatively retains it.
    pub fn reserve_money(
        &self,
        scopes: &[MoneyScope],
        mut receipt: MoneyReceipt,
    ) -> Result<(), String> {
        // Refuse silent allowance resets when upgrading an older namespace-local ledger.
        for mode in ["production", "evaluation"] {
            let path = self.dir.join(format!("{mode}.json"));
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err("legacy ledger symlink".into());
            }
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let legacy: super::State =
                        serde_json::from_slice(&bytes).map_err(|_| "invalid legacy ledger")?;
                    if !legacy.money.receipts.is_empty() {
                        return Err(
                            "legacy monetary receipts require reviewed campaign migration".into(),
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("legacy ledger unavailable".into()),
            }
        }
        self.campaign().transaction(|state| {
            if state.money.stopped {
                return Err("campaign_spending_stopped".into());
            }
            if let Some(verified) = &state.money.openrouter
                && scopes
                    .iter()
                    .map(|s| s.cap_nano_usd)
                    .min()
                    .is_none_or(|cap| cap > verified["allowance"].as_u64().unwrap_or(0))
            {
                return Err("openrouter_allowance_changed".into());
            }
            if scopes.is_empty()
                || receipt.id.is_empty()
                || receipt.reserved_nano_usd == 0
                || state.money.receipts.iter().any(|r| r.id == receipt.id)
                || receipt.scopes != scopes.iter().map(|s| s.id.clone()).collect::<Vec<_>>()
            {
                return Err("invalid monetary reservation".into());
            }
            let mut scopes = scopes.to_vec();
            if scopes.iter().any(|s| s.id == "campaign/assist") {
                return Err("campaign parent is ledger-owned".into());
            }
            scopes.push(MoneyScope {
                id: "campaign/assist".into(),
                cap_nano_usd: 30_000_000_000,
            });
            receipt.scopes.push("campaign/assist".into());
            let mut ids = std::collections::BTreeSet::new();
            for scope in &scopes {
                if scope.id.is_empty() || !ids.insert(&scope.id) {
                    return Err("invalid money scope".into());
                }
                let (old_cap, used) = state
                    .money
                    .counters
                    .get(&scope.id)
                    .copied()
                    .unwrap_or((scope.cap_nano_usd, 0));
                if used
                    .checked_add(receipt.reserved_nano_usd)
                    .is_none_or(|total| total > old_cap.min(scope.cap_nano_usd))
                {
                    return Err("money_budget_exhausted".into());
                }
            }
            for scope in &scopes {
                let counter = state
                    .money
                    .counters
                    .entry(scope.id.clone())
                    .or_insert((scope.cap_nano_usd, 0));
                counter.0 = counter.0.min(scope.cap_nano_usd);
                counter.1 += receipt.reserved_nano_usd;
            }
            if let Some((index, hash)) = &state.money.campaign_root {
                if receipt.request_hash != *hash {
                    return Err("campaign_root_hash_changed".into());
                }
                receipt.usage["campaign_root_index"] = serde_json::json!(index);
            }
            state.money.receipts.push(receipt);
            Ok(())
        })
    }
    /// Reconcile known usage once. Unknown/failed calls keep the full reservation.
    /// Overruns remain charged and permanently block subsequent work in exhausted scopes.
    pub fn finish_money(
        &self,
        id: &str,
        actual: Option<u64>,
        usage: UsageValue,
        complete: bool,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            let receipt = state
                .money
                .receipts
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or("unknown money reservation")?;
            if receipt.outcome != "reserved" {
                return Err("money receipt already final".into());
            }
            let jev_metadata = (receipt.usage["ceiling"] == crate::assist::jev::CEILING).then(|| receipt.usage.clone());
            let dispatched = receipt.usage["openrouter_dispatched"] == true;
            let requested_identity = receipt.usage["requested_identity"].clone();
            let image_table = receipt.usage.get("image_table").cloned();
            let root_index = receipt.usage.get("campaign_root_index").cloned();
            let zero_cost_refused = dispatched && actual == Some(0)
                && usage["zero_cost_refused"] == true;
            let charged = actual.unwrap_or(receipt.reserved_nano_usd);
            for scope in &receipt.scopes {
                let counter = state
                    .money
                    .counters
                    .get_mut(scope)
                    .ok_or("missing money counter")?;
                counter.1 = counter
                    .1
                    .saturating_sub(receipt.reserved_nano_usd)
                    .saturating_add(charged);
            }
            if charged > receipt.reserved_nano_usd || usage["bound_breach"] == true {
                state.money.stopped = true;
            }
            receipt.actual_nano_usd = actual;
            receipt.usage = usage;
            if let Some(metadata) = jev_metadata {
                for name in ["ceiling", "price_policy", "price_source", "price_date", "price_expires_ms", "billing_verified", "prepaid_no_refill_attested"] {
                    receipt.usage[name] = metadata[name].clone();
                }
                receipt.usage["reconciliation"] = serde_json::json!({"state":"unsupported","actual_debit":null});
            }
            if let Some(index) = root_index { receipt.usage["campaign_root_index"] = index; }
            if let Some(table) = image_table {
                receipt.usage["image_table"] = table;
            }
            if dispatched {
                receipt.usage["openrouter_dispatched"] = serde_json::json!(true);
                receipt.usage["requested_identity"] = requested_identity;
                receipt.usage["reconciliation"] = serde_json::json!({"state":if zero_cost_refused { "zero_cost_refused" } else { "pending" },"matches":false,"attempts":0,"attempted_ms":null});
                receipt.usage["qualification_eligible"] = serde_json::json!(false);
            }
            receipt.outcome = if receipt.usage["bound_breach"] == true {
                "usage_limit_exceeded"
            } else if charged > receipt.reserved_nano_usd {
                "cost_limit_exceeded"
            } else if zero_cost_refused {
                "zero_cost_refused"
            } else if complete {
                "completed"
            } else {
                "incomplete"
            }
            .into();
            if receipt.outcome == "incomplete" && receipt.usage["transport_failure"].is_null() {
                receipt.usage["transport_failure"] = serde_json::json!("other");
            }
            if receipt.usage["transport_failure"] == "other" && receipt.usage["transport_error_kind"].is_null() {
                receipt.usage["transport_error_kind"] = serde_json::json!("unknown_kind");
            }
            Ok(())
        })
    }
    /// Explicit operator settlement: retain every unknown charge at its reservation.
    /// This is not provider reconciliation and can never release allowance or qualify.
    /// The campaign runner must validate its frozen root bindings before calling.
    pub fn settle_unknown_at_reservation(&self) -> Result<(), String> {
        self.campaign().transaction(|state| {
            if state.money.stopped && !recoverable_missing_generation(&state.money) {
                return Err("campaign_spending_stopped".into());
            }
            for receipt in &state.money.receipts {
                if receipt.usage["reconciliation"]["state"] == "mismatch"
                    && receipt.outcome != "settled_conservatively"
                    && !missing_generation(receipt)
                {
                    return Err("openrouter_reconciliation_mismatch".into());
                }
                if receipt.actual_nano_usd.is_none()
                    && (!matches!(receipt.outcome.as_str(), "reserved" | "incomplete")
                        || receipt.reserved_nano_usd == 0
                        || receipt.usage["campaign_root_index"].as_u64().is_none())
                {
                    return Err("invalid unknown cost settlement".into());
                }
            }
            for receipt in &mut state.money.receipts {
                if receipt.actual_nano_usd.is_some() {
                    continue;
                }
                // Counters already include this entire reservation. Leave them intact.
                receipt.usage["unknown_cost_settlement"] = serde_json::json!({
                    "method":"operator_full_reservation", "conservative":true,
                    "reconciled":false, "previous_actual_nano_usd":null,
                    "previous_charge_nano_usd":receipt.reserved_nano_usd,
                    "previous_outcome":receipt.outcome,
                    "charge_nano_usd":receipt.reserved_nano_usd,
                    "settled_ms":crate::budget_ledger::now_ms()});
                receipt.actual_nano_usd = Some(receipt.reserved_nano_usd);
                receipt.outcome = "settled_conservatively".into();
                receipt.usage["qualification_eligible"] = serde_json::json!(false);
            }
            state.money.stopped = false;
            Ok(())
        })
    }
    /// Bind the allowance and baseline to the campaign before any reservation.
    pub fn openrouter_preflight(
        &self,
        allowance: u64,
        fetch: impl FnOnce() -> Result<crate::assist::openrouter::Ceiling, String>,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            if state.money.stopped { return Ok(Err("campaign_spending_stopped".into())); }
            if state.money.openrouter.is_some() { return Ok(Ok(())); }
            let result = fetch().and_then(|snapshot| {
                state.money.ceiling_events.push(serde_json::json!({"phase":"preflight","allowance":allowance,"snapshot":snapshot}));
                if allowance == 0 || allowance > snapshot.remaining { return Err("openrouter_allowance_exceeds_ceiling".into()); }
                if !state.money.receipts.is_empty() { return Err("openrouter_campaign_not_fresh".into()); }
                state.money.openrouter = Some(serde_json::json!({"allowance":allowance,"baseline":snapshot}));
                Ok(())
            });
            state.money.ceiling_events.push(serde_json::json!({"phase":"preflight","allowance":allowance,"refusal":result.as_ref().err()}));
            Ok(result)
        })?
    }
    /// Check under the campaign lock after pacing, including the current reservation.
    pub(crate) fn openrouter_dispatch_check(
        &self,
        hash: crate::evidence::canonical::Digest,
        fetch: impl FnOnce() -> Result<crate::assist::openrouter::Ceiling, String>,
    ) -> Result<crate::assist::openrouter::DispatchPermit, String> {
        self.campaign().transaction(|state| {
            let result = (|| {
                if state.money.stopped {
                    return Err("campaign_spending_stopped".into());
                }
                let verified = state
                    .money
                    .openrouter
                    .as_ref()
                    .ok_or("openrouter_preflight_required")?;
                let baseline: crate::assist::openrouter::Ceiling =
                    serde_json::from_value(verified["baseline"].clone())
                        .map_err(|_| "openrouter_baseline_invalid")?;
                let snapshot = fetch()?;
                state
                    .money
                    .ceiling_events
                    .push(serde_json::json!({"phase":"dispatch","snapshot":snapshot}));
                let mut outstanding = 0u64;
                let mut settled = 0u64;
                for r in &state.money.receipts {
                    if r.outcome == "reserved" || r.actual_nano_usd.is_none() {
                        outstanding = outstanding
                            .checked_add(r.reserved_nano_usd)
                            .ok_or("openrouter_accounting_overflow")?;
                    } else if r.usage["openrouter_dispatched"] == true
                        || r.outcome == "settled_conservatively"
                    {
                        settled = settled
                            .checked_add(r.actual_nano_usd.unwrap_or(0))
                            .ok_or("openrouter_accounting_overflow")?;
                    }
                }
                let mut unreflected = 0;
                let mut has_usage_baseline = false;
                for (old, new) in [
                    (baseline.key_usage, snapshot.key_usage),
                    (baseline.account_usage, snapshot.account_usage),
                ] {
                    if let Some(old) = old {
                        has_usage_baseline = true;
                        let new = new.ok_or("openrouter_usage_unavailable")?;
                        if new < old
                            || new - old
                                > settled
                                    .saturating_add(crate::assist::openrouter::CONSUMER_TOLERANCE)
                        {
                            return Err("openrouter_concurrent_consumer".into());
                        }
                        // Key and account counters can update at different times. Use
                        // the least reflected spend so neither can release it early.
                        unreflected = unreflected.max(settled.saturating_sub(new - old));
                    }
                }
                if !has_usage_baseline {
                    unreflected = settled;
                }
                let effective_remaining = snapshot.remaining.saturating_sub(unreflected);
                state.money.ceiling_events.push(serde_json::json!({
                    "phase":"admission", "settled":settled, "unreflected":unreflected,
                    "outstanding":outstanding, "effective_remaining":effective_remaining
                }));
                if outstanding > effective_remaining {
                    return Err("openrouter_remaining_exhausted".into());
                }
                let receipt = state
                    .money
                    .receipts
                    .iter_mut()
                    .find(|r| {
                        r.request_hash == hash
                            && r.outcome == "reserved"
                            && r.usage["openrouter_dispatched"] != true
                    })
                    .ok_or("openrouter_reservation_required")?;
                receipt.usage["openrouter_dispatched"] = serde_json::json!(true);
                receipt.usage["reconciliation"] = serde_json::json!({"state":"pending","matches":false,"attempts":0,"attempted_ms":null});
                receipt.usage["qualification_eligible"] = serde_json::json!(false);
                Ok(crate::assist::openrouter::DispatchPermit::new(hash))
            })();
            if let Err(reason) = &result {
                state.money.stopped = true;
                state
                    .money
                    .ceiling_events
                    .push(serde_json::json!({"phase":"dispatch","refusal":reason}));
            }
            Ok(result)
        })?
    }
    /// Attach a later lookup to the original receipt without changing settled charges.
    /// Pending lookups retain their history; terminal records are immutable.
    pub fn record_openrouter_reconciliation(
        &self,
        id: &str,
        generation: Result<crate::assist::openrouter::Generation, &'static str>,
        attempts: u32,
        waited_ms: u64,
        attempted_ms: u64,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            let receipt = state.money.receipts.iter_mut().find(|r| r.id == id)
                .ok_or("unknown money reservation")?;
            if matches!(receipt.usage["reconciliation"]["state"].as_str(), Some("matched" | "mismatch" | "zero_cost_refused"))
                || (receipt.outcome == "settled_conservatively" && receipt.usage["generation_id"].is_null()) { return Ok(()); }
            let drifted = generation.as_ref().is_ok_and(|g| {
                let model = receipt.usage["requested_identity"]["model"].as_str()
                    .or_else(|| receipt.usage["response_identity"]["returned_model"].as_str()).unwrap_or("");
                let pin = receipt.usage["requested_identity"]["revision"].as_str().unwrap_or("");
                (!model.is_empty() && g.model != model && !crate::assist::openrouter::dated_pin(model, &g.model))
                    || (crate::assist::openrouter::dated_pin(model, pin) && g.model != pin)
            });
            // Only authoritative generation identity and cost can settle an unknown
            // incomplete charge. Known charges are never rewritten. Breaches stop spend.
            if receipt.actual_nano_usd.is_none() && receipt.outcome == "incomplete"
                && receipt.usage["openrouter_dispatched"] == true && !drifted
                && receipt.usage["requested_identity"]["model"].as_str().is_some_and(|s| !s.is_empty())
                && receipt.usage["requested_identity"]["revision"].as_str().is_some_and(|s| !s.is_empty())
                && let Ok(g) = &generation
            {
                let cost = g.cost_nano_usd;
                for scope in &receipt.scopes {
                    let counter = state.money.counters.get_mut(scope).ok_or("missing money counter")?;
                    counter.1 = counter.1.checked_sub(receipt.reserved_nano_usd)
                        .and_then(|n| n.checked_add(cost)).ok_or("openrouter_accounting_overflow")?;
                }
                receipt.usage["unknown_cost_settlement"] = serde_json::json!({"previous_actual_nano_usd":null,
                    "previous_charge_nano_usd":receipt.reserved_nano_usd,"generation_response_hash":g.response_hash});
                receipt.actual_nano_usd = Some(cost);
                if cost > receipt.reserved_nano_usd {
                    receipt.outcome = "cost_limit_exceeded".into();
                    state.money.stopped = true;
                }
            }
            // An authoritative generation may lower a conservative charge. It
            // never turns an unavailable root into an answer or silently raises spend.
            if receipt.outcome == "settled_conservatively" && !drifted
                && receipt.usage["requested_identity"]["model"].as_str().is_some_and(|s| !s.is_empty())
                && receipt.usage["requested_identity"]["revision"].as_str().is_some_and(|s| !s.is_empty())
                && let Ok(g) = &generation
                && let Some(previous) = receipt.actual_nano_usd
                && g.cost_nano_usd <= previous
            {
                for scope in &receipt.scopes {
                    let counter = state.money.counters.get_mut(scope).ok_or("missing money counter")?;
                    counter.1 = counter.1.checked_sub(previous - g.cost_nano_usd)
                        .ok_or("openrouter_accounting_overflow")?;
                }
                receipt.actual_nano_usd = Some(g.cost_nano_usd);
                receipt.usage["unknown_cost_settlement"]["reconciled"] = serde_json::json!(true);
                receipt.usage["unknown_cost_settlement"]["billed_nano_usd"] = serde_json::json!(g.cost_nano_usd);
            }
            let cost_matches = generation.as_ref().is_ok_and(|g| receipt.actual_nano_usd
                .is_some_and(|actual| if receipt.outcome == "settled_conservatively" {actual == g.cost_nano_usd} else {actual.abs_diff(g.cost_nano_usd) <= crate::assist::openrouter::RECONCILIATION_TOLERANCE}));
            let reason = match &generation {
                Err(reason) => Some(*reason),
                Ok(_) if !cost_matches => Some("openrouter_generation_cost_mismatch"),
                Ok(_) if drifted => Some("provider revision drift quarantined"),
                Ok(_) => None,
            };
            let pending = matches!(reason, Some("openrouter_generation_not_ready" | "openrouter_accounting_unavailable" | "openrouter_reconciliation_deadline"));
            let matches = reason.is_none();
            let status = if pending { "pending" } else if matches { "matched" } else { "mismatch" };
            if let Ok(g) = &generation {
                receipt.usage["revision_identity"] = serde_json::json!({
                    "dated_model":g.model,"provider_name":g.provider_name,
                    "requested_revision":receipt.usage["requested_identity"]["revision"],
                    "revision_drifted":drifted,"quarantined":drifted,
                });
            }
            let attempt = serde_json::json!({"state":status,"generation":generation.as_ref().ok(),
                "matches":matches,"reason":reason,"attempts":attempts,"waited_ms":waited_ms,"attempted_ms":attempted_ms});
            let mut history = receipt.usage["reconciliation"]["history"].as_array().cloned().unwrap_or_default();
            history.push(attempt.clone());
            receipt.usage["reconciliation"] = attempt;
            receipt.usage["reconciliation"]["history"] = serde_json::json!(history);
            receipt.usage["qualification_eligible"] = serde_json::json!(matches
                && receipt.outcome == "completed" && receipt.usage["identity_error"].is_null());
            if status == "mismatch" { state.money.stopped = true; }
            Ok(())
        })
    }
    /// Return bounded metadata receipts for review and partial-failure handoff.
    pub fn money_receipts(&self) -> Result<Vec<MoneyReceipt>, String> {
        self.campaign()
            .transaction(|state| Ok(state.money.receipts.clone()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::Ledger;
    use super::*;

    #[cfg(feature = "assist")]
    #[test]
    fn continuation_conservatively_charges_pacing_and_unknown_and_never_repeats() {
        for (actual, class, dispatched) in
            [(Some(0), "client_deadline", false), (None, "reset", true)]
        {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(temp.path(), true);
            ledger
                .bind_campaign(serde_json::json!({"fixture":true}), false)
                .unwrap();
            let hash = crate::evidence::canonical::Digest::of_bytes(b"fixture");
            ledger.begin_campaign_root(0, hash.clone()).unwrap();
            let scopes = vec![MoneyScope {
                id: "fixture".into(),
                cap_nano_usd: 100,
            }];
            let receipt = MoneyReceipt {
                id: "first".into(),
                request_hash: hash,
                scopes: vec!["fixture".into()],
                reserved_nano_usd: 100,
                actual_nano_usd: None,
                outcome: "reserved".into(),
                usage: serde_json::json!({}),
            };
            ledger.reserve_money(&scopes, receipt.clone()).unwrap();
            ledger
                .finish_money(
                    "first",
                    actual,
                    serde_json::json!({"transport_failure":class,"not_dispatched":!dispatched}),
                    false,
                )
                .unwrap();
            assert!(ledger.settle_continuation_root(0).unwrap());
            assert!(ledger.settle_continuation_root(0).unwrap());
            let reopened = Ledger::new(temp.path(), true);
            let settled = &reopened.money_receipts().unwrap()[0];
            assert_eq!(settled.outcome, "settled_conservatively");
            assert_eq!(settled.actual_nano_usd, Some(100));
            assert_eq!(settled.usage["qualification_eligible"], false);
            let mut next = receipt;
            next.id = "next".into();
            assert_eq!(
                reopened.reserve_money(&scopes, next).unwrap_err(),
                "money_budget_exhausted"
            );
        }
    }
    #[cfg(feature = "assist")]
    #[test]
    fn conservative_reconciliation_lowers_only_and_keeps_root_unavailable() {
        for cost in [60, 101] {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(temp.path(), true);
            ledger.campaign().transaction(|s| {
                s.money.counters.insert("fixture".into(), (1000,100));
                s.money.receipts.push(MoneyReceipt {id:"fixture".into(),request_hash:crate::evidence::canonical::Digest::of_bytes(b"fixture"),scopes:vec!["fixture".into()],reserved_nano_usd:100,actual_nano_usd:Some(100),outcome:"settled_conservatively".into(),usage:serde_json::json!({"generation_id":"gen-fixture","openrouter_dispatched":true,"requested_identity":{"model":"fixture","revision":"fixture"},"unknown_cost_settlement":{},"reconciliation":{"state":"pending"}})});
                Ok(())
            }).unwrap();
            let generation = crate::assist::openrouter::Generation {
                model: "fixture".into(),
                provider_name: "fixture".into(),
                cost_nano_usd: cost,
                response_hash: crate::evidence::canonical::Digest::of_bytes(b"generation"),
            };
            ledger
                .record_openrouter_reconciliation("fixture", Ok(generation), 1, 0, 1)
                .unwrap();
            let receipt = &ledger.money_receipts().unwrap()[0];
            assert_eq!(receipt.outcome, "settled_conservatively");
            assert_eq!(receipt.actual_nano_usd, Some(cost.min(100)));
            assert_eq!(
                receipt.usage["reconciliation"]["state"],
                if cost <= 100 { "matched" } else { "mismatch" }
            );
            ledger
                .campaign()
                .transaction(|s| {
                    assert_eq!(s.money.counters["fixture"].1, cost.min(100));
                    assert_eq!(s.money.stopped, cost > 100);
                    Ok(())
                })
                .unwrap();
        }
    }
    #[cfg(feature = "assist")]
    #[cfg(feature = "assist")]
    #[test]
    fn g12_later_missing_generation_recovers_with_prior_conservative_history() {
        let receipt = MoneyReceipt {
            id: "unknown".into(),
            request_hash: crate::evidence::canonical::Digest::of_bytes(b"fixture"),
            scopes: vec![],
            reserved_nano_usd: 100,
            actual_nano_usd: None,
            outcome: "incomplete".into(),
            usage: serde_json::json!({"campaign_root_index":0,"reconciliation":{"state":"mismatch","reason":"openrouter_generation_missing"}}),
        };
        let mut prior = receipt.clone();
        prior.id = "settled".into();
        prior.actual_nano_usd = Some(100);
        prior.outcome = "settled_conservatively".into();
        let state = MoneyState {
            stopped: true,
            receipts: vec![prior, receipt],
            ..Default::default()
        };
        assert!(recoverable_missing_generation(&state));
    }
    #[cfg(feature = "assist")]
    #[test]
    fn g12_missing_generation_settlement_preserves_other_stop_causes() {
        for cause in ["operator", "ceiling", "bound", "identity", "generation_id"] {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(temp.path(), true);
            ledger.campaign().transaction(|s| {
                s.money.stopped = true;
                s.money.receipts = vec![MoneyReceipt {
                    id:"unknown".into(),request_hash:crate::evidence::canonical::Digest::of_bytes(b"fixture"),
                    scopes:vec![],reserved_nano_usd:100,actual_nano_usd:None,outcome:"incomplete".into(),
                    usage:serde_json::json!({"campaign_root_index":0,"reconciliation":{"state":"mismatch","reason":"openrouter_generation_missing"}})
                }];
                match cause {
                    "ceiling" => s.money.ceiling_events.push(serde_json::json!({"refusal":"openrouter_concurrent_consumer"})),
                    "bound" => s.money.receipts[0].usage["bound_breach"] = serde_json::json!(true),
                    "identity" => s.money.receipts[0].usage["identity_error"] = serde_json::json!("drift"),
                    "generation_id" => s.money.receipts[0].usage["generation_id"] = serde_json::json!("gen-found"),
                    _ => (),
                }
                Ok(())
            }).unwrap();
            if cause == "operator" {
                ledger.stop_spending().unwrap();
            }
            assert!(ledger.settle_unknown_at_reservation().is_err(), "{cause}");
            assert!(
                ledger.money_receipts().unwrap()[0]
                    .actual_nano_usd
                    .is_none()
            );
        }
    }
    #[cfg(feature = "assist")]
    #[test]
    fn g12_resume_identity_and_reconciliation_preserve_prior_spend() {
        use crate::{assist::openrouter::Generation, evidence::canonical::Digest};
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), true);
        let identity = serde_json::json!({"plan":"fixture","policy":"epoch-2","allowance":100});
        assert!(ledger.bind_campaign(identity.clone(), true).is_err());
        ledger.bind_campaign(identity.clone(), false).unwrap();
        assert!(
            ledger
                .bind_campaign(serde_json::json!({"plan":"changed"}), true)
                .is_err()
        );
        ledger
            .begin_campaign_root(0, Digest::of_bytes(b"fixture"))
            .unwrap();
        let scopes = [MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: 100,
        }];
        let make = |id: &str| MoneyReceipt {
            id: id.into(),
            request_hash: Digest::of_bytes(b"fixture"),
            scopes: vec!["smoke".into()],
            reserved_nano_usd: 60,
            actual_nano_usd: None,
            outcome: "reserved".into(),
            usage: serde_json::json!({"openrouter_dispatched":true,
                "requested_identity":{"model":"fixture-model","revision":"absent"}}),
        };
        ledger.reserve_money(&scopes, make("first")).unwrap();
        ledger
            .finish_money(
                "first",
                None,
                serde_json::json!({"generation_id":"gen-fixture","transport_failure":"timeout"}),
                false,
            )
            .unwrap();
        assert!(ledger.reserve_money(&scopes, make("too-early")).is_err());
        assert_eq!(
            ledger.money_receipts().unwrap()[0].usage["campaign_root_index"],
            0
        );
        let generation = Generation {
            cost_nano_usd: 50,
            response_hash: Digest::of_bytes(b"proof"),
            model: "fixture-model".into(),
            provider_name: "fixture-provider".into(),
        };
        ledger
            .record_openrouter_reconciliation("first", Ok(generation.clone()), 1, 0, 1)
            .unwrap();
        let receipt = ledger.money_receipts().unwrap().remove(0);
        assert_eq!(receipt.actual_nano_usd, Some(50));
        assert_eq!(receipt.outcome, "incomplete");
        assert_eq!(receipt.usage["reconciliation"]["state"], "matched");
        assert_eq!(receipt.usage["transport_failure"], "timeout");
        ledger.bind_campaign(identity, true).unwrap();
        // Reopening did not reset 50 spent; another reservation of 60 is refused.
        assert!(ledger.reserve_money(&scopes, make("retry")).is_err());
        ledger
            .record_openrouter_reconciliation("first", Ok(generation), 1, 0, 2)
            .unwrap();
        assert_eq!(
            ledger.money_receipts().unwrap()[0].actual_nano_usd,
            Some(50)
        );
    }
    #[cfg(feature = "assist")]
    #[test]
    fn g12_unknown_settlement_requires_identity_and_stops_on_overrun() {
        use crate::{assist::openrouter::Generation, evidence::canonical::Digest};
        for (model, cost, actual, outcome) in [
            ("wrong-model", 0, None, "incomplete"),
            ("fixture-model", 0, Some(0), "incomplete"),
            ("fixture-model", 101, Some(101), "cost_limit_exceeded"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let ledger = Ledger::new(temp.path(), true);
            ledger.reserve_money(&[MoneyScope{id:"smoke".into(),cap_nano_usd:200}],MoneyReceipt{
                id:"fixture".into(),request_hash:Digest::of_bytes(b"fixture"),scopes:vec!["smoke".into()],
                reserved_nano_usd:100,actual_nano_usd:None,outcome:"reserved".into(),
                usage:serde_json::json!({"openrouter_dispatched":true,"requested_identity":{"model":"fixture-model","revision":"absent"}})
            }).unwrap();
            ledger
                .finish_money("fixture", None, serde_json::json!({}), false)
                .unwrap();
            ledger
                .record_openrouter_reconciliation(
                    "fixture",
                    Ok(Generation {
                        cost_nano_usd: cost,
                        response_hash: Digest::of_bytes(b"proof"),
                        model: model.into(),
                        provider_name: "fixture".into(),
                    }),
                    1,
                    0,
                    1,
                )
                .unwrap();
            let r = ledger.money_receipts().unwrap().remove(0);
            assert_eq!(r.actual_nano_usd, actual);
            assert_eq!(r.outcome, outcome);
            if model == "wrong-model" || cost > 100 {
                assert!(ledger.bind_campaign(serde_json::json!({}), false).is_err());
            }
        }
    }
    #[cfg(feature = "assist")]
    #[test]
    fn g12_conservative_settlement_keeps_known_cost_and_unreflected_ceiling_charge() {
        use crate::{assist::openrouter::Ceiling, evidence::canonical::Digest};
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), true);
        let baseline = Ceiling {
            remaining: 200_000_000,
            key_usage: Some(0),
            account_usage: Some(0),
            hashes: [Digest::of_bytes(b"key"), Digest::of_bytes(b"account")],
        };
        ledger
            .bind_campaign(serde_json::json!({"fixture":true}), false)
            .unwrap();
        ledger
            .openrouter_preflight(200_000_000, || Ok(baseline.clone()))
            .unwrap();
        let scopes = [MoneyScope {
            id: "smoke".into(),
            cap_nano_usd: 200_000_000,
        }];
        for (i, id, amount) in [(0, "known", 10_000_000), (1, "unknown", 60_000_000)] {
            let hash = Digest::of_bytes(id.as_bytes());
            ledger.begin_campaign_root(i, hash.clone()).unwrap();
            ledger
                .reserve_money(
                    &scopes,
                    MoneyReceipt {
                        id: id.into(),
                        request_hash: hash,
                        scopes: vec!["smoke".into()],
                        reserved_nano_usd: amount,
                        actual_nano_usd: None,
                        outcome: "reserved".into(),
                        usage: serde_json::json!({}),
                    },
                )
                .unwrap();
            if id == "known" {
                ledger
                    .finish_money(id, Some(5_000_000), serde_json::json!({}), true)
                    .unwrap();
            }
        }
        let known = serde_json::to_value(&ledger.money_receipts().unwrap()[0]).unwrap();
        ledger.settle_unknown_at_reservation().unwrap();
        assert_eq!(
            serde_json::to_value(&ledger.money_receipts().unwrap()[0]).unwrap(),
            known
        );
        let next = Digest::of_bytes(b"next");
        ledger.begin_campaign_root(2, next.clone()).unwrap();
        ledger
            .reserve_money(
                &scopes,
                MoneyReceipt {
                    id: "next".into(),
                    request_hash: next.clone(),
                    scopes: vec!["smoke".into()],
                    reserved_nano_usd: 115_000_000,
                    actual_nano_usd: None,
                    outcome: "reserved".into(),
                    usage: serde_json::json!({}),
                },
            )
            .unwrap();
        let mut fresh = baseline;
        fresh.remaining = 140_000_000;
        assert_eq!(
            ledger
                .openrouter_dispatch_check(next, || Ok(fresh))
                .err()
                .unwrap(),
            "openrouter_remaining_exhausted"
        );
        assert!(ledger.settle_unknown_at_reservation().is_err());
    }
    #[test]
    fn ordinary_ai_ledger_rewrites_preserve_assist_money_receipts() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), false);
        let expected = MoneyState {
            stopped: false,
            campaign_identity: None,
            campaign_root: None,
            openrouter: None,
            ceiling_events: Vec::new(),
            counters: BTreeMap::from([("epoch/frozen".into(), (100, 60))]),
            receipts: vec![MoneyReceipt {
                id: "reserved-before-feature-change".into(),
                request_hash: crate::evidence::canonical::Digest::of_bytes(b"frozen request"),
                scopes: vec!["epoch/frozen".into()],
                reserved_nano_usd: 60,
                actual_nano_usd: None,
                outcome: "reserved".into(),
                usage: serde_json::Value::Null,
            }],
        };
        let expected = serde_json::to_value(expected).unwrap();
        ledger
            .transaction(|state| {
                state.money = serde_json::from_value(expected.clone()).unwrap();
                Ok(())
            })
            .unwrap();
        // This is the ordinary AI ledger path, compiled with assist disabled too.
        assert_eq!(ledger.used("unrelated").unwrap(), 0);
        let actual = ledger
            .transaction(|state| Ok(serde_json::to_value(&state.money).unwrap()))
            .unwrap();
        assert_eq!(actual, expected);
    }
}
