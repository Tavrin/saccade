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
        mut attempt: Attempt,
        probe: bool,
    ) -> Result<String, String> {
        self.transaction(|s| {
            if scopes.is_empty()
                || attempt.batch_size == 0
                || attempt.provider.is_empty()
                || attempt.model.is_empty()
            {
                return Err(io("invalid reservation"));
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
    pub fn finish(
        &self,
        id: &str,
        outcome: &str,
        stop: bool,
        retry_at: Option<u64>,
    ) -> Result<(), String> {
        if !["answered", "unavailable", "invalid", "not_dispatched"].contains(&outcome) {
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
