//! Offline R11 acceptance: real child processes and injected HTTP replies.
#![cfg(feature = "ai")]
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use saccade_core::budget_ledger::{Attempt, Caps, Ledger, Scope};
use saccade_core::evidence::canonical::Digest;
use saccade_core::judge_provider::{Keys, transport::*};
use saccade_core::judge_stats::evaluation::{Observation, Outcome, metrics};
use saccade_core::root_policy::RootPolicy;
use serde_json::json;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Duration;

fn caps(total: u64, j: u64, g: u64) -> Caps {
    Caps {
        total,
        providers: BTreeMap::from([
            ("jev".into(), j),
            ("gemini".into(), g),
            ("custom".into(), total),
        ]),
    }
}
fn attempt(provider: &str) -> Attempt {
    Attempt {
        id: saccade_core::budget_ledger::new_id(),
        provider: provider.into(),
        model: "fixture-model".into(),
        payload_sha256: Digest::of_bytes(b"offline"),
        source_roots: vec!["capture".into()],
        batch_size: 8,
        started_ms: saccade_core::budget_ledger::now_ms(),
        outcome: "reserved".into(),
    }
}
#[test]
fn ledger_process_worker() {
    let Some(dir) = std::env::var_os("SACCADE_R11_LEDGER_WORKER") else {
        return;
    };
    let ledger = Ledger::new(std::path::Path::new(&dir), false);
    let scopes = vec![
        Scope {
            id: "run/shared".into(),
            caps: caps(10, 7, 7),
        },
        Scope {
            id: "project/shared".into(),
            caps: caps(9, 6, 6),
        },
        Scope {
            id: "parent/shared".into(),
            caps: caps(8, 4, 4),
        },
    ];
    let provider = std::env::var("SACCADE_R11_PROVIDER").unwrap();
    for _ in 0..8 {
        let _ = ledger.reserve(&scopes, attempt(&provider), false);
    }
    if std::env::var_os("SACCADE_R11_CRASH").is_some() {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(std::path::Path::new(&dir).join("production.lock"))
            .unwrap();
        fs2::FileExt::lock_exclusive(&lock).unwrap();
        std::process::exit(71);
    }
}
#[test]
fn concurrent_processes_never_exceed_run_project_parent_or_provider_caps() {
    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for i in 0..12 {
        children.push(
            std::process::Command::new(&exe)
                .args(["--exact", "ledger_process_worker", "--nocapture"])
                .env("SACCADE_R11_LEDGER_WORKER", dir.path())
                .env(
                    "SACCADE_R11_PROVIDER",
                    if i % 2 == 0 { "jev" } else { "gemini" },
                )
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let ledger = Ledger::new(dir.path(), false);
    let rows = ledger.attempts().unwrap();
    assert_eq!(ledger.used("run/shared").unwrap(), 8);
    assert_eq!(ledger.used("project/shared").unwrap(), 8);
    assert_eq!(ledger.used("parent/shared").unwrap(), 8);
    assert_eq!(rows.iter().filter(|a| a.provider == "jev").count(), 4);
    assert_eq!(rows.iter().filter(|a| a.provider == "gemini").count(), 4);
    assert!(rows.iter().all(|a| a.batch_size == 8));
}
#[test]
fn crashed_reservations_remain_consumed_and_os_lock_is_released() {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "ledger_process_worker"])
        .env("SACCADE_R11_LEDGER_WORKER", dir.path())
        .env("SACCADE_R11_PROVIDER", "jev")
        .env("SACCADE_R11_CRASH", "1")
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(71));
    let ledger = Ledger::new(dir.path(), false);
    assert_eq!(ledger.used("run/shared").unwrap(), 4);
    assert!(
        ledger
            .attempts()
            .unwrap()
            .iter()
            .all(|a| a.outcome == "reserved")
    );
    let scope = Scope {
        id: "run/shared".into(),
        caps: caps(100, 100, 100),
    };
    for _ in 0..3 {
        ledger
            .reserve(std::slice::from_ref(&scope), attempt("jev"), false)
            .unwrap();
    }
    assert!(ledger.reserve(&[scope], attempt("jev"), false).is_err());
    assert_eq!(Ledger::new(dir.path(), true).used("run/shared").unwrap(), 0);
}
struct Mock {
    replies: RefCell<Vec<Result<HttpReply, String>>>,
    sent: RefCell<Vec<(String, String)>>,
}
impl Http for Mock {
    fn post(
        &self,
        url: &str,
        header: (&str, &str),
        _: &[u8],
        _: Duration,
    ) -> Result<HttpReply, String> {
        self.sent.borrow_mut().push((url.into(), header.1.into()));
        self.replies.borrow_mut().remove(0)
    }
}
fn mock(statuses: &[u16]) -> Mock {
    Mock {
        replies: RefCell::new(
            statuses
                .iter()
                .map(|s| {
                    Ok(HttpReply {
                        status: *s,
                        retry_after_secs: None,
                        body: br#"{"ok":true}"#.to_vec(),
                    })
                })
                .collect(),
        ),
        sent: RefCell::new(vec![]),
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    user: UserConfig,
    roots: RootPolicy,
    auth: Authorization,
    ledger: Ledger,
    keys: Keys,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("captures");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(dir.path().join("jev.env"), "JEV_API_KEY=builtin-jev-secret").unwrap();
        std::fs::write(
            dir.path().join("gemini.env"),
            "SACCADE_GEMINI_API_KEY=builtin-gemini-secret",
        )
        .unwrap();
        std::fs::write(dir.path().join("custom.env"), "CUSTOM_KEY=dedicated-secret").unwrap();
        let user = UserConfig {
            roots: vec![RootSetting {
                id: "capture".into(),
                path: root.clone(),
                egress: EgressSetting::Allow,
            }],
            ..Default::default()
        };
        let mut roots = RootPolicy::new(&[root], None, false, &[]).unwrap();
        user.apply(&mut roots).unwrap();
        let auth = Authorization {
            enabled: true,
            scopes: vec![Scope {
                id: "run/fixture".into(),
                caps: caps(6, 3, 3),
            }],
        };
        let ledger = Ledger::new(&dir.path().join("ledger"), false);
        let keys = Keys::new(Some(dir.path().into()));
        Self {
            dir,
            user,
            roots,
            auth,
            ledger,
            keys,
        }
    }
    fn transport<'a>(&'a self, http: &'a dyn Http) -> Transport<'a> {
        Transport {
            user: &self.user,
            roots: &self.roots,
            authorization: &self.auth,
            ledger: &self.ledger,
            keys: &self.keys,
            http,
        }
    }
}
#[test]
fn startup_flag_and_positive_budget_are_both_required_before_dispatch() {
    let mut f = Fixture::new();
    let m = mock(&[]);
    f.auth.enabled = false;
    let e = f
        .transport(&m)
        .once(
            "jev",
            "model",
            b"{}",
            &["capture".into()],
            1,
            Duration::from_secs(1),
            true,
        )
        .unwrap_err();
    assert_eq!(e.message, "network_authorization_required");
    f.auth.enabled = true;
    f.auth.scopes[0].caps.total = 0;
    assert!(
        f.transport(&m)
            .once(
                "jev",
                "model",
                b"{}",
                &["capture".into()],
                1,
                Duration::from_secs(1),
                true
            )
            .is_err()
    );
    assert!(m.sent.borrow().is_empty());
    assert!(f.ledger.attempts().unwrap().is_empty());
}
#[test]
fn denied_unknown_incomplete_relocated_and_overlapping_provenance_dispatch_zero() {
    let mut f = Fixture::new();
    let m = mock(&[]);
    for sources in [
        vec![],
        vec!["unknown".into()],
        vec!["capture".into(), "unknown".into()],
    ] {
        assert!(
            f.transport(&m)
                .once(
                    "jev",
                    "model",
                    b"{}",
                    &sources,
                    1,
                    Duration::from_secs(1),
                    true
                )
                .is_err()
        );
    }
    f.user.roots.push(RootSetting {
        id: "denied-child".into(),
        path: f.user.roots[0].path.clone(),
        egress: EgressSetting::Deny,
    });
    assert!(
        f.transport(&m)
            .once(
                "jev",
                "model",
                b"{}",
                &["capture".into()],
                1,
                Duration::from_secs(1),
                true
            )
            .is_err()
    );
    assert!(m.sent.borrow().is_empty());
}
#[cfg(unix)]
#[test]
fn explicit_storage_alias_requires_permission_for_both_route_and_target() {
    let mut f = Fixture::new();
    let home = f.user.roots[0].path.clone();
    let storage = f.dir.path().join("storage");
    std::fs::create_dir(&storage).unwrap();
    std::fs::write(storage.join("image.png"), b"fixture").unwrap();
    let alias = home.join("nas");
    std::os::unix::fs::symlink(&storage, &alias).unwrap();
    f.roots = RootPolicy::new(
        std::slice::from_ref(&home),
        None,
        false,
        std::slice::from_ref(&storage),
    )
    .unwrap();
    f.user.roots.push(RootSetting {
        id: "storage".into(),
        path: storage,
        egress: EgressSetting::Deny,
    });
    f.user.apply(&mut f.roots).unwrap();
    let sources = saccade_core::paths::source_paths(&alias.join("image.png")).unwrap();
    assert_eq!(sources.len(), 2);
    let m = mock(&[200]);
    assert!(
        f.transport(&m)
            .once(
                "jev",
                "model",
                b"{}",
                &sources,
                1,
                Duration::from_secs(1),
                true
            )
            .is_err()
    );
    assert!(m.sent.borrow().is_empty());
    f.user.roots[1].egress = EgressSetting::Allow;
    f.user.apply(&mut f.roots).unwrap();
    f.transport(&m)
        .once(
            "jev",
            "model",
            b"{}",
            &sources,
            1,
            Duration::from_secs(1),
            true,
        )
        .unwrap();
    assert_eq!(m.sent.borrow().len(), 1);
    f.user.roots.push(RootSetting {
        id: "denied-alias".into(),
        path: alias,
        egress: EgressSetting::Deny,
    });
    assert!(
        f.transport(&m)
            .once(
                "jev",
                "model",
                b"{}",
                &sources,
                1,
                Duration::from_secs(1),
                true
            )
            .is_err()
    );
    assert_eq!(m.sent.borrow().len(), 1);
}
#[test]
fn project_nested_endpoint_and_key_bindings_are_rejected() {
    for key in [
        "base_url",
        "endpoint",
        "credential",
        "key_file",
        "key_var",
        "api_key",
        "env_var",
        "user_config",
        "allow_provider_calls",
    ] {
        assert!(reject_project_overrides(&json!({"nested":[{key:"injected"}]})).is_err());
        assert!(parse_project_policy(&format!("{key} = 'injected'")).is_err());
    }
    assert!(parse_project_policy("budget_calls=2\nproviders=['jev']\nmodel='jev-latest'").is_ok());
}
#[test]
fn custom_url_receives_only_dedicated_credentials_and_redirects_stop() {
    let mut f = Fixture::new();
    f.user.providers.insert(
        "custom".into(),
        CustomProvider {
            endpoint: "https://custom.invalid/chat/completions".into(),
            key_file: "custom.env".into(),
            key_var: "CUSTOM_KEY".into(),
        },
    );
    let m = mock(&[302]);
    assert!(
        f.transport(&m)
            .execute(
                "custom",
                &["model".into()],
                |_| Ok(b"{}".to_vec()),
                &["capture".into()],
                1,
                Duration::from_secs(1),
                false
            )
            .is_err()
    );
    assert_eq!(
        *m.sent.borrow(),
        vec![(
            "https://custom.invalid/chat/completions".into(),
            "Bearer dedicated-secret".into()
        )]
    );
    f.user.providers.get_mut("custom").unwrap().key_file = "jev.env".into();
    assert!(f.user.validate().is_err());
    let text = std::fs::read_to_string(f.dir.path().join("ledger/production.json")).unwrap();
    assert!(!text.contains("secret"));
}
#[test]
fn retries_429_503_and_success_count_each_http_request() {
    struct Clock(std::cell::Cell<Duration>);
    impl RetryClock for Clock {
        fn elapsed(&self) -> Duration {
            self.0.get()
        }
        fn sleep(&self, wait: Duration) {
            self.0.set(self.0.get() + wait);
        }
    }
    let clock = Clock(std::cell::Cell::new(Duration::ZERO));
    let f = Fixture::new();
    let m = mock(&[429, 503, 200]);
    let out = f
        .transport(&m)
        .execute_observed_with_clock(
            "jev",
            &["model".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            8,
            Duration::from_secs(5),
            false,
            &mut |_| {},
            &clock,
        )
        .unwrap();
    assert_eq!(out.attempts.len(), 3);
    assert!(clock.elapsed() >= Duration::from_secs(3));
    assert!(clock.elapsed() < Duration::from_secs(5));
    assert_eq!(out.latency_ms, clock.elapsed().as_millis() as u64);
    assert_eq!(f.ledger.used("run/fixture").unwrap(), 3);
    assert_eq!(m.sent.borrow().len(), 3);
    // An insufficient deadline still defers before dispatching a retry.
    let clock = Clock(std::cell::Cell::new(Duration::ZERO));
    let f = Fixture::new();
    let m = mock(&[429, 200]);
    let error = f
        .transport(&m)
        .execute_observed_with_clock(
            "jev",
            &["model".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            8,
            Duration::from_secs(1),
            false,
            &mut |_| {},
            &clock,
        )
        .unwrap_err();
    assert_eq!(
        error.class,
        saccade_core::decision_provider::RetryClass::RateLimited
    );
    assert!(error.message.contains("retry exceeds deadline"));
    assert_eq!(m.sent.borrow().len(), 1);
    assert_eq!(f.ledger.used("run/fixture").unwrap(), 1);
    assert_eq!(clock.elapsed(), Duration::ZERO);
}
#[test]
fn retry_after_and_timeouts_defer_and_fallback_stays_inside_shared_budget() {
    let f = Fixture::new();
    let m = Mock {
        sent: RefCell::new(vec![]),
        replies: RefCell::new(vec![
            Ok(HttpReply {
                status: 503,
                retry_after_secs: Some(300),
                body: vec![],
            }),
            Ok(HttpReply {
                status: 200,
                retry_after_secs: None,
                body: b"{}".to_vec(),
            }),
        ]),
    };
    let out = f
        .transport(&m)
        .execute(
            "gemini",
            &["first".into(), "fallback".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            1,
            Duration::from_secs(5),
            false,
        )
        .unwrap();
    assert_eq!(out.model, "fallback");
    assert_eq!(out.attempts.len(), 2);
    let timeout = Mock {
        sent: RefCell::new(vec![]),
        replies: RefCell::new(vec![Err("mock timeout".into())]),
    };
    let e = f
        .transport(&timeout)
        .execute(
            "jev",
            &["model".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            1,
            Duration::from_secs(1),
            false,
        )
        .unwrap_err();
    assert!(e.message.contains("deferred"));
}
#[test]
fn pinned_evaluation_never_substitutes_models() {
    let f = Fixture::new();
    let m = mock(&[503]);
    assert!(
        f.transport(&m)
            .execute(
                "gemini",
                &["first".into(), "fallback".into()],
                |_| Ok(b"{}".to_vec()),
                &["capture".into()],
                1,
                Duration::from_secs(1),
                true
            )
            .is_err()
    );
    assert_eq!(m.sent.borrow().len(), 1);
}
#[test]
fn shared_probe_deadline_allows_one_probe_and_authentication_stops_provider() {
    let f = Fixture::new();
    let a = attempt("jev");
    let id = f.ledger.reserve(&f.auth.scopes, a, true).unwrap();
    f.ledger.finish(&id, "unavailable", false, None).unwrap();
    let mut next = attempt("jev");
    next.started_ms += 301_000;
    let id = f
        .ledger
        .reserve(&f.auth.scopes, next.clone(), true)
        .unwrap();
    next.id = saccade_core::budget_ledger::new_id();
    assert!(f.ledger.reserve(&f.auth.scopes, next, true).is_err());
    f.ledger.finish(&id, "unavailable", true, None).unwrap();
    assert!(
        f.ledger
            .reserve(&f.auth.scopes, attempt("jev"), false)
            .is_err()
    );
}
fn reply(status: u16, retry_after_secs: Option<u64>, body: &[u8]) -> Result<HttpReply, String> {
    Ok(HttpReply {
        status,
        retry_after_secs,
        body: body.to_vec(),
    })
}
fn replies(list: Vec<Result<HttpReply, String>>) -> Mock {
    Mock {
        replies: RefCell::new(list),
        sent: RefCell::new(vec![]),
    }
}
/// R12 regression: Jev refused oversized batches with
/// `400 {"detail":{"error_type":"max_tokens_exceeded"}}`. The refusal must be
/// recorded as request-specific: it may not cool down or stop Jev for others.
#[test]
fn jev_token_limit_refusal_is_rejected_without_cooling_the_provider() {
    let f = Fixture::new();
    // Recorded shape (no key, no private data): closed questions over shared state.
    let payload = serde_json::to_vec(&json!({"model":"jev-latest",
        "state":{"requests":[{"question":{"id":"triage.route.v1"},"evidence":{"facts":["x".repeat(62_000)]}}]},
        "questions":{"q0":{"type":"choice","instructions":"Use state.requests[0].","criteria":{"a":"a"}}}}))
    .unwrap();
    let body = br#"{"detail":{"error_type":"max_tokens_exceeded"}}"#;
    let m = replies(vec![reply(400, None, body)]);
    let mut seen = Vec::new();
    let e = f
        .transport(&m)
        .execute_observed(
            "jev",
            &["jev-latest".into()],
            |_| Ok(payload.clone()),
            &["capture".into()],
            4,
            Duration::from_secs(5),
            false,
            &mut |a| seen.push(a.clone()),
        )
        .unwrap_err();
    assert!(e.message.starts_with("HTTP 400 "));
    assert_eq!(m.sent.borrow().len(), 1, "a request fault is never retried");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].status, Some(400));
    assert_eq!(seen[0].body, body);
    assert!(request_rejected(seen[0].status));
    let attempts = f.ledger.attempts().unwrap();
    assert_eq!(attempts.last().unwrap().outcome, "rejected");
    assert_eq!(f.ledger.retry_at("jev", "jev-latest").unwrap(), 0);
    let ok = mock(&[200]);
    f.transport(&ok)
        .execute(
            "jev",
            &["jev-latest".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            1,
            Duration::from_secs(5),
            false,
        )
        .unwrap();
    assert_eq!(ok.sent.borrow().len(), 1);
}
#[test]
fn rate_limits_back_off_on_the_same_model_honouring_retry_after_before_fallback() {
    let f = Fixture::new();
    let m = replies(vec![
        reply(429, Some(1), b"{}"),
        reply(503, None, b"{}"),
        reply(200, None, b"{}"),
    ]);
    let start = std::time::Instant::now();
    let mut seen = Vec::new();
    let out = f
        .transport(&m)
        .execute_observed(
            "gemini",
            &["first".into(), "fallback".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            1,
            Duration::from_secs(30),
            false,
            &mut |a| seen.push(a.clone()),
        )
        .unwrap();
    assert_eq!(out.model, "first", "backoff precedes fallback");
    assert_eq!(out.attempts.len(), 3);
    assert_eq!(
        seen.iter().map(|a| a.status).collect::<Vec<_>>(),
        vec![Some(429), Some(503)]
    );
    assert_eq!(seen[0].retry_after_secs, Some(1));
    // 1 s Retry-After (>= first backoff) plus the second, doubled backoff.
    assert!(start.elapsed() >= Duration::from_secs(3));
    assert_eq!(f.ledger.used("run/fixture").unwrap(), 3);
}
#[test]
fn structured_retry_delay_beyond_the_deadline_defers_without_another_dispatch() {
    let gemini_429 = br#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[
        {"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"119.2s"}]}}"#;
    assert_eq!(body_retry_delay(gemini_429), Some(120));
    assert_eq!(body_retry_delay(b"{}"), None);
    let f = Fixture::new();
    let m = replies(vec![reply(429, None, gemini_429), reply(200, None, b"{}")]);
    let e = f
        .transport(&m)
        .execute(
            "gemini",
            &["first".into()],
            |_| Ok(b"{}".to_vec()),
            &["capture".into()],
            1,
            Duration::from_secs(5),
            false,
        )
        .unwrap_err();
    assert!(e.message.starts_with("deferred"));
    assert!(e.retry_after_secs.unwrap() >= 120);
    assert_eq!(m.sent.borrow().len(), 1);
    assert!(
        f.ledger.retry_at("gemini", "first").unwrap()
            > saccade_core::budget_ledger::now_ms() + 100_000
    );
}
#[test]
fn pacing_limits_rate_and_concurrency_in_the_shared_ledger_without_consuming_budget() {
    use saccade_core::budget_ledger::PaceLimits;
    let f = Fixture::new();
    let pace = PaceLimits {
        provider_rpm: 2,
        provider_concurrency: 1,
        model_rpm: 2,
        model_concurrency: 1,
        lease_ms: 60_000,
    };
    let now = saccade_core::budget_ledger::now_ms();
    let at = |offset: u64| Attempt {
        started_ms: now - offset,
        ..attempt("gemini")
    };
    let first = f
        .ledger
        .reserve_paced(&f.auth.scopes, at(59_000), false, Some(&pace))
        .unwrap();
    let busy = f
        .ledger
        .reserve_paced(&f.auth.scopes, at(0), false, Some(&pace))
        .unwrap_err();
    assert!(busy.starts_with("paced retry_at="), "{busy}");
    f.ledger.finish(&first, "answered", false, None).unwrap();
    let second = f
        .ledger
        .reserve_paced(&f.auth.scopes, at(1_000), false, Some(&pace))
        .unwrap();
    f.ledger.finish(&second, "answered", false, None).unwrap();
    let full = f
        .ledger
        .reserve_paced(&f.auth.scopes, at(0), false, Some(&pace))
        .unwrap_err();
    assert_eq!(full, format!("paced retry_at={}", now - 59_000 + 60_001));
    assert_eq!(f.ledger.used("run/fixture").unwrap(), 2);
    // The transport waits inside its timeout, otherwise defers without dispatch.
    let mut paced = Fixture::new();
    paced.user.pacing.insert(
        "gemini".into(),
        PaceSetting {
            requests_per_minute: 1,
            concurrency: 1,
        },
    );
    let m = mock(&[200, 200]);
    let t = paced.transport(&m);
    let (_, id) = t
        .once(
            "gemini",
            "m",
            b"{}",
            &["capture".into()],
            1,
            Duration::from_secs(1),
            true,
        )
        .unwrap();
    paced.ledger.finish(&id, "answered", false, None).unwrap();
    let e = t
        .once(
            "gemini",
            "other",
            b"{}",
            &["capture".into()],
            1,
            Duration::from_secs(1),
            true,
        )
        .unwrap_err();
    assert!(e.message.starts_with("deferred retry_at="), "{}", e.message);
    assert_eq!(m.sent.borrow().len(), 1);
    assert_eq!(paced.ledger.used("run/fixture").unwrap(), 1);
    // Model entries only narrow the provider entry; zero limits are invalid.
    paced.user.pacing.insert(
        "gemini/m".into(),
        PaceSetting {
            requests_per_minute: 9,
            concurrency: 3,
        },
    );
    assert_eq!(paced.user.pace("gemini", "m").1.requests_per_minute, 1);
    paced.user.pacing.insert(
        "jev".into(),
        PaceSetting {
            requests_per_minute: 0,
            concurrency: 1,
        },
    );
    assert!(paced.user.validate().is_err());
}
fn observation(outcome: Outcome, answer: Option<&str>) -> Observation {
    Observation {
        case_id: "case".into(),
        question: "triage.route.v1".into(),
        provider: "jev".into(),
        model: "model".into(),
        vision_model: None,
        depends_on_model_observation: false,
        truth: Some("suspected_regression".into()),
        answer: answer.map(str::to_owned),
        probability: Some(0.9),
        outcome,
        attempts: 1,
        latency_ms: Some(10),
        usage: None,
        cost: None,
        order: None,
        critical_error: false,
    }
}
#[test]
fn conditional_accuracy_availability_abstention_and_operational_resolution_have_distinct_denominators()
 {
    let rows = vec![
        observation(Outcome::Answered, Some("suspected_regression")),
        observation(Outcome::Answered, Some("likely_noise")),
        observation(Outcome::Abstained, Some("abstain")),
        observation(Outcome::Unavailable, None),
        observation(Outcome::Invalid, None),
        observation(Outcome::Deferred, None),
        observation(Outcome::BudgetBlocked, None),
        observation(Outcome::Denied, None),
    ];
    let v = metrics(&rows);
    assert_eq!(v["conditional_accuracy"], 0.5);
    assert_eq!(v["availability"], 3.0 / 7.0);
    assert_eq!(v["answer_coverage"], 2.0 / 7.0);
    assert_eq!(v["correct_resolutions_over_eligible"], 1.0 / 7.0);
    assert_eq!(v["abstained"], 1);
    assert_eq!(v["independent_labelled_cases"], 1);
    let mut row = rows[0].clone();
    row.truth = None;
    assert!(metrics(&[row])["conditional_accuracy"].is_null());
}
#[cfg(feature = "evaluation")]
mod evaluator_tests {
    use super::*;
    use saccade_core::evidence::{Artifact, Document, canonical, case::*};
    use saccade_core::judge_bench::evaluator::{self, Manifest, Reference, Task};
    use std::path::Path;
    fn reference(file: &Path) -> Reference {
        Reference {
            path: file.file_name().unwrap().to_string_lossy().into(),
            sha256: Digest::of_bytes(&std::fs::read(file).unwrap()),
        }
    }
    fn setup(f: &Fixture) -> (std::path::PathBuf, Manifest) {
        let root = &f.user.roots[0].path;
        let document: Document =
            canonical::decode(include_bytes!("fixtures/evidence/case.json")).unwrap();
        let Artifact::Case(mut case) = document.artifact else {
            panic!("fixture")
        };
        case.requests.clear();
        case.proposals.clear();
        case.human_decisions.clear();
        case.next_actions.clear();
        for (n, input) in case.inputs.iter_mut().enumerate() {
            let file = root.join(format!("input{n}.bin"));
            std::fs::write(&file, [n as u8; 8]).unwrap();
            input.content = ArtifactRef {
                path: file.file_name().unwrap().to_string_lossy().into(),
                sha256: Digest::of_bytes(&std::fs::read(file).unwrap()),
            };
            input.sidecars.clear();
            input.provenance.source_roots = vec!["capture".into()];
        }
        case.provenance.source_roots = vec!["capture".into()];
        for name in ["deltas", "noise", "region_facts"] {
            case.facts.retain(|f| f.name != name);
            case.facts.push(Fact {
                id: format!("fixture-{name}"),
                name: name.into(),
                units: "fixture".into(),
                scope: case.scope.clone(),
                source: FactSource::Measured,
                artifact: case.measurement.report.clone(),
                source_identity: case.measurement.semantic_sha256.clone(),
                value: if name == "noise" {
                    Availability::missing("fixture evidence absent")
                } else {
                    Availability::Available {
                        value: FactValue::Text("{}".into()),
                    }
                },
                depends_on_model_observation: false,
                observation_refs: vec![],
            });
        }
        case.refresh_id().unwrap();
        saccade_core::judge_evidence::prepare_context(&mut case).unwrap();
        let request =
            saccade_core::judge_evidence::encode(&case, "triage.route.v1", Default::default())
                .unwrap();
        let case_file = root.join("case.json");
        std::fs::write(
            &case_file,
            canonical::bytes(&Document::new(Artifact::Case(case.clone()))).unwrap(),
        )
        .unwrap();
        let request_file = root.join("request.json");
        std::fs::write(
            &request_file,
            canonical::bytes(&Document::new(Artifact::DecisionRequest(Box::new(
                request.clone(),
            ))))
            .unwrap(),
        )
        .unwrap();
        let doc: Document =
            canonical::decode(include_bytes!("fixtures/evidence/human_decision.json")).unwrap();
        let Artifact::HumanDecision(mut human) = doc.artifact else {
            panic!("human fixture")
        };
        human.binding.case_id = case.case_id.clone();
        human.binding.input_hashes = case
            .inputs
            .iter()
            .map(|i| (i.id.clone(), i.content.sha256.clone()))
            .collect();
        human.binding.scope = case.scope.clone();
        human.binding.request_ids = vec![request.request_id.clone()];
        human.refresh_id().unwrap();
        let labels = saccade_core::evidence::human::Labels {
            schema: "saccade-labels.v2".into(),
            items: vec![saccade_core::evidence::human::Label {
                case_id: case.case_id.clone(),
                request_id: request.request_id.clone(),
                question_id: request.question.id,
                answer: "needs_eyes".into(),
                human_decision_id: human.decision_id.clone(),
                exposure: human.exposure.clone(),
            }],
        };
        let human_file = root.join("human.json");
        std::fs::write(
            &human_file,
            canonical::bytes(&Document::new(Artifact::HumanDecision(human))).unwrap(),
        )
        .unwrap();
        let label_file = root.join("labels.json");
        std::fs::write(&label_file, canonical::bytes(&labels).unwrap()).unwrap();
        let task = Task {
            case_id: case.case_id.clone(),
            request: reference(&request_file),
            case: reference(&case_file),
            provider: "jev".into(),
            model: "a-model".into(),
            fallback: vec![],
            labels: Some(reference(&label_file)),
            human_decision: Some(reference(&human_file)),
            split: "held_out".into(),
            order: None,
            synthetic: false,
        };
        let manifest = Manifest {
            schema: "saccade-evaluation.v1".into(),
            job: "score".into(),
            mode: "pinned_provider".into(),
            dataset_sha256: Digest::of_bytes(b"dataset"),
            split_sha256: Digest::of_bytes(b"split"),
            encoder_version: saccade_core::judge_evidence::ENCODER_VERSION.into(),
            adapter_version: "r11/1".into(),
            tasks: vec![task],
            budget_calls: 3,
            elapsed_ms: 10_000,
            window_end_unix_ms: saccade_core::budget_ledger::now_ms() + 600_000,
            retry_secs: vec![1],
            batch_size: 1,
            seed: 0,
            cache_policy: "resume".into(),
            gates: json!({"attempted_completion":0.95}),
            labeler_count: 1,
            exposure: json!({"saw_model_proposals":false}),
            test_retest: None,
            price: None,
        };
        (root.join("manifest.json"), manifest)
    }
    struct Replies {
        statuses: RefCell<Vec<u16>>,
        models: RefCell<Vec<String>>,
    }
    impl Http for Replies {
        fn post(
            &self,
            _: &str,
            _: (&str, &str),
            payload: &[u8],
            _: Duration,
        ) -> Result<HttpReply, String> {
            let body: serde_json::Value = serde_json::from_slice(payload).unwrap();
            let model = body["model"].as_str().unwrap();
            self.models.borrow_mut().push(model.into());
            let status = self.statuses.borrow_mut().remove(0);
            let answer = "needs_eyes";
            let answers = body["state"]["question"]["answers"].as_array().unwrap();
            let probabilities: serde_json::Map<String, serde_json::Value> = answers
                .iter()
                .map(|a| {
                    (
                        a.as_str().unwrap().to_owned(),
                        json!(if a == answer { 0.6 } else { 0.1 }),
                    )
                })
                .collect();
            Ok(HttpReply{status,retry_after_secs:None,body:json!({"model":model,"answers":{"q":{"choice":answer,"probabilities":probabilities}},"usage":{"input_tokens":10,"output_tokens":2}}).to_string().into_bytes()})
        }
    }
    fn run(
        f: &Fixture,
        mock: &Replies,
        m: &Manifest,
        file: &Path,
        state: &Path,
    ) -> serde_json::Value {
        let ledger = Ledger::new(&f.dir.path().join("ledger"), true);
        let mut auth = f.auth.clone();
        auth.scopes[0].id = format!("eval/{}", canonical::digest(m).unwrap().as_str());
        let transport = Transport {
            ledger: &ledger,
            authorization: &auth,
            ..f.transport(mock)
        };
        evaluator::run(m, file, state, &transport).unwrap()
    }
    #[test]
    fn failure_is_deferred_then_resumes_exact_job_without_replaying_completed_cells() {
        let f = Fixture::new();
        let (file, m) = setup(&f);
        let state = f.dir.path().join("resume.json");
        let mock = Replies {
            statuses: RefCell::new(vec![503, 200]),
            models: RefCell::new(vec![]),
        };
        let first = run(&f, &mock, &m, &file, &state);
        assert_eq!(first["execution"], "incomplete");
        assert_eq!(first["metrics"][0]["metrics"]["unavailable"], 0);
        assert_eq!(first["metrics"][0]["metrics"]["deferred"], 1);
        assert!(first["metrics"][0]["metrics"]["conditional_accuracy"].is_null());
        std::thread::sleep(Duration::from_millis(1010));
        let second = run(&f, &mock, &m, &file, &state);
        assert_eq!(second["metrics"][0]["metrics"]["conditional_accuracy"], 1.0);
        assert_eq!(second["metrics"][0]["metrics"]["attempts"], 2);
        let third = run(&f, &mock, &m, &file, &state);
        assert_eq!(third["metrics"], second["metrics"]);
        assert_eq!(mock.models.borrow().len(), 2);
        let mut changed = m.clone();
        changed.seed = 1;
        assert!(
            evaluator::run(
                &changed,
                &file,
                &state,
                &Transport {
                    ledger: &Ledger::new(&f.dir.path().join("ledger"), true),
                    ..f.transport(&mock)
                }
            )
            .is_err()
        );
    }
    #[test]
    fn evaluation_failure_finishes_its_own_reservation_when_another_job_appends() {
        struct Interleaved<'a> {
            ledger: &'a Ledger,
            replies: Replies,
            sibling: RefCell<Option<String>>,
        }
        impl Http for Interleaved<'_> {
            fn post(
                &self,
                url: &str,
                header: (&str, &str),
                payload: &[u8],
                timeout: Duration,
            ) -> Result<HttpReply, String> {
                let id = self
                    .ledger
                    .reserve(
                        &[saccade_core::budget_ledger::Scope {
                            id: "sibling-job".into(),
                            caps: caps(1, 1, 1),
                        }],
                        attempt("gemini"),
                        false,
                    )
                    .unwrap();
                *self.sibling.borrow_mut() = Some(id);
                self.replies.post(url, header, payload, timeout)
            }
        }
        let f = Fixture::new();
        let (file, m) = setup(&f);
        let ledger = Ledger::new(&f.dir.path().join("ledger"), true);
        let mock = Interleaved {
            ledger: &ledger,
            replies: Replies {
                statuses: RefCell::new(vec![503]),
                models: RefCell::new(vec![]),
            },
            sibling: RefCell::new(None),
        };
        let transport = Transport {
            ledger: &ledger,
            ..f.transport(&mock)
        };
        let result = evaluator::run(
            &m,
            &file,
            &f.dir.path().join("interleaved.json"),
            &transport,
        )
        .unwrap();
        assert_eq!(result["cells"][0]["observation"]["attempts"], 1);
        let attempts = ledger.attempts().unwrap();
        assert_eq!(
            attempts
                .iter()
                .find(|a| a.provider == "jev")
                .unwrap()
                .outcome,
            "unavailable"
        );
        assert_eq!(
            attempts
                .iter()
                .find(|a| Some(&a.id) == mock.sibling.borrow().as_ref())
                .unwrap()
                .outcome,
            "reserved"
        );
    }
    #[test]
    fn round_robin_keeps_models_fair_and_budget_exhaustion_incomplete() {
        let mut f = Fixture::new();
        let (file, mut m) = setup(&f);
        let mut b = m.tasks[0].clone();
        b.model = "b-model".into();
        m.tasks.push(m.tasks[0].clone());
        m.tasks.push(b);
        m.budget_calls = 3;
        f.auth.scopes[0].caps.total = 2;
        let mock = Replies {
            statuses: RefCell::new(vec![200, 200]),
            models: RefCell::new(vec![]),
        };
        let result = run(&f, &mock, &m, &file, &f.dir.path().join("fair.json"));
        assert_eq!(*mock.models.borrow(), vec!["a-model", "b-model"]);
        assert_eq!(result["execution"], "incomplete");
        assert_eq!(
            result["cells"][1]["observation"]["outcome"],
            "budget_blocked"
        );
    }
    #[test]
    fn plan_rejects_insufficient_budget_and_pinned_fallback_before_dispatch() {
        let f = Fixture::new();
        let (file, mut m) = setup(&f);
        m.tasks.push(m.tasks[0].clone());
        m.budget_calls = 1;
        let mock = Replies {
            statuses: RefCell::new(vec![]),
            models: RefCell::new(vec![]),
        };
        assert!(evaluator::plan(&m, &file, &f.transport(&mock)).is_err());
        m.budget_calls = 3;
        m.tasks[0].fallback = vec!["fallback".into()];
        assert!(evaluator::plan(&m, &file, &f.transport(&mock)).is_err());
        assert!(mock.models.borrow().is_empty());
    }
    #[test]
    fn calibration_only_fits_designated_split_and_synthetic_controls_have_no_quality_support() {
        let f = Fixture::new();
        let (file, mut m) = setup(&f);
        m.job = "calibrate".into();
        let mock = Replies {
            statuses: RefCell::new(vec![200]),
            models: RefCell::new(vec![]),
        };
        assert!(evaluator::plan(&m, &file, &f.transport(&mock)).is_err());
        m.tasks[0].split = "calibration".into();
        let result = run(&f, &mock, &m, &file, &f.dir.path().join("fit.json"));
        assert_eq!(result["calibration"]["fits"][0]["fit"]["fitted"], true);
        assert_eq!(
            result["calibration"]["fits"][0]["identity"]["split_hash"],
            serde_json::to_value(&m.split_sha256).unwrap()
        );
        assert_eq!(result["qualified"], false);
        m.tasks[0].synthetic = true;
        let mock = Replies {
            statuses: RefCell::new(vec![200]),
            models: RefCell::new(vec![]),
        };
        let result = run(&f, &mock, &m, &file, &f.dir.path().join("synthetic.json"));
        assert!(result["metrics"][0]["metrics"]["conditional_accuracy"].is_null());
    }
}
