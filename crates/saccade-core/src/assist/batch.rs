//! Durable frozen Batch lifecycle. An uncertain submit is never retried blindly.
use super::{Error, Result, decode, digest, read_bytes, require, write};
use crate::evidence::canonical::Digest;
use crate::judge_provider::batch as adapter;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Durable state, independent of a provider's job-level success label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Immutable plan, not yet dispatched.
    Planned,
    /// Accepted operation with known provider ID.
    Submitted,
    /// Dispatch intent persisted, but receipt missing or timeout ambiguous.
    SubmissionUnknown,
    /// Remote work still incomplete.
    Pending,
    /// Every frozen item succeeded and its identity validated.
    Completed,
    /// At least one item failed or is missing; success at job level is insufficient.
    Partial,
    /// Terminal provider or identity failure.
    Failed,
}
/// Immutable batch plan, paired with a separate mutable job receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// Exact pinned Gemini model.
    pub model: String,
    /// Bound observed revision; collect refuses drift.
    pub revision: String,
    /// Frozen provider inline requests with hashed item metadata.
    pub requests: Vec<Value>,
    /// Expiring price schedule.
    pub price_id: String,
    /// Explicit epoch budget; no automatic top-up.
    pub max_spend_nano_usd: u64,
}
impl Plan {
    /// Validate request hashes, duplicate identities and request bounds before submit.
    pub fn validate(&self) -> Result<()> {
        require(
            self.model == super::schema::GEMINI
                && !self.revision.is_empty()
                && self.requests.len() <= 128
                && !self.requests.is_empty()
                && self.price_id == super::execution::PRICE_ID
                && self.max_spend_nano_usd > 0
                && self.max_spend_nano_usd <= 250_000_000_000,
            "batch plan policy",
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for request in &self.requests {
            let id = request["metadata"]["job_id"]
                .as_str()
                .ok_or(Error::Invalid("batch item ID"))?;
            require(ids.insert(id), "duplicate batch item")?;
            let expected = adapter::inline_request(id, request["request"].clone())
                .map_err(|_| Error::Invalid("batch request"))?;
            require(
                expected == *request
                    && serde_json::to_vec(&request["request"])
                        .map_err(|_| Error::Invalid("batch serialization"))?
                        .len()
                        <= 64_000,
                "batch frozen request mismatch",
            )?;
        }
        Ok(())
    }
}
/// Persisted job receipt, never containing credentials or provider error text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Job {
    /// saccade-assist-batch.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-assist-batch.v1")))]
    pub schema: String,
    /// Exact immutable plan identity.
    pub plan_hash: Digest,
    /// Durable lifecycle.
    pub state: State,
    /// Provider operation ID when known.
    pub operation: Option<String>,
    /// Result identity and receipts per frozen item.
    pub results: Vec<Value>,
    /// Missing/failed item count; not hidden by job success.
    pub failed_items: usize,
    /// Last local transition time.
    pub updated_ms: u64,
}
fn operation(name: &str) -> Result<()> {
    require(
        name.strip_prefix("batches/").is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        }),
        "invalid provider operation ID",
    )
}
fn transaction<T>(path: &Path, change: impl FnOnce(&mut Job) -> Result<T>) -> Result<T> {
    let lock_path = path.with_extension("lock");
    require(
        !std::fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()),
        "batch lock symlink",
    )?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|_| Error::Storage)?;
    FileExt::lock_exclusive(&lock).map_err(|_| Error::Storage)?;
    let mut job: Job = decode(&read_bytes(path, 32 * 1024 * 1024)?)?;
    require(job.schema == "saccade-assist-batch.v1", "batch schema")?;
    let result = change(&mut job)?;
    job.updated_ms = crate::budget_ledger::now_ms();
    write(path, &job)?;
    Ok(result)
}
/// Create once. Existing jobs must have precisely the same plan; never truncate them.
pub fn plan(path: &Path, frozen: &Plan) -> Result<Job> {
    frozen.validate()?;
    let expected = digest(frozen)?;
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|_| Error::Storage)?;
    let initial = Job {
        schema: "saccade-assist-batch.v1".into(),
        plan_hash: expected.clone(),
        state: State::Planned,
        operation: None,
        results: vec![],
        failed_items: 0,
        updated_ms: crate::budget_ledger::now_ms(),
    };
    // Atomic create under the same stable lock used for transitions.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|_| Error::Storage)?;
    FileExt::lock_exclusive(&lock).map_err(|_| Error::Storage)?;
    if path.exists() {
        let existing: Job = decode(&read_bytes(path, 32 * 1024 * 1024)?)?;
        require(
            existing.schema == initial.schema && existing.plan_hash == expected,
            "existing batch plan changed",
        )?;
        return Ok(existing);
    }
    write(path, &initial)?;
    Ok(initial)
}
/// Persist dispatch intent before sending. A crash leaves submission_unknown.
/// The caller must first reserve the batch via the shared authorization/money path.
pub fn begin_submit(path: &Path, frozen: &Plan) -> Result<Vec<u8>> {
    frozen.validate()?;
    transaction(path, |job| {
        require(
            job.plan_hash == digest(frozen)? && job.state == State::Planned,
            "batch cannot resubmit; recover operation receipt",
        )?;
        let bytes = adapter::submission(&frozen.model, "assist-evaluation", &frozen.requests)
            .map_err(|_| Error::Invalid("batch payload"))?;
        job.state = State::SubmissionUnknown;
        Ok(bytes)
    })
}
/// Bind the durable provider operation receipt. Errors/timeouts keep uncertain state.
pub fn submitted(path: &Path, frozen: &Plan, name: &str) -> Result<()> {
    operation(name)?;
    transaction(path, |job| {
        require(
            job.plan_hash == digest(frozen)?
                && job.state == State::SubmissionUnknown
                && job.operation.is_none(),
            "batch receipt state",
        )?;
        job.operation = Some(name.into());
        job.state = State::Submitted;
        Ok(())
    })
}
/// Update/collect recorded or live responses against every exact frozen request.
/// Per-item errors are explicit; revision mismatch is a failed item, not qualified reuse.
pub fn collect(path: &Path, frozen: &Plan, response: &Value) -> Result<Job> {
    frozen.validate()?;
    transaction(path, |job| {
        require(
            job.plan_hash == digest(frozen)?
                && matches!(job.state, State::Submitted | State::Pending)
                && response["name"].as_str() == job.operation.as_deref(),
            "batch collection identity/state",
        )?;
        let state = response["metadata"]["state"]
            .as_str()
            .or_else(|| response["state"].as_str())
            .unwrap_or("");
        if [
            "BATCH_STATE_PENDING",
            "BATCH_STATE_RUNNING",
            "JOB_STATE_PENDING",
            "JOB_STATE_RUNNING",
        ]
        .contains(&state)
        {
            job.state = State::Pending;
            return Ok(job.clone());
        }
        let items = match adapter::collect(response, &frozen.requests) {
            Ok(items) => items,
            Err(_) => {
                job.state = State::Failed;
                job.failed_items = frozen.requests.len();
                return Ok(job.clone());
            }
        };
        job.results.clear();
        job.failed_items = 0;
        for (id, body) in items {
            let valid = body.get("batch_error_status").is_none()
                && body["modelVersion"].as_str() == Some(&frozen.revision);
            if !valid {
                job.failed_items += 1;
            }
            let bytes = serde_json::to_vec(&body).map_err(|_| Error::Invalid("batch item JSON"))?;
            let usage = super::execution::usage(&bytes);
            let cost = super::execution::cost_nano("gemini", &usage, job.updated_ms, true);
            job.results.push(serde_json::json!({"job_id":id,"ok":valid,"response_hash":Digest::of_bytes(&bytes),"usage":usage,"cost_nano_usd":cost,"response":if valid {body} else {Value::Null}}));
        }
        job.state = if job.failed_items == 0 {
            State::Completed
        } else {
            State::Partial
        };
        Ok(job.clone())
    })
}

/// Submit one frozen plan with fixed keys and existing egress, attempt and money policy.
/// Dispatch intent is durable before HTTP. Unknown outcomes require operation recovery.
pub fn submit_authorized(
    path: &Path,
    frozen: &Plan,
    executor: &super::execution::Executor<'_>,
    http: &dyn crate::judge_provider::batch::BatchHttp,
) -> Result<Job> {
    use crate::budget_ledger::{Attempt, MoneyReceipt, MoneyScope};
    frozen.validate()?;
    executor
        .transport
        .authorization
        .check()
        .map_err(|_| Error::Policy("batch authorization required"))?;
    executor
        .transport
        .user
        .authorize(&executor.sources, executor.transport.roots)
        .map_err(|_| Error::Policy("batch egress denied"))?;
    require(
        executor.transport.keys.default_policy_dir(),
        "batch fixed keys required",
    )?;
    let now = crate::budget_ledger::now_ms();
    require(
        now < super::execution::PRICE_EXPIRES_MS,
        "batch prices expired",
    )?;
    let payload = adapter::submission(&frozen.model, "assist-evaluation", &frozen.requests)
        .map_err(|_| Error::Invalid("batch payload"))?;
    let per_item = super::execution::cost_nano(
        "gemini",
        &super::schema::Usage {
            input_tokens: Some(super::execution::INPUT_LIMIT),
            candidate_tokens: Some(super::execution::OUTPUT_LIMIT),
            thinking_tokens: Some(0),
            ..Default::default()
        },
        now,
        true,
    )
    .ok_or(Error::Policy("batch reservation unknown"))?;
    let mut scopes = executor.money_scopes.clone();
    scopes.push(MoneyScope {
        id: format!("batch/{}", digest(frozen)?.as_str()),
        cap_nano_usd: frozen.max_spend_nano_usd,
    });
    let id = crate::local::random_token();
    executor
        .ledger
        .reserve_money(
            &scopes,
            MoneyReceipt {
                id: id.clone(),
                request_hash: Digest::of_bytes(&payload),
                scopes: scopes.iter().map(|s| s.id.clone()).collect(),
                reserved_nano_usd: per_item
                    .checked_mul(frozen.requests.len() as u64)
                    .ok_or(Error::Policy("batch reservation overflow"))?,
                actual_nano_usd: None,
                outcome: "reserved".into(),
                usage: Value::Null,
            },
        )
        .map_err(|_| Error::Policy("batch spend exhausted"))?;
    let attempt = executor
        .ledger
        .reserve(
            &executor.transport.authorization.scopes,
            Attempt {
                id: crate::local::random_token(),
                provider: "gemini".into(),
                model: frozen.model.clone(),
                payload_sha256: Digest::of_bytes(&payload),
                source_roots: executor.sources.clone(),
                batch_size: frozen.requests.len(),
                started_ms: now,
                outcome: "reserved".into(),
            },
            false,
        )
        .map_err(|_| Error::Policy("batch request budget exhausted"))?;
    let payload = begin_submit(path, frozen)?;
    let client = adapter::GeminiBatch {
        keys: executor.transport.keys,
        http,
    };
    match client.submit(&frozen.model, &payload) {
        Ok(response) => {
            let name = response["name"]
                .as_str()
                .ok_or(Error::Invalid("batch receipt missing"))?;
            submitted(path, frozen, name)?;
            executor
                .ledger
                .finish(&attempt, "batch_submitted", false, None)
                .map_err(|_| Error::Storage)?;
            // Monetary reservation stays consumed until all per-item costs are known.
            executor
                .ledger
                .finish_money(&id, None, Value::Null, true)
                .map_err(|_| Error::Storage)?;
            decode(&read_bytes(path, 32 * 1024 * 1024)?)
        }
        Err(_) => {
            executor
                .ledger
                .finish(&attempt, "submission_unknown", false, None)
                .map_err(|_| Error::Storage)?;
            executor
                .ledger
                .finish_money(&id, None, Value::Null, false)
                .map_err(|_| Error::Storage)?;
            Err(Error::Provider)
        }
    }
}
