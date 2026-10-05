//! Locked cross-process reservations. A persisted reservation is consumed even
//! when its owner crashes; payload bytes and credentials never enter the ledger.
use crate::evidence::canonical::Digest;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A global and provider-specific ceiling for one independent budget scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caps {
    /// Total real attempts, not number of cases in batches.
    pub total: u64,
    /// Missing provider IDs have no allowance.
    pub providers: BTreeMap<String, u64>,
}
/// Run/project/optional parent scope. Evaluation uses a separate namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Stable budget identity; callers never accept this from model responses.
    pub id: String,
    /// Ceilings may decrease on reopening, never increase.
    pub caps: Caps,
}
/// Exact metadata reserved before an external dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    /// Locally generated reservation identity.
    pub id: String,
    /// Approved provider ID.
    pub provider: String,
    /// Actual requested model.
    pub model: String,
    /// Exact external body hash.
    pub payload_sha256: Digest,
    /// Transitive policy roots, without payload data.
    pub source_roots: Vec<String>,
    /// Cases/questions in this single request.
    pub batch_size: usize,
    /// Local reservation time.
    pub started_ms: u64,
    /// Reserved remains consumed when no outcome was recorded.
    pub outcome: String,
}
/// Local payload policy decision, distinct from consumed external attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadAudit {
    /// Intended approved provider identifier.
    pub provider: String,
    /// Exact body hash, without body bytes.
    pub payload_sha256: Digest,
    /// Transitive provenance.
    pub source_roots: Vec<String>,
    /// Allow or deny. This is audit data, never authority.
    pub policy: String,
}
/// Client-side pacing for one reservation: a provider-wide and a model-specific
/// sliding 60-second request window plus in-flight concurrency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaceLimits {
    /// Provider-wide dispatches per sliding minute.
    pub provider_rpm: u32,
    /// Provider-wide unfinished reservations.
    pub provider_concurrency: u32,
    /// Same-model dispatches per sliding minute.
    pub model_rpm: u32,
    /// Same-model unfinished reservations.
    pub model_concurrency: u32,
    /// An unfinished reservation older than this no longer holds a slot
    /// (crashed owners); it stays consumed for budget purposes.
    pub lease_ms: u64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Counter {
    caps: Option<Caps>,
    total: u64,
    providers: BTreeMap<String, u64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Probe {
    failures: u32,
    next_probe_at: u64,
    lease_until: u64,
    stopped: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct State {
    // wave4
    #[cfg(feature = "assist")]
    #[serde(default)]
    money: money::MoneyState,
    scopes: BTreeMap<String, Counter>,
    attempts: Vec<Attempt>,
    probes: BTreeMap<String, Probe>,
    #[serde(default)]
    payloads: Vec<PayloadAudit>,
}
/// Persistent ledger with an OS advisory lock released automatically on process death.
#[derive(Debug, Clone)]
pub struct Ledger {
    dir: PathBuf,
    namespace: String,
}
fn io(e: impl std::fmt::Display) -> String {
    format!("budget ledger: {e}")
}
impl Ledger {
    /// Namespace must be production or evaluation; files are never shared between them.
    pub fn new(dir: &Path, evaluation: bool) -> Self {
        Self {
            dir: dir.to_owned(),
            namespace: if evaluation {
                "evaluation"
            } else {
                "production"
            }
            .into(),
        }
    }
    fn transaction<T>(&self, f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let lock_path = self.dir.join(format!("{}.lock", self.namespace));
        let state_path = self.dir.join(format!("{}.json", self.namespace));
        for p in [&lock_path, &state_path] {
            if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(io("refusing symlink"));
            }
        }
        let lock: File = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(io)?;
        FileExt::lock_exclusive(&lock).map_err(io)?;
        let mut state = match std::fs::read(&state_path) {
            Ok(b) => serde_json::from_slice(&b).map_err(io)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(io(e)),
        };
        let result = f(&mut state)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir).map_err(io)?;
        tmp.write_all(&serde_json::to_vec(&state).map_err(io)?)
            .map_err(io)?;
        tmp.as_file().sync_all().map_err(io)?;
        tmp.persist(&state_path).map_err(io)?;
        // The separate lock inode remains stable across atomic state replacement.
        #[cfg(unix)]
        File::open(&self.dir)
            .and_then(|d| d.sync_all())
            .map_err(io)?;
        Ok(result)
    }
    /// Atomically consume every scope and the provider allowance, including a
    /// single shared probe lease when a failed provider becomes eligible again.
    pub fn reserve(
        &self,
        scopes: &[Scope],
        attempt: Attempt,
        probe: bool,
    ) -> Result<String, String> {
        self.reserve_paced(scopes, attempt, probe, None)
    }
    /// [`Ledger::reserve`] plus client-side pacing checked under the same lock,
    /// so concurrent processes cannot jointly exceed a rate or concurrency limit.
    /// A paced refusal consumes nothing and reports `paced retry_at=<ms>`.
    pub fn reserve_paced(
        &self,
        scopes: &[Scope],
        mut attempt: Attempt,
        probe: bool,
        pace: Option<&PaceLimits>,
    ) -> Result<String, String> {
        self.transaction(|s| {
            if scopes.is_empty()
                || attempt.batch_size == 0
                || attempt.provider.is_empty()
                || attempt.model.is_empty()
            {
                return Err(io("invalid reservation"));
            }
            if let Some(pace) = pace {
                let now = attempt.started_ms;
                let wait = [
                    (pace.provider_rpm, pace.provider_concurrency, false),
                    (pace.model_rpm, pace.model_concurrency, true),
                ]
                .into_iter()
                .filter_map(|(rpm, concurrency, per_model)| {
                    let mine = |a: &&Attempt| {
                        a.provider == attempt.provider && (!per_model || a.model == attempt.model)
                    };
                    let mut window: Vec<u64> = s
                        .attempts
                        .iter()
                        .filter(mine)
                        .filter(|a| a.outcome != "not_dispatched")
                        .map(|a| a.started_ms)
                        .filter(|t| t.saturating_add(60_000) > now)
                        .collect();
                    window.sort_unstable();
                    let in_flight = s
                        .attempts
                        .iter()
                        .filter(mine)
                        .filter(|a| {
                            a.outcome == "reserved"
                                && a.started_ms.saturating_add(pace.lease_ms) > now
                        })
                        .count();
                    let rate_at = (window.len() >= rpm.max(1) as usize)
                        .then(|| window[window.len() - rpm.max(1) as usize].saturating_add(60_001));
                    let slot_at = (in_flight >= concurrency.max(1) as usize)
                        .then(|| now.saturating_add(1_000));
                    rate_at.max(slot_at)
                })
                .max();
                if let Some(at) = wait {
                    return Err(format!("paced retry_at={at}"));
                }
            }
            let mut ids = std::collections::BTreeSet::new();
            for scope in scopes {
                if scope.id.is_empty() || !ids.insert(&scope.id) {
                    return Err(io("invalid or duplicate scope"));
                }
                let c = s.scopes.entry(scope.id.clone()).or_default();
                let effective = match &c.caps {
                    None => scope.caps.clone(),
                    Some(old) => Caps {
                        total: old.total.min(scope.caps.total),
                        providers: old
                            .providers
                            .iter()
                            .map(|(p, cap)| {
                                (
                                    p.clone(),
                                    (*cap).min(*scope.caps.providers.get(p).unwrap_or(&0)),
                                )
                            })
                            .collect(),
                    },
                };
                if c.total >= effective.total
                    || c.providers.get(&attempt.provider).copied().unwrap_or(0)
                        >= effective
                            .providers
                            .get(&attempt.provider)
                            .copied()
                            .unwrap_or(0)
                {
                    return Err("budget_exhausted".into());
                }
                c.caps = Some(effective);
            }
            if s.attempts.iter().any(|a| a.id == attempt.id) {
                return Err(io("duplicate reservation"));
            }
            if s.probes.get(&attempt.provider).is_some_and(|p| p.stopped) {
                return Err("provider_stopped".into());
            }
            if probe {
                let p = s
                    .probes
                    .entry(format!("{}/{}", attempt.provider, attempt.model))
                    .or_default();
                if p.stopped {
                    return Err("provider_stopped".into());
                }
                if attempt.started_ms < p.next_probe_at.max(p.lease_until) {
                    return Err(format!(
                        "deferred retry_at={}",
                        p.next_probe_at.max(p.lease_until)
                    ));
                }
                if p.failures > 0 {
                    p.lease_until = attempt.started_ms.saturating_add(300_000);
                }
            }
            for scope in scopes {
                let c = s.scopes.entry(scope.id.clone()).or_default();
                c.total += 1;
                *c.providers.entry(attempt.provider.clone()).or_default() += 1;
            }
            attempt.outcome = "reserved".into();
            let id = attempt.id.clone();
            s.attempts.push(attempt);
            Ok(id)
        })
    }
    /// Persist a bounded classification. Failed probes back off from 30 to 300 s.
    /// `rejected` records a request-specific refusal (HTTP 400/413/422): the
    /// attempt stays consumed, but it is not evidence that the provider or
    /// model is unavailable, so it neither cools down nor stops either.
    pub fn finish(
        &self,
        id: &str,
        outcome: &str,
        stop: bool,
        retry_at: Option<u64>,
    ) -> Result<(), String> {
        if ![
            "answered",
            "unavailable",
            "invalid",
            "not_dispatched",
            "rejected",
        ]
        .contains(&outcome)
        {
            return Err(io("unknown outcome"));
        }
        self.transaction(|s| {
            let a = s
                .attempts
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or_else(|| io("unknown reservation"))?;
            a.outcome = outcome.into();
            let p = s
                .probes
                .entry(format!("{}/{}", a.provider, a.model))
                .or_default();
            if outcome == "answered" {
                *p = Probe::default();
            } else if outcome == "rejected" {
                p.lease_until = 0;
            } else if outcome != "not_dispatched" {
                p.failures = p.failures.saturating_add(1);
                p.next_probe_at = crate::judge::now_ms()
                    .saturating_add(
                        (30_000u64.saturating_mul(1u64 << p.failures.min(4).saturating_sub(1)))
                            .min(300_000),
                    )
                    .max(retry_at.unwrap_or(0));
                p.lease_until = 0;
                p.stopped |= stop;
            }
            if stop {
                s.probes.entry(a.provider.clone()).or_default().stopped = true;
            }
            Ok(())
        })
    }
    /// Persist the payload policy decision before any dispatch, including denial.
    pub fn audit_payload(&self, audit: PayloadAudit) -> Result<(), String> {
        self.transaction(|s| {
            s.payloads.push(audit);
            Ok(())
        })
    }
    /// Shared provider/model retry deadline, including an in-flight probe lease.
    pub fn retry_at(&self, provider: &str, model: &str) -> Result<u64, String> {
        self.transaction(|s| {
            if s.probes.get(provider).is_some_and(|p| p.stopped) {
                return Ok(u64::MAX);
            }
            Ok(s.probes
                .get(&format!("{provider}/{model}"))
                .map_or(0, |p| p.next_probe_at.max(p.lease_until)))
        })
    }
    /// Number consumed in a scope, including crashed reservations.
    pub fn used(&self, scope: &str) -> Result<u64, String> {
        self.transaction(|s| Ok(s.scopes.get(scope).map_or(0, |c| c.total)))
    }
    /// Audit entries contain hashes and classifications only.
    pub fn attempts(&self) -> Result<Vec<Attempt>, String> {
        self.transaction(|s| Ok(s.attempts.clone()))
    }
}

/// Fresh local run/reservation identifier, never an execution authority.
pub fn new_id() -> String {
    crate::local::random_token()
}
/// Current local audit timestamp; clock metadata never grants authority.
pub fn now_ms() -> u64 {
    crate::judge::now_ms()
}

// wave4
#[cfg(feature = "assist")]
#[path = "assist/money.rs"]
mod money;
#[cfg(feature = "assist")]
pub use money::{MoneyReceipt, MoneyScope};
