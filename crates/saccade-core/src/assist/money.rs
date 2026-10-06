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
