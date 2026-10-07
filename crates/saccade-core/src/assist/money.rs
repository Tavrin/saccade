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
    receipts: Vec<MoneyReceipt>,
    #[serde(default)]
    openrouter: Option<UsageValue>,
    #[serde(default)]
    ceiling_events: Vec<UsageValue>,
}
#[cfg(feature = "assist")]
impl Ledger {
    fn campaign(&self) -> Self {
        Self {
            dir: self.dir.clone(),
            namespace: "campaign".into(),
        }
    }
    /// Durably stop all modes and epochs in this campaign after a demonstrated breach.
    pub fn stop_spending(&self) -> Result<(), String> {
        self.campaign().transaction(|state| {
            state.money.stopped = true;
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
            let dispatched = receipt.usage["openrouter_dispatched"] == true;
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
            if dispatched {
                receipt.usage["openrouter_dispatched"] = serde_json::json!(true);
            }
            receipt.outcome = if receipt.usage["bound_breach"] == true {
                "usage_limit_exceeded"
            } else if charged > receipt.reserved_nano_usd {
                "cost_limit_exceeded"
            } else if complete {
                "completed"
            } else {
                "incomplete"
            }
            .into();
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
                    } else if r.usage["openrouter_dispatched"] == true {
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
                receipt.usage = serde_json::json!({"openrouter_dispatched":true});
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
    /// Attach authoritative generation reconciliation to the original money receipt.
    pub fn record_openrouter_reconciliation(
        &self,
        id: &str,
        generation: Result<(u64, crate::evidence::canonical::Digest), &'static str>,
        matches: bool,
        attempts: u32,
        waited_ms: u64,
    ) -> Result<(), String> {
        self.campaign().transaction(|state| {
            let receipt = state
                .money
                .receipts
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or("unknown money reservation")?;
            let reason = match &generation {
                Err(reason) => Some(*reason),
                Ok(_) if !matches => Some("openrouter_generation_cost_mismatch"),
                Ok(_) => None,
            };
            receipt.usage["reconciliation"] = serde_json::json!({
                "generation":generation.as_ref().ok(),"matches":matches,
                "reason":reason,"attempts":attempts,"waited_ms":waited_ms,
            });
            if !matches {
                state.money.stopped = true;
            }
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

    #[test]
    fn ordinary_ai_ledger_rewrites_preserve_assist_money_receipts() {
        let temp = tempfile::tempdir().unwrap();
        let ledger = Ledger::new(temp.path(), false);
        let expected = MoneyState {
            stopped: false,
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
