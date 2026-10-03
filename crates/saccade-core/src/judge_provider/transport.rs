//! User-owned endpoints, transitive egress checks and shared budgeted HTTP.
use super::{Keys, Secret};
use crate::budget_ledger::{Attempt, Caps, Ledger, Scope};
use crate::decision_provider::{ProviderFailure, RetryClass, Usage};
use crate::evidence::canonical::Digest;
use crate::root_policy::{Egress, RootPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Machine-local network export permission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressSetting {
    /// Default; local review remains available.
    #[default]
    Deny,
    /// Explicit human-owned export authorization.
    Allow,
}
/// Machine-local root permission; project files cannot create these records.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootSetting {
    /// Stable provenance identifier.
    pub id: String,
    /// Absolute local root, resolved canonically on loading.
    pub path: PathBuf,
    /// Human-owned egress policy, default deny.
    #[serde(default)]
    pub egress: EgressSetting,
}
/// Dedicated custom endpoint and credential binding, owned by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomProvider {
    /// HTTPS endpoint including the request path.
    pub endpoint: String,
    /// Dedicated plain file name under the user configuration directory.
    pub key_file: String,
    /// Dedicated key variable; built-in bindings are forbidden.
    pub key_var: String,
}
/// Only this user-level document can configure network/path/key authority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    /// Registered transitive source permissions.
    #[serde(default)]
    pub roots: Vec<RootSetting>,
    /// Generated output authorization for CLI reviews.
    pub out_root: Option<PathBuf>,
    /// Approved custom providers keyed by a separate provider ID.
    #[serde(default)]
    pub providers: BTreeMap<String, CustomProvider>,
    /// Optional persistent parent cap, shared by CLI and MCP runs.
    pub parent_budget: Option<Caps>,
}
impl UserConfig {
    /// Read a human-selected user file; missing default configuration grants nothing.
    pub fn load(path: &Path) -> Result<Self, String> {
        let config: Self = if path.exists() {
            toml::from_str(
                &std::fs::read_to_string(path).map_err(|_| "cannot read user configuration")?,
            )
            .map_err(|_| "invalid user configuration")?
        } else {
            Self::default()
        };
        config.validate()?;
        Ok(config)
    }
    /// Reject ambiguous roots and credential reuse before any key is read.
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        for r in &self.roots {
            if r.id.is_empty() || !ids.insert(&r.id) || !r.path.is_absolute() || !r.path.is_dir() {
                return Err("invalid user root policy".into());
            }
        }
        for (id, p) in &self.providers {
            if ["jev", "gemini", "human", "opencode"].contains(&id.as_str())
                || id.is_empty()
                || !p.endpoint.starts_with("https://")
                || p.endpoint[8..]
                    .split('/')
                    .next()
                    .is_none_or(|h| h.is_empty() || h.contains(['@', '?', '#']))
                || p.endpoint.contains(['?', '#', '\\'])
                || p.key_file.is_empty()
                || p.key_file.contains(['/', '\\'])
                || ["jev.env", "gemini.env", ".", ".."].contains(&p.key_file.as_str())
                || p.key_var.is_empty()
                || ["JEV_API_KEY", "SACCADE_GEMINI_API_KEY", "GEMINI_API_KEY"]
                    .contains(&p.key_var.as_str())
            {
                return Err("custom providers require a distinct ID, HTTPS endpoint and dedicated credentials".into());
            }
        }
        Ok(())
    }
    /// Assign only user-owned egress permissions to the startup registry.
    pub fn apply(&self, policy: &mut RootPolicy) -> Result<(), String> {
        self.validate()?;
        for r in &mut policy.roots {
            r.egress = if self.roots.iter().any(|u| {
                u.egress == EgressSetting::Allow
                    && crate::paths::canonicalize(&u.path).is_ok_and(|p| p == r.path)
            }) {
                Egress::Allow
            } else {
                Egress::Deny
            };
        }
        Ok(())
    }
    /// Resolve all provenance roots and aliases. Denial and unknown provenance win.
    pub fn authorize(&self, sources: &[String], policy: &RootPolicy) -> Result<(), String> {
        if sources.is_empty() {
            return Err("egress_denied: incomplete source provenance".into());
        }
        for source in sources {
            let setting = self.roots.iter().find(|r| &r.id == source);
            let path = match setting {
                Some(r) => r.path.clone(),
                None if Path::new(source).is_absolute() => PathBuf::from(source),
                _ => return Err("egress_denied: unknown source root".into()),
            };
            let canonical = crate::paths::canonicalize(&path)
                .map_err(|_| "egress_denied: unavailable source root")?;
            // Check lexical and physical aliases against every overlapping user policy.
            let mut matched = false;
            for r in &self.roots {
                let root = crate::paths::canonicalize(&r.path)
                    .map_err(|_| "egress_denied: unavailable policy root")?;
                if canonical.starts_with(&root)
                    || path.starts_with(&r.path)
                    || root.starts_with(&canonical)
                {
                    matched = true;
                    if r.egress == EgressSetting::Deny {
                        return Err("egress_denied: source root denies export".into());
                    }
                }
            }
            let registered = if policy.root_of(&canonical).is_some() {
                policy.egress(&canonical) == Egress::Allow
            } else {
                // Explicit storage targets are reachable through an allowed root,
                // but do not become independently browsable registered roots.
                policy
                    .roots
                    .iter()
                    .any(|r| r.egress == Egress::Allow && policy.allows(&r.path, &canonical))
            };
            if !matched || !registered {
                return Err("egress_denied: unclassified or denied root".into());
            }
        }
        Ok(())
    }
    fn endpoint(
        &self,
        provider: &str,
        model: &str,
        keys: &Keys,
    ) -> Result<(String, String, Secret), String> {
        if model.is_empty()
            || !model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        {
            return Err("invalid provider model ID".into());
        }
        match provider {
            "jev" => Ok((
                "https://api.typesafe.ai/v1/systemone".into(),
                "Authorization".into(),
                keys.load("jev.env", "JEV_API_KEY")?,
            )),
            "gemini" => Ok((
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
                ),
                "x-goog-api-key".into(),
                keys.load("gemini.env", "SACCADE_GEMINI_API_KEY")?,
            )),
            other => {
                let p = self
                    .providers
                    .get(other)
                    .ok_or("provider is not approved in user configuration")?;
                self.validate()?;
                let secret = keys.load(&p.key_file, &p.key_var)?;
                for (file, var) in [
                    ("jev.env", "JEV_API_KEY"),
                    ("gemini.env", "SACCADE_GEMINI_API_KEY"),
                ] {
                    if keys
                        .load(file, var)
                        .is_ok_and(|builtin| builtin.expose() == secret.expose())
                    {
                        return Err(
                            "custom credentials must not reuse built-in provider keys".into()
                        );
                    }
                }
                Ok((p.endpoint.clone(), "Authorization".into(), secret))
            }
        }
    }
}
/// Project restrictions can reduce authority and choose approved IDs only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPolicy {
    /// Optional lower attempt cap.
    pub budget_calls: Option<u64>,
    /// Approved subset; empty uses the startup set.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Approved requested model, without endpoint or key fields.
    pub model: Option<String>,
    /// Named routing profile; project files cannot alter execution authority.
    pub profile: Option<String>,
    /// Approved visual fallback model IDs, without endpoints.
    #[serde(default)]
    pub gemini_models: Vec<String>,
}
/// Reject security fields even when nested in a historical project artifact.
pub fn reject_project_overrides(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                if [
                    "base_url",
                    "endpoint",
                    "credential",
                    "credentials",
                    "key_file",
                    "key_var",
                    "keys_dir",
                    "api_key",
                    "env_var",
                    "user_config",
                    "allow_provider_calls",
                ]
                .contains(&k.as_str())
                {
                    return Err("project endpoint and credential overrides are forbidden".into());
                }
                reject_project_overrides(v)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                reject_project_overrides(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}
/// Startup authority cannot be raised by a tool, report, project or model.
#[derive(Debug, Clone)]
pub struct Authorization {
    /// Explicit CLI --run or MCP startup --allow-provider-calls.
    pub enabled: bool,
    /// Run, project and optional parent caps.
    pub scopes: Vec<Scope>,
}
impl Authorization {
    /// Check before preparation or credential access.
    pub fn check(&self) -> Result<(), String> {
        if !self.enabled || self.scopes.is_empty() || self.scopes.iter().any(|s| s.caps.total == 0)
        {
            Err("network_authorization_required".into())
        } else {
            Ok(())
        }
    }
}
/// Exact response and deterministic HTTP metadata, usable by offline fixtures.
pub struct HttpReply {
    /// HTTP status (redirects are never followed).
    pub status: u16,
    /// Provider wait in seconds.
    pub retry_after_secs: Option<u64>,
    /// Exact provider envelope.
    pub body: Vec<u8>,
}
/// The only raw HTTP boundary. Tests supply recorded replies without sockets.
pub trait Http {
    /// Send one reserved request. Secret-bearing headers must never be logged.
    fn post(
        &self,
        url: &str,
        header: (&str, &str),
        payload: &[u8],
        timeout: Duration,
    ) -> Result<HttpReply, String>;
}
fn retry_after(value: &str) -> Option<u64> {
    value.trim().parse().ok().or_else(|| {
        httpdate::parse_http_date(value).ok().map(|at| {
            at.duration_since(std::time::SystemTime::now())
                .map_or(0, |d| {
                    d.as_secs().saturating_add(u64::from(d.subsec_nanos() > 0))
                })
        })
    })
}
/// Parse usage independently from answer text. Missing rates/cost stay unknown.
pub fn usage(body: &[u8]) -> Option<Usage> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let u = value.get("usage").or_else(|| value.get("usageMetadata"))?;
    let tokens = |names: &[&str]| names.iter().find_map(|n| u[*n].as_u64());
    Some(Usage {
        input_tokens: tokens(&["input_tokens", "prompt_tokens", "promptTokenCount"]),
        output_tokens: tokens(&["output_tokens", "completion_tokens", "candidatesTokenCount"]),
        cost: u["cost"].as_f64().filter(|c| c.is_finite() && *c >= 0.0),
        currency: u["currency"].as_str().map(str::to_owned),
    })
}
/// Blocking production client with bounded body, deadline and zero redirects.
pub struct Network;
impl Http for Network {
    fn post(
        &self,
        url: &str,
        header: (&str, &str),
        payload: &[u8],
        timeout: Duration,
    ) -> Result<HttpReply, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        let mut response = agent
            .post(url)
            .header("Content-Type", "application/json")
            .header(header.0, header.1)
            .send(payload)
            .map_err(|_| "transport unavailable or timed out".to_owned())?;
        let status = response.status().as_u16();
        let retry_after_secs = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(retry_after);
        let body = response
            .body_mut()
            .with_config()
            .limit(256 * 1024)
            .read_to_vec()
            .map_err(|_| "provider response unavailable or too large".to_owned())?;
        Ok(HttpReply {
            status,
            retry_after_secs,
            body,
        })
    }
}
/// Authorized transport shared by canonical adapters and historical backends.
pub struct Transport<'a> {
    /// Machine-local policy owner.
    pub user: &'a UserConfig,
    /// Shared registry.
    pub roots: &'a RootPolicy,
    /// Immutable startup limits.
    pub authorization: &'a Authorization,
    /// Shared production or isolated evaluation ledger.
    pub ledger: &'a Ledger,
    /// Credential directory selected by the human, never the project.
    pub keys: &'a Keys,
    /// Live or mocked HTTP boundary.
    pub http: &'a dyn Http,
}
/// Outcome with actual answering identity and every persisted reservation.
#[derive(Debug, Clone)]
pub struct Exchange {
    /// Exact provider response.
    pub body: Vec<u8>,
    /// Actual model after an authorized fallback.
    pub model: String,
    /// Attempt identities, including retries.
    pub attempts: Vec<String>,
    /// Elapsed time for this resolution.
    pub latency_ms: u64,
}
fn failure(class: RetryClass, message: &str, wait: Option<u64>) -> ProviderFailure {
    ProviderFailure {
        class,
        message: message.into(),
        retry_after_secs: wait,
    }
}
impl Transport<'_> {
    /// Exactly one external attempt. Policy is re-read from its human-owned file
    /// by CLI/MCP before constructing the transport; checks repeat on every call.
    #[allow(clippy::too_many_arguments)]
    pub fn once(
        &self,
        provider: &str,
        model: &str,
        payload: &[u8],
        sources: &[String],
        batch_size: usize,
        timeout: Duration,
        probe: bool,
    ) -> Result<(Vec<u8>, String), ProviderFailure> {
        let config = |m: String| failure(RetryClass::AuthenticationOrConfiguration, &m, None);
        self.authorization.check().map_err(config)?;
        let policy = self.user.authorize(sources, self.roots);
        self.ledger
            .audit_payload(crate::budget_ledger::PayloadAudit {
                provider: provider.into(),
                payload_sha256: Digest::of_bytes(payload),
                source_roots: sources.to_vec(),
                policy: if policy.is_ok() { "allow" } else { "deny" }.into(),
            })
            .map_err(config)?;
        policy.map_err(config)?;
        let (url, header, secret) = self
            .user
            .endpoint(provider, model, self.keys)
            .map_err(config)?;
        let id = self
            .ledger
            .reserve(
                &self.authorization.scopes,
                Attempt {
                    id: crate::local::random_token(),
                    provider: provider.into(),
                    model: model.into(),
                    payload_sha256: Digest::of_bytes(payload),
                    source_roots: sources.to_vec(),
                    batch_size,
                    started_ms: crate::judge::now_ms(),
                    outcome: "reserved".into(),
                },
                probe,
            )
            .map_err(config)?;
        // Re-resolve aliases against the human-owned policy immediately before dispatch.
        if let Err(e) = self.user.authorize(sources, self.roots) {
            self.ledger
                .finish(&id, "not_dispatched", false, None)
                .map_err(config)?;
            return Err(config(e));
        }
        let auth = if header == "Authorization" {
            format!("Bearer {}", secret.expose())
        } else {
            secret.expose().into()
        };
        let reply = self.http.post(&url, (&header, &auth), payload, timeout);
        match reply {
            Ok(r) if (200..300).contains(&r.status) => Ok((r.body, id)),
            Ok(r) => {
                let class = if r.status == 429 {
                    RetryClass::RateLimited
                } else if r.status >= 500 {
                    RetryClass::Transient
                } else {
                    RetryClass::AuthenticationOrConfiguration
                };
                Err(ProviderFailure {
                    class,
                    message: format!("HTTP {} reservation={id}", r.status),
                    retry_after_secs: r.retry_after_secs,
                })
            }
            Err(_) => Err(failure(
                RetryClass::Transient,
                &format!("transport unavailable reservation={id}"),
                None,
            )),
        }
    }
    /// Production uses at most two retries, jittered backoff and a finite deadline.
    /// Pinned evaluation forbids substitution and lets its scheduler resume waits.
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        provider: &str,
        models: &[String],
        payload: impl Fn(&str) -> Result<Vec<u8>, String>,
        sources: &[String],
        batch_size: usize,
        deadline: Duration,
        pinned: bool,
    ) -> Result<Exchange, ProviderFailure> {
        let start = Instant::now();
        let mut attempts = Vec::new();
        let mut last = failure(
            RetryClass::AuthenticationOrConfiguration,
            "no approved models",
            None,
        );
        for model in models.iter().take(if pinned { 1 } else { models.len() }) {
            let bytes = payload(model)
                .map_err(|_| failure(RetryClass::InvalidResponse, "invalid payload", None))?;
            for retry in 0..=if pinned { 0 } else { 2 } {
                let remaining = deadline.saturating_sub(start.elapsed());
                if remaining.is_zero() {
                    return Err(failure(
                        RetryClass::Transient,
                        "deferred: deadline reached",
                        Some(5),
                    ));
                }
                match self.once(
                    provider,
                    model,
                    &bytes,
                    sources,
                    batch_size,
                    remaining,
                    retry == 0,
                ) {
                    Ok((body, id)) => {
                        self.ledger
                            .finish(&id, "answered", false, None)
                            .map_err(|_| {
                                failure(
                                    RetryClass::AuthenticationOrConfiguration,
                                    "cannot persist attempt outcome",
                                    None,
                                )
                            })?;
                        attempts.push(id);
                        return Ok(Exchange {
                            body,
                            model: model.clone(),
                            attempts,
                            latency_ms: start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(e) => {
                        let id = e.message.split("reservation=").nth(1).map(str::to_owned);
                        if let Some(id) = &id {
                            attempts.push(id.clone());
                        }
                        let transient =
                            matches!(e.class, RetryClass::Transient | RetryClass::RateLimited);
                        if !transient || retry == 2 || pinned {
                            if let Some(id) = id {
                                self.ledger
                                    .finish(
                                        &id,
                                        "unavailable",
                                        !transient,
                                        e.retry_after_secs.map(|s| {
                                            crate::judge::now_ms()
                                                .saturating_add(s.saturating_mul(1000))
                                        }),
                                    )
                                    .map_err(|_| {
                                        failure(
                                            RetryClass::AuthenticationOrConfiguration,
                                            "cannot persist failure",
                                            None,
                                        )
                                    })?;
                            }
                            if !transient {
                                return Err(e);
                            }
                            last = e;
                            break;
                        }
                        let jitter = u64::from(
                            Digest::of_bytes(&bytes)
                                .as_str()
                                .bytes()
                                .last()
                                .unwrap_or(0),
                        ) * 3;
                        let wait = Duration::from_millis((1_000u64 << retry) + jitter)
                            .max(Duration::from_secs(e.retry_after_secs.unwrap_or(0)));
                        if wait >= deadline.saturating_sub(start.elapsed()) {
                            if let Some(id) = id {
                                self.ledger
                                    .finish(
                                        &id,
                                        "unavailable",
                                        false,
                                        Some(
                                            crate::judge::now_ms()
                                                .saturating_add(wait.as_millis() as u64),
                                        ),
                                    )
                                    .map_err(|_| {
                                        failure(
                                            RetryClass::AuthenticationOrConfiguration,
                                            "cannot persist failure",
                                            None,
                                        )
                                    })?;
                            }
                            last = failure(
                                e.class,
                                "deferred: retry exceeds deadline",
                                Some(wait.as_secs().max(1)),
                            );
                            break;
                        }
                        // Failure remains consumed even if the process dies during backoff.
                        if let Some(id) = id {
                            self.ledger
                                .finish(&id, "unavailable", false, None)
                                .map_err(|_| {
                                    failure(
                                        RetryClass::AuthenticationOrConfiguration,
                                        "cannot persist failure",
                                        None,
                                    )
                                })?;
                        }
                        std::thread::sleep(wait);
                    }
                }
            }
        }
        Err(last)
    }
}
/// Canonical Jev adapter bridge. The coordinator owns provenance and limits.
pub struct JevTransport<'a> {
    /// Shared checked boundary.
    pub transport: &'a Transport<'a>,
    /// Actual pinned model.
    pub model: String,
    /// All transitive sources.
    pub sources: Vec<String>,
    /// Finite execution deadline.
    pub deadline: Duration,
}
impl super::observations::DecisionTransport for JevTransport<'_> {
    fn exchange(&self, payload: &[u8]) -> Result<(Vec<u8>, Option<Usage>), ProviderFailure> {
        let result = self.transport.execute(
            "jev",
            std::slice::from_ref(&self.model),
            |_| Ok(payload.to_vec()),
            &self.sources,
            1,
            self.deadline,
            false,
        )?;
        let usage = usage(&result.body);
        Ok((result.body, usage))
    }
}

/// Strict project settings cannot bind keys, endpoints or filesystem authority.
pub fn parse_project_policy(text: &str) -> Result<ProjectPolicy, String> {
    let policy: ProjectPolicy = toml::from_str(text).map_err(
        |_| "invalid project policy: endpoint, credential and authority fields are forbidden",
    )?;
    if policy
        .profile
        .as_ref()
        .is_some_and(|p| !["triage", "lookdev", "ci"].contains(&p.as_str()))
    {
        return Err("unknown review routing profile".into());
    }
    Ok(policy)
}
/// Execute a bounded R10 anonymous presentation through the same checked boundary.
/// The actual fallback identities are derived from persisted attempts, never model text.
#[allow(clippy::too_many_arguments)]
pub fn vision(
    transport: &Transport<'_>,
    case: &crate::evidence::case::EvidenceCase,
    presentation: &crate::judge_evidence::vision::VisionPresentation,
    policy: &super::observations::FallbackPolicy,
    sources: &[String],
    response_file: &Path,
    deadline: Duration,
    pinned: Option<&str>,
) -> Result<super::observations::CompletedVision, String> {
    let payload =
        super::observations::gemini_payload(case, presentation).map_err(|e| e.to_string())?;
    let active = pinned.map(|m| vec![m.to_owned()]);
    let exchange = transport
        .execute(
            "gemini",
            active.as_deref().unwrap_or(&policy.models),
            |_| Ok(payload.clone()),
            sources,
            1,
            deadline,
            false,
        )
        .map_err(|e| e.message)?;
    let attempts = transport.ledger.attempts()?;
    let outcomes: Vec<_> = exchange
        .attempts
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let a = attempts
                .iter()
                .find(|a| &a.id == id)
                .ok_or("missing vision reservation")?;
            let answered = i + 1 == exchange.attempts.len();
            Ok(super::observations::FallbackOutcome {
                model: a.model.clone(),
                answered,
                failure: if answered {
                    None
                } else {
                    Some(RetryClass::Transient)
                },
            })
        })
        .collect::<Result<_, String>>()?;
    let completed = super::observations::decode_gemini(
        case,
        presentation,
        super::observations::GeminiExchange {
            payload: &payload,
            response: &exchange.body,
            identity: crate::evidence::request::ProviderIdentity {
                provider: "gemini".into(),
                model: exchange.model,
                revision: None,
            },
            policy,
            outcomes: &outcomes,
            response_path: response_file,
        },
    )
    .map_err(|e| e.to_string())?;
    let parent = response_file
        .parent()
        .ok_or("invalid vision response output")?;
    std::fs::create_dir_all(parent).map_err(|_| "cannot create vision response directory")?;
    if std::fs::symlink_metadata(response_file).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("refusing response symlink".into());
    }
    std::fs::write(response_file, &exchange.body).map_err(|_| "cannot persist vision response")?;
    Ok(completed)
}
