//! Durable frozen Batch lifecycle. An uncertain submit is never retried blindly.
use super::{Error, Result, decode, digest, read_bytes, require, write};
use crate::evidence::canonical::Digest;
use crate::judge_provider::batch as adapter;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Public frozen source-plan schema; source bytes are verified before export.
pub const PLAN_SCHEMA: &str = "saccade-assist-batch-plan.v1";
/// Immutable ordinary-file reference, resolved through the user's root policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Absolute canonical source path; never a network URL.
    pub path: String,
    /// SHA-256 of the exact source bytes.
    pub sha256: Digest,
}
/// Reconstructible task descriptor and its exhaustive transitive read set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SourceTask {
    /// Existing assist workflow to reproduce locally.
    pub task: super::schema::Task,
    /// Original image or report reference.
    pub artifact: Reference,
    /// Selected report entry, when needed.
    pub entry: Option<String>,
    /// Original individual mask manifest.
    pub mask_manifest: Option<Reference>,
    /// Original producer packets.
    pub source_evidence: Vec<Reference>,
    /// Literal visible condition.
    pub condition_text: Option<String>,
    /// Closed visible condition category.
    pub condition_kind: Option<String>,
    /// Requested original-pixel scope.
    pub box_px: Option<[u32; 4]>,
    /// Stable source target.
    pub target: Option<String>,
    /// Stable second target.
    pub second_target: Option<String>,
    /// Producer declared incomplete scope.
    pub incomplete_capture: bool,
    /// Original pixels are unavailable.
    pub pre_masked: bool,
    /// Exact closure: report, screenshots, masks and every producer packet.
    pub transitive: Vec<Reference>,
    /// Frozen IDs for each blind order of this task.
    pub job_ids: Vec<String>,
}
/// Public plan includes sources rather than trusting embedded provider payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FrozenPlan {
    /// saccade-assist-batch-plan.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-assist-batch-plan.v1")))]
    pub schema: String,
    /// Provider requests and budget frozen before submit.
    pub plan: Plan,
    /// Reconstructible source tasks, verified transitively by CLI/MCP.
    pub tasks: Vec<SourceTask>,
}
impl FrozenPlan {
    /// Reject omitted, duplicate or unrelated task/request/source bindings.
    pub fn validate(&self) -> Result<()> {
        self.plan.validate()?;
        require(
            self.schema == PLAN_SCHEMA && !self.tasks.is_empty() && self.tasks.len() <= 128,
            "frozen source plan schema/count",
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for task in &self.tasks {
            require(
                !task.job_ids.is_empty()
                    && task.job_ids.len() <= 2
                    && task.transitive.len() <= 6
                    && !task.transitive.is_empty()
                    && task.source_evidence.len() <= 2,
                "source task bounds",
            )?;
            let mut paths = std::collections::BTreeSet::new();
            for source in &task.transitive {
                require(
                    std::path::Path::new(&source.path).is_absolute()
                        && source.path.len() <= 4096
                        && paths.insert(&source.path),
                    "source reference path/duplicate",
                )?;
            }
            for source in std::iter::once(&task.artifact)
                .chain(task.mask_manifest.iter())
                .chain(&task.source_evidence)
            {
                require(
                    task.transitive.contains(source),
                    "missing transitive source reference",
                )?;
            }
            for id in &task.job_ids {
                require(ids.insert(id.as_str()), "duplicate task request binding")?;
            }
        }
        let expected: std::collections::BTreeSet<_> = self
            .plan
            .requests
            .iter()
            .filter_map(|r| r["metadata"]["job_id"].as_str())
            .collect();
        require(ids == expected, "unbound frozen requests")
    }
}

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
                        <= 32 * 1024 * 1024,
                "batch frozen request mismatch",
            )?;
            super::price::gemini_bounds(
                &serde_json::to_vec(&request["request"])
                    .map_err(|_| Error::Invalid("batch request bytes"))?,
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
    /// Durable monetary reservation for settlement after collection.
    #[serde(default)]
    pub money_id: Option<String>,
    /// Original dispatch time binds prices across delayed collection.
    #[serde(default)]
    pub submitted_ms: Option<u64>,
    /// Canonical public source descriptor binding, when used by CLI/MCP.
    #[serde(default)]
    pub source_plan_hash: Option<Digest>,
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
        money_id: None,
        submitted_ms: None,
        source_plan_hash: None,
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
/// Bind transitive public sources once before dispatch; later operations refuse drift.
pub fn bind_sources(path: &Path, frozen: &FrozenPlan) -> Result<()> {
    frozen.validate()?;
    let hash = digest(&frozen.tasks)?;
    transaction(path, |job| {
        require(
            job.plan_hash == digest(&frozen.plan)?,
            "source plan job mismatch",
        )?;
        if let Some(existing) = &job.source_plan_hash {
            require(existing == &hash, "source plan changed")?;
        } else {
            require(
                job.state == State::Planned,
                "cannot rebind dispatched source plan",
            )?;
            job.source_plan_hash = Some(hash);
        }
        Ok(())
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
            let bytes = serde_json::to_vec(&body).map_err(|_| Error::Invalid("batch item JSON"))?;
            let usage = super::execution::usage(&bytes);
            let request = frozen
                .requests
                .iter()
                .find(|r| r["metadata"]["job_id"].as_str() == Some(&id))
                .ok_or(Error::Invalid("batch item request absent"))?;
            let valid = body.get("batch_error_status").is_none()
                && body["modelVersion"].as_str() == Some(&frozen.revision)
                && batch_answer_valid(&body, &request["request"]);
            if !valid {
                job.failed_items += 1;
            }

            let bounds = super::price::gemini_bounds(
                &serde_json::to_vec(&request["request"])
                    .map_err(|_| Error::Invalid("batch request bytes"))?,
            )?;
            let cost = {
                super::execution::cost_nano(
                    "gemini",
                    &usage,
                    job.submitted_ms.unwrap_or(job.updated_ms),
                    true,
                )
            };
            let breach = cost.is_some() && !bounds.contains(&usage);
            job.results.push(serde_json::json!({"job_id":id,"ok":valid,"bound_breach":breach,"response_hash":Digest::of_bytes(&bytes),"usage":usage,"cost_nano_usd":cost,"response":if valid {body} else {Value::Null}}));
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
    require(
        receipt(path, frozen)?.state == State::Planned,
        "batch cannot resubmit; recover operation receipt",
    )?;
    require(
        !executor
            .deadline
            .saturating_duration_since(std::time::Instant::now())
            .is_zero(),
        "batch submit deadline",
    )?;
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
    executor
        .transport
        .keys
        .load("gemini.env", "SACCADE_GEMINI_API_KEY")
        .map_err(|_| Error::Policy("fixed Gemini credentials unavailable"))?;
    let now = crate::budget_ledger::now_ms();
    require(
        now < super::execution::PRICE_EXPIRES_MS,
        "batch prices expired",
    )?;
    executor
        .transport
        .keys
        .load("gemini.env", "SACCADE_GEMINI_API_KEY")
        .map_err(|_| Error::Policy("batch credentials unavailable before reservation"))?;
    let payload = adapter::submission(&frozen.model, "assist-evaluation", &frozen.requests)
        .map_err(|_| Error::Invalid("batch payload"))?;
    let reservation = frozen.requests.iter().try_fold(0u64, |sum, request| {
        let bytes = serde_json::to_vec(&request["request"])
            .map_err(|_| Error::Invalid("batch request bytes"))?;
        let bounds = super::price::gemini_bounds(&bytes)?;
        let cost = super::execution::cost_nano(
            "gemini",
            &super::schema::Usage {
                input_tokens: Some(bounds.input),
                candidate_tokens: Some(bounds.output),
                thinking_tokens: Some(0),
                total_tokens: Some(bounds.input + bounds.output),
                ..Default::default()
            },
            now,
            true,
        )
        .ok_or(Error::Policy("batch reservation unknown"))?;
        sum.checked_add(cost)
            .ok_or(Error::Policy("batch reservation overflow"))
    })?;
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
                reserved_nano_usd: reservation,
                actual_nano_usd: None,
                outcome: "reserved".into(),
                usage: Value::Null,
            },
        )
        .map_err(|_| Error::Policy("batch spend exhausted"))?;
    let (provider_pace, model_pace) = executor.transport.user.pace("gemini", &frozen.model);
    let pace = crate::budget_ledger::PaceLimits {
        provider_rpm: provider_pace.requests_per_minute,
        provider_concurrency: provider_pace.concurrency,
        model_rpm: model_pace.requests_per_minute,
        model_concurrency: model_pace.concurrency,
        lease_ms: 300_000,
    };
    let attempt = executor
        .ledger
        .reserve_paced(
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
            Some(&pace),
        )
        .map_err(|_| {
            let _ = executor.ledger.finish_money(
                &id,
                Some(0),
                serde_json::json!({"not_dispatched":true}),
                false,
            );
            Error::Policy("batch request budget exhausted")
        })?;
    let payload = begin_submit(path, frozen)?;
    transaction(path, |job| {
        job.money_id = Some(id.clone());
        job.submitted_ms = Some(now);
        Ok(())
    })?;
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
                .finish(&attempt, "answered", false, None)
                .map_err(|_| Error::Storage)?;
            // Keep the receipt reserved until a terminal collection settles it.
            decode(&read_bytes(path, 32 * 1024 * 1024)?)
        }
        Err(_) => {
            executor
                .ledger
                .finish(&attempt, "unavailable", false, None)
                .map_err(|_| Error::Storage)?;
            executor
                .ledger
                .finish_money(&id, None, Value::Null, false)
                .map_err(|_| Error::Storage)?;
            Err(Error::Provider)
        }
    }
}

/// Read and validate a local receipt against its exact frozen plan.
pub fn receipt(path: &Path, frozen: &Plan) -> Result<Job> {
    frozen.validate()?;
    let job: Job = decode(&read_bytes(path, 32 * 1024 * 1024)?)?;
    require(
        job.schema == "saccade-assist-batch.v1" && job.plan_hash == digest(frozen)?,
        "batch receipt plan mismatch",
    )?;
    Ok(job)
}
/// Poll once under the shared authorization, transitive egress and attempt policy.
/// Status never waits or resubmits. Raw provider errors are not persisted.
pub fn poll_authorized(
    path: &Path,
    frozen: &Plan,
    executor: &super::execution::Executor<'_>,
    http: &dyn adapter::BatchHttp,
) -> Result<Value> {
    use crate::budget_ledger::Attempt;
    executor
        .transport
        .authorization
        .check()
        .map_err(|_| Error::Policy("batch polling authorization"))?;
    executor
        .transport
        .user
        .authorize(&executor.sources, executor.transport.roots)
        .map_err(|_| Error::Policy("batch polling egress"))?;
    require(
        executor.transport.keys.default_policy_dir(),
        "batch fixed keys required",
    )?;
    let job = receipt(path, frozen)?;
    let name = job.operation.as_deref().ok_or(Error::Invalid(
        "unknown batch operation; recover original receipt",
    ))?;
    operation(name)?;
    let timeout = executor
        .deadline
        .saturating_duration_since(std::time::Instant::now());
    require(
        !timeout.is_zero() && timeout <= std::time::Duration::from_secs(300),
        "batch poll deadline",
    )?;
    let (provider, model) = executor.transport.user.pace("gemini", &frozen.model);
    let pace = crate::budget_ledger::PaceLimits {
        provider_rpm: provider.requests_per_minute,
        provider_concurrency: provider.concurrency,
        model_rpm: model.requests_per_minute,
        model_concurrency: model.concurrency,
        lease_ms: 300_000,
    };
    let attempt = executor
        .ledger
        .reserve_paced(
            &executor.transport.authorization.scopes,
            Attempt {
                id: crate::local::random_token(),
                provider: "gemini".into(),
                model: frozen.model.clone(),
                payload_sha256: digest(&serde_json::json!({"get":name}))?,
                source_roots: executor.sources.clone(),
                batch_size: 1,
                started_ms: crate::budget_ledger::now_ms(),
                outcome: "reserved".into(),
            },
            false,
            Some(&pace),
        )
        .map_err(|_| Error::Policy("batch polling request cap or pacing"))?;
    let client = adapter::GeminiBatch {
        keys: executor.transport.keys,
        http,
    };
    let result = client.get(name);
    executor
        .ledger
        .finish(
            &attempt,
            if result.is_ok() {
                "answered"
            } else {
                "unavailable"
            },
            false,
            None,
        )
        .map_err(|_| Error::Storage)?;
    let response = result.map_err(|_| Error::Provider)?;
    require(
        response["name"].as_str() == Some(name),
        "batch poll operation mismatch",
    )?;
    Ok(response)
}
/// Settle once after terminal collection. Unknown or failed cells retain the full charge.
/// A crash between collection and settlement can retry settlement without another HTTP.
pub fn settle(path: &Path, frozen: &Plan, ledger: &crate::budget_ledger::Ledger) -> Result<Job> {
    let job = receipt(path, frozen)?;
    require(
        matches!(job.state, State::Completed | State::Partial | State::Failed),
        "batch settlement before terminal collection",
    )?;
    if let Some(id) = &job.money_id {
        let receipts = ledger.money_receipts().map_err(|_| Error::Storage)?;
        let monetary = receipts
            .iter()
            .find(|r| &r.id == id)
            .ok_or(Error::Invalid("batch monetary receipt absent"))?;
        if monetary.outcome == "reserved" {
            if job.results.iter().any(|r| r["bound_breach"] == true) {
                ledger.stop_spending().map_err(|_| Error::Storage)?;
            }
            let actual = if job.results.len() == frozen.requests.len() {
                job.results.iter().try_fold(0u64, |sum, item| {
                    sum.checked_add(item["cost_nano_usd"].as_u64()?)
                })
            } else {
                None
            };
            ledger.finish_money(id,actual,serde_json::json!({"bound_breach":job.results.iter().any(|r|r["bound_breach"]==true),"items":job.results.iter().map(|r|serde_json::json!({"job_id":r["job_id"],"usage":r["usage"],"cost_nano_usd":r["cost_nano_usd"]})).collect::<Vec<_>>()}),job.state==State::Completed).map_err(|_|Error::Storage)?;
        }
    }
    Ok(job)
}

fn batch_answer_valid(body: &Value, request: &Value) -> bool {
    if body["candidates"].as_array().is_none_or(|c| c.len() != 1)
        || body["candidates"][0]["finishReason"] != "STOP"
    {
        return false;
    }
    let Some(parts) = body["candidates"][0]["content"]["parts"].as_array() else {
        return false;
    };
    let text: String = parts
        .iter()
        .filter(|p| p["thought"] != true)
        .filter_map(|p| p["text"].as_str())
        .collect();
    let Some(binding) = request["contents"][0]["parts"][0]["text"]
        .as_str()
        .and_then(|s| decode::<Value>(s.as_bytes()).ok())
    else {
        return false;
    };
    let Ok(answer) = decode::<super::workflow::WireAnswer>(text.as_bytes()) else {
        return false;
    };
    if binding["request_hash"].as_str() != Some(answer.request_hash.as_str())
        || (answer.outcome != super::schema::Outcome::Unverifiable
            && answer.observations.is_empty())
    {
        return false;
    }
    answer.observations.iter().all(|o| {
        let view = binding["views"]
            .as_array()
            .and_then(|views| views.iter().find(|v| v["slot"] == o.slot));
        let Some(view) = view else {
            return false;
        };
        let Some(dimensions) = view["dimensions"].as_array().filter(|d| d.len() == 2) else {
            return false;
        };
        let Some(width) = dimensions[0].as_u64().and_then(|n| u32::try_from(n).ok()) else {
            return false;
        };
        let Some(height) = dimensions[1].as_u64().and_then(|n| u32::try_from(n).ok()) else {
            return false;
        };
        let Ok(transform) =
            serde_json::from_value::<super::geometry::Transform>(view["transform"].clone())
        else {
            return false;
        };
        let Ok(geometry) = transform.from_normalized(&o.geometry, [width, height]) else {
            return false;
        };
        let Ok(capture) = serde_json::from_value::<[u32; 4]>(view["capture_scope"].clone()) else {
            return false;
        };
        super::workflow::contains(&geometry, capture)
            && o.statement.len() <= 1024
            && super::workflow::validate_statement(o.kind, &o.statement).is_ok()
            && o.uncertainty.is_finite()
            && (0.0..=1.0).contains(&o.uncertainty)
            && !o.evidence_refs.is_empty()
            && o.evidence_refs.len() <= 16
            && o.evidence_refs.iter().all(|reference| {
                view["regions"].as_array().is_some_and(|regions| {
                    regions.iter().any(|r| {
                        r["id"] == *reference
                            && serde_json::from_value::<[u32; 4]>(r["rect_px"].clone())
                                .is_ok_and(|rect| super::workflow::intersects(&geometry, rect))
                    })
                })
            })
    })
}
