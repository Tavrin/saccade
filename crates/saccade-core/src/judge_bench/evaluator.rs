//! Manifest-bound, resumable evaluation. Model failures are operational outcomes,
//! never invented labels or incorrect committed answers.
use crate::decision_provider::{ProviderResponse, RetryClass};
use crate::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    request::{DecisionRequest, ProviderIdentity},
};
use crate::judge_provider::{observations, transport::Transport};
use crate::judge_stats::evaluation::{Observation, Outcome};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// Exact local reference, hash-checked before planning or dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Portable path relative to the manifest.
    pub path: String,
    /// Exact file bytes.
    pub sha256: Digest,
}
impl Reference {
    /// Verify local content; endpoint-looking paths and traversal are not authorities.
    pub fn read(&self, manifest: &Path) -> Result<Vec<u8>> {
        if self.path.contains('\\') || self.path.contains("://") {
            return Err(Error::Config(
                "evaluation references must be local portable paths".into(),
            ));
        }
        let path = crate::paths::resolve(&self.path, manifest);
        let path = crate::paths::canonicalize(path)
            .map_err(crate::run::io_err("evaluation reference".into()))?;
        let bytes =
            std::fs::read(path).map_err(crate::run::io_err("evaluation reference".into()))?;
        if Digest::of_bytes(&bytes) != self.sha256 {
            return Err(Error::Config("stale evaluation reference".into()));
        }
        Ok(bytes)
    }
}
/// One applicable question/case/provider/order cell; no synthetic controls count as real support.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Independent change case identity, checked against the request.
    pub case_id: Digest,
    /// Exact canonical question.
    pub request: Reference,
    /// Original case, carrying transitive provenance.
    pub case: Reference,
    /// Approved provider identifier, never an endpoint.
    pub provider: String,
    /// Pinned requested model.
    pub model: String,
    /// Only production-policy replay can use these models.
    #[serde(default)]
    pub fallback: Vec<String>,
    /// Human held-out or calibration label, exact canonical labels.v2 document.
    pub labels: Option<Reference>,
    /// Human decision record that the label refers to.
    pub human_decision: Option<Reference>,
    /// Dataset split: calibration or held_out.
    pub split: String,
    /// Presentation order identity.
    pub order: Option<String>,
    /// Synthetic controls can test denominators but provide no real support.
    #[serde(default)]
    pub synthetic: bool,
}
/// Reproducible score/calibrate/conformance job; strict fields reject endpoint/key injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// saccade-evaluation.v1.
    pub schema: String,
    /// score, calibrate or conformance.
    pub job: String,
    /// pinned_provider or production_policy.
    pub mode: String,
    /// Exact corpus identity, supplied by its owner.
    pub dataset_sha256: Digest,
    /// Exact split assignment identity.
    pub split_sha256: Digest,
    /// Fixed catalog/encoder identity.
    pub encoder_version: String,
    /// Adapter/routing version.
    pub adapter_version: String,
    /// Exact evaluation cells.
    pub tasks: Vec<Task>,
    /// Global attempts, including every retry/fallback.
    pub budget_calls: u64,
    /// Finite per-invocation execution window; pending items are retained.
    pub elapsed_ms: u64,
    /// Absolute resume window end.
    pub window_end_unix_ms: u64,
    /// Explicit retry schedule, normally 5,15,45,120,300 seconds.
    pub retry_secs: Vec<u64>,
    /// Adapter batch size; canonical adapters currently support one cell.
    pub batch_size: usize,
    /// Reproducible round-robin starting point.
    pub seed: u64,
    /// resume or refresh. Refresh requires a new manifest identity/job budget.
    pub cache_policy: String,
    /// Coverage and task qualification tolerances; never self-certifies qualification.
    pub gates: Value,
    /// Human labeler count, published separately from quality.
    pub labeler_count: usize,
    /// Recorded reviewer exposure conditions.
    pub exposure: Value,
    /// Test-retest plan/reference, explicit unknown when absent.
    pub test_retest: Option<Reference>,
    /// Pinned price metadata, null means unknown.
    pub price: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cell {
    observation: Observation,
    retry_at: Option<u64>,
    retries: usize,
    next_model: usize,
    complete: bool,
    #[serde(default)]
    calibrator: Option<crate::review::CalibratorIdentity>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    manifest_id: Digest,
    cells: Vec<Cell>,
}
fn config(e: impl std::fmt::Display) -> Error {
    Error::Config(e.to_string())
}
/// Load JSON or TOML without accepting arbitrary security fields.
pub fn read(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path)
        .map_err(crate::run::io_err("reading evaluation manifest".into()))?;
    let m: Manifest = if path.extension().is_some_and(|e| e == "toml") {
        toml::from_str(&text).map_err(config)?
    } else {
        serde_json::from_str(&text)?
    };
    validate(&m)?;
    Ok(m)
}
fn validate(m: &Manifest) -> Result<()> {
    if m.schema != "saccade-evaluation.v1"
        || !["score", "calibrate", "conformance"].contains(&m.job.as_str())
        || !["pinned_provider", "production_policy"].contains(&m.mode.as_str())
        || m.tasks.is_empty()
        || m.budget_calls == 0
        || m.elapsed_ms == 0
        || m.window_end_unix_ms == 0
        || m.retry_secs.is_empty()
        || m.retry_secs.len() > 5
        || m.retry_secs.contains(&0)
        || m.batch_size != 1
        || !["resume", "refresh"].contains(&m.cache_policy.as_str())
        || m.encoder_version != crate::judge_evidence::ENCODER_VERSION
        || m.adapter_version.is_empty()
        || !m.gates.is_object()
    {
        return Err(config("invalid evaluation manifest contract"));
    }
    for t in &m.tasks {
        if t.provider.is_empty()
            || t.model.is_empty()
            || !["held_out", "calibration"].contains(&t.split.as_str())
            || (m.mode == "pinned_provider" && !t.fallback.is_empty())
            || (m.job == "calibrate" && t.split != "calibration")
            || t.labels.is_some() != t.human_decision.is_some()
        {
            return Err(config(
                "invalid evaluation task or pinned fallback/calibration split",
            ));
        }
    }
    crate::judge_provider::transport::reject_project_overrides(&serde_json::to_value(m)?)
        .map_err(config)
}
fn request(
    t: &Task,
    file: &Path,
) -> Result<(DecisionRequest, crate::evidence::case::EvidenceCase)> {
    let d: Document = canonical::decode(&t.request.read(file)?).map_err(config)?;
    d.validate().map_err(config)?;
    let Artifact::DecisionRequest(r) = d.artifact else {
        return Err(config("evaluation task requires a canonical request"));
    };
    let d: Document = canonical::decode(&t.case.read(file)?).map_err(config)?;
    d.validate().map_err(config)?;
    let Artifact::Case(c) = d.artifact else {
        return Err(config("evaluation task requires a canonical case"));
    };
    r.validate_for(&c).map_err(config)?;
    crate::questions::validate_request(&r).map_err(config)?;
    if t.case_id != r.case_id {
        return Err(config("evaluation task case mismatch"));
    }
    Ok((*r, *c))
}
/// Retain all declared transitive sources. Every input must carry provenance.
pub fn sources(case: &crate::evidence::case::EvidenceCase) -> Vec<String> {
    if case.provenance.source_roots.is_empty()
        || case
            .inputs
            .iter()
            .any(|i| i.provenance.source_roots.is_empty())
    {
        return vec![];
    }
    case.provenance
        .source_roots
        .iter()
        .chain(case.inputs.iter().flat_map(|i| &i.provenance.source_roots))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
/// Check provenance and the actual transitive files, including relocated references.
pub fn dispatch_sources(
    case: &crate::evidence::case::EvidenceCase,
    document: &Path,
) -> Result<Vec<String>> {
    let mut roots = sources(case);
    if roots.is_empty() {
        return Ok(roots);
    }
    for input in &case.inputs {
        for reference in std::iter::once(&input.content).chain(&input.sidecars) {
            reference.verify(document).map_err(config)?;
            roots.extend(
                crate::paths::source_paths(&crate::paths::resolve(&reference.path, document))
                    .map_err(config)?,
            );
        }
    }
    Ok(roots)
}
fn truth(t: &Task, r: &DecisionRequest, file: &Path) -> Result<Option<String>> {
    let (Some(labels), Some(human)) = (&t.labels, &t.human_decision) else {
        return Ok(None);
    };
    let labels: crate::evidence::human::Labels =
        canonical::decode(&labels.read(file)?).map_err(config)?;
    let doc: Document = canonical::decode(&human.read(file)?).map_err(config)?;
    doc.validate().map_err(config)?;
    let Artifact::HumanDecision(h) = doc.artifact else {
        return Err(config("labels need a human decision"));
    };
    if labels.schema != crate::labels::QUESTION_LABELS_SCHEMA {
        return Err(config("labels must use the canonical labels.v2 contract"));
    }
    for label in &labels.items {
        label.validate_for(r, &h).map_err(config)?;
    }
    if t.synthetic {
        return Ok(None);
    }
    Ok(labels
        .items
        .iter()
        .find(|l| l.request_id == r.request_id)
        .map(|l| l.answer.clone()))
}
/// Planning verifies immutable evidence and excludes denied cells from required attempts.
pub fn plan(m: &Manifest, file: &Path, transport: &Transport<'_>) -> Result<Value> {
    validate(m)?;
    let mut permitted = 0u64;
    let mut cells = Vec::new();
    for t in &m.tasks {
        let (r, c) = request(t, file)?;
        truth(t, &r, file)?;
        let roots = dispatch_sources(&c, &crate::paths::resolve(&t.case.path, file))?;
        let allowed = transport.user.authorize(&roots, transport.roots).is_ok();
        permitted += u64::from(allowed);
        cells.push(json!({"case_id":t.case_id,"question":r.question.id,"provider":t.provider,"model":t.model,"source_roots":roots,"policy":if allowed{"allow"}else{"deny"},"payload_sha256":Digest::of_bytes(&payload(&r,&t.provider,&t.model)?)}));
    }
    if permitted > m.budget_calls {
        return Err(config("evaluation plan exceeds its attempt budget"));
    }
    Ok(
        json!({"schema":"saccade-evaluation.v1","job":m.job,"mode":m.mode,"manifest_id":canonical::digest(m).map_err(config)?,"initial_requests":permitted,"maximum_attempts":m.budget_calls,"batch_size":m.batch_size,"cells":cells,"qualified":false}),
    )
}
fn payload(r: &DecisionRequest, provider: &str, model: &str) -> Result<Vec<u8>> {
    if provider == "jev" {
        observations::jev_payload(r, model).map_err(config)
    } else if provider == "gemini" {
        Err(config(
            "Gemini evaluation requires an R10 visual presentation, not a text decision request",
        ))
    } else {
        canonical::bytes(r).map_err(config)
    }
}
fn save(file: &Path, state: &State) -> Result<()> {
    use std::io::Write;
    let parent = file.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(crate::run::io_err("evaluation state".into()))?;
    tmp.write_all(&canonical::bytes(state).map_err(config)?)
        .map_err(crate::run::io_err("evaluation state".into()))?;
    tmp.as_file()
        .sync_all()
        .map_err(crate::run::io_err("evaluation state".into()))?;
    tmp.persist(file).map_err(config)?;
    Ok(())
}
/// Run or resume round-robin cells. No fallback occurs in pinned evaluation;
/// waits remain in state rather than blocking the process for minutes.
pub fn run(
    m: &Manifest,
    file: &Path,
    state_file: &Path,
    transport: &Transport<'_>,
) -> Result<Value> {
    use fs2::FileExt;
    transport.authorization.check().map_err(config)?;
    plan(m, file, transport)?;
    let parent = state_file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(crate::run::io_err("evaluation state directory".into()))?;
    if std::fs::symlink_metadata(state_file).is_ok_and(|f| f.file_type().is_symlink()) {
        return Err(config("refusing evaluation state symlink"));
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(state_file.with_extension("lock"))
        .map_err(crate::run::io_err("evaluation lock".into()))?;
    FileExt::lock_exclusive(&lock).map_err(crate::run::io_err("evaluation lock".into()))?;
    let id = canonical::digest(m).map_err(config)?;
    let mut state: State = if state_file.exists() {
        canonical::decode(
            &std::fs::read(state_file).map_err(crate::run::io_err("evaluation state".into()))?,
        )
        .map_err(config)?
    } else {
        let mut cells = Vec::new();
        for t in &m.tasks {
            let (r, _) = request(t, file)?;
            cells.push(Cell {
                observation: Observation {
                    case_id: t.case_id.as_str().into(),
                    question: r.question.id.clone(),
                    provider: t.provider.clone(),
                    model: t.model.clone(),
                    vision_model: r
                        .evidence
                        .observation_context
                        .as_ref()
                        .map(|c| c.extractor.model.clone()),
                    depends_on_model_observation: !r.evidence.observations.is_empty(),
                    truth: truth(t, &r, file)?,
                    answer: None,
                    probability: None,
                    outcome: Outcome::Deferred,
                    attempts: 0,
                    latency_ms: None,
                    usage: None,
                    cost: None,
                    order: t.order.clone(),
                    critical_error: false,
                },
                retry_at: None,
                retries: 0,
                next_model: 0,
                complete: false,
                calibrator: None,
            });
        }
        State {
            manifest_id: id.clone(),
            cells,
        }
    };
    if state.manifest_id != id || state.cells.len() != m.tasks.len() {
        return Err(config("evaluation resume identity differs"));
    }
    let started = std::time::Instant::now();
    let mut schedule: std::collections::BTreeMap<_, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, c) in state.cells.iter().enumerate() {
        schedule
            .entry((c.observation.model.clone(), c.observation.question.clone()))
            .or_default()
            .push(i);
    }
    let groups: Vec<_> = schedule.into_values().collect();
    let mut order = Vec::new();
    for n in 0..groups.iter().map(Vec::len).max().unwrap_or(0) {
        for g in &groups {
            if let Some(i) = g.get(n) {
                order.push(*i);
            }
        }
    }
    if !order.is_empty() {
        let n = order.len();
        order.rotate_left((m.seed % n as u64) as usize);
    }
    for index in order {
        let now = crate::judge::now_ms();
        if now >= m.window_end_unix_ms || started.elapsed().as_millis() >= u128::from(m.elapsed_ms)
        {
            break;
        }
        let cell = &mut state.cells[index];
        if cell.complete || cell.retry_at.is_some_and(|at| at > now) {
            continue;
        }
        let t = &m.tasks[index];
        let (r, c) = request(t, file)?;
        let source_roots = dispatch_sources(&c, &crate::paths::resolve(&t.case.path, file))?;
        if transport
            .user
            .authorize(&source_roots, transport.roots)
            .is_err()
        {
            cell.observation.outcome = Outcome::Denied;
            cell.complete = true;
            save(state_file, &state)?;
            continue;
        }
        let model = std::iter::once(&t.model)
            .chain(&t.fallback)
            .nth(cell.next_model)
            .ok_or_else(|| config("invalid fallback state"))?;
        let bytes = payload(&r, &t.provider, model)?;
        let cell_scope = format!("evaluation/{}/cell/{index}", id.as_str());
        let consumed = transport.ledger.used(&cell_scope).map_err(config)?;
        let cell = &mut state.cells[index];
        if consumed > cell.observation.attempts {
            cell.observation.attempts = consumed;
            cell.observation.outcome = Outcome::Deferred;
            cell.retries = cell.retries.max(consumed as usize);
        }
        let mut cell_auth = transport.authorization.clone();
        cell_auth.scopes.push(crate::budget_ledger::Scope {
            id: cell_scope.clone(),
            caps: crate::budget_ledger::Caps {
                total: m.budget_calls,
                providers: std::collections::BTreeMap::from([(t.provider.clone(), m.budget_calls)]),
            },
        });
        let cell_transport = Transport {
            user: transport.user,
            roots: transport.roots,
            authorization: &cell_auth,
            ledger: transport.ledger,
            keys: transport.keys,
            http: transport.http,
        };
        let attempt_start = std::time::Instant::now();
        // Record pending identity before dispatch. A crash resumes conservatively
        // using the ledger's payload reservations, including ambiguous attempts.
        save(state_file, &state)?;
        let timeout = Duration::from_millis(
            m.elapsed_ms
                .saturating_sub(started.elapsed().as_millis() as u64)
                .min(m.window_end_unix_ms.saturating_sub(now)),
        );
        let outcome =
            cell_transport.once(&t.provider, model, &bytes, &source_roots, 1, timeout, false);
        let cell = &mut state.cells[index];
        cell.observation.attempts = transport.ledger.used(&cell_scope).map_err(config)?;
        cell.observation.latency_ms = Some(
            cell.observation
                .latency_ms
                .unwrap_or(0)
                .saturating_add(attempt_start.elapsed().as_millis() as u64),
        );
        match outcome {
            Ok((body, reservation)) => {
                let identity = ProviderIdentity {
                    provider: t.provider.clone(),
                    model: model.clone(),
                    revision: None,
                };
                let response = if t.provider == "jev" {
                    observations::decode_jev(
                        &r,
                        &bytes,
                        &body,
                        identity,
                        crate::judge_provider::transport::usage(&body),
                    )
                    .map_err(config)
                } else {
                    ProviderResponse::from_bytes(
                        identity,
                        &bytes,
                        &body,
                        crate::judge_provider::transport::usage(&body),
                    )
                    .map_err(config)
                };
                let response = response.and_then(|response| {
                    let cap = crate::judge_provider::observations::JevAdapter {
                        identity: response.audit.identity.clone(),
                        transport: &NoExchange,
                    };
                    use crate::decision_provider::DecisionProvider;
                    let mut capabilities = cap.capabilities();
                    capabilities.probabilities = true;
                    response
                        .clone()
                        .into_proposal(&r, &capabilities)
                        .map_err(config)?;
                    Ok(response)
                });
                match response {
                    Ok(response) => {
                        if let Some(label) = &t.labels {
                            cell.calibrator = Some(
                                crate::review::CalibratorIdentity::for_request(
                                    m.dataset_sha256.as_str().into(),
                                    &r,
                                    response.audit.identity.clone(),
                                    label.sha256.clone(),
                                    m.split_sha256.clone(),
                                )
                                .map_err(config)?,
                            );
                        }
                        cell.observation.model = model.clone();
                        cell.observation.answer = Some(response.answer.answer.clone());
                        cell.observation.probability = response
                            .answer
                            .probabilities
                            .as_ref()
                            .and_then(|p| p.get(&response.answer.answer).copied());
                        cell.observation.outcome = if response.answer.answer == "abstain" {
                            Outcome::Abstained
                        } else {
                            Outcome::Answered
                        };
                        cell.observation.usage = response
                            .usage
                            .as_ref()
                            .map(serde_json::to_value)
                            .transpose()?;
                        cell.observation.cost = response.usage.and_then(|u| u.cost);
                        if let Some(truth) = &cell.observation.truth {
                            cell.observation.critical_error = crate::questions::score(
                                &r.question.id,
                                truth,
                                Some(&response.answer.answer),
                                crate::questions::ScoringContext::default(),
                            )
                            .map_err(config)?
                            .harmful_miss;
                        }
                        transport
                            .ledger
                            .finish(&reservation, "answered", false, None)
                            .map_err(config)?;
                    }
                    Err(_) => {
                        cell.observation.outcome = Outcome::Invalid;
                        transport
                            .ledger
                            .finish(&reservation, "invalid", false, None)
                            .map_err(config)?;
                    }
                }
                cell.complete = true;
            }
            Err(e) => {
                // Other processes may append attempts while this HTTP request is
                // pending. Finish only the reservation attached to this failure.
                if let Some(reservation) = e.message.split("reservation=").nth(1) {
                    transport
                        .ledger
                        .finish(
                            reservation,
                            "unavailable",
                            matches!(e.class, RetryClass::AuthenticationOrConfiguration),
                            None,
                        )
                        .map_err(config)?;
                }
                let transient = matches!(e.class, RetryClass::Transient | RetryClass::RateLimited);
                cell.observation.outcome = if e.message.contains("budget_exhausted") {
                    Outcome::BudgetBlocked
                } else if e.message.contains("egress_denied") {
                    Outcome::Denied
                } else if transient {
                    Outcome::Deferred
                } else {
                    Outcome::Unavailable
                };
                if transient && cell.retries < m.retry_secs.len() {
                    cell.retry_at = Some(
                        now.saturating_add(
                            m.retry_secs[cell.retries]
                                .max(e.retry_after_secs.unwrap_or(0))
                                .saturating_mul(1000),
                        ),
                    );
                    cell.retries += 1;
                } else if transient
                    && m.mode == "production_policy"
                    && cell.next_model < t.fallback.len()
                {
                    cell.next_model += 1;
                    cell.retries = 0;
                    cell.retry_at = Some(now);
                } else {
                    cell.complete = true;
                    if transient {
                        cell.observation.outcome = Outcome::Unavailable;
                    }
                }
            }
        }
        save(state_file, &state)?;
    }
    save(state_file, &state)?;
    let observations: Vec<_> = state.cells.iter().map(|c| c.observation.clone()).collect();
    let complete = state.cells.iter().all(|c| c.complete)
        && !state.cells.iter().any(|c| {
            matches!(
                c.observation.outcome,
                Outcome::BudgetBlocked | Outcome::Deferred
            )
        });
    let mut fits: std::collections::BTreeMap<
        Digest,
        (crate::review::CalibratorIdentity, Vec<Observation>),
    > = std::collections::BTreeMap::new();
    if m.job == "calibrate" {
        for cell in &state.cells {
            if let Some(identity) = &cell.calibrator {
                fits.entry(identity.digest().map_err(config)?)
                    .or_insert_with(|| (identity.clone(), vec![]))
                    .1
                    .push(cell.observation.clone());
            }
        }
    }
    let fits=fits.into_iter().map(|(identity_sha256,(identity,rows))|json!({"identity_sha256":identity_sha256,"identity":identity,"fit":crate::judge_stats::evaluation::fit(&rows)})).collect::<Vec<_>>();
    Ok(
        json!({"schema":"saccade-evaluation.v1","job":m.job,"mode":m.mode,"manifest_id":id,"execution":if complete{"complete"}else{"incomplete"},"qualified":false,"labeler_count":m.labeler_count,"exposure":m.exposure,"test_retest":m.test_retest,"price":m.price,"metrics":crate::judge_stats::evaluation::strata(&observations),"cells":state.cells,"state":crate::paths::portable(state_file),"calibration":if m.job=="calibrate"{json!({"split":"calibration","fits":fits,"qualified":false})}else{Value::Null}}),
    )
}
struct NoExchange;
impl observations::DecisionTransport for NoExchange {
    fn exchange(
        &self,
        _: &[u8],
    ) -> std::result::Result<
        (Vec<u8>, Option<crate::decision_provider::Usage>),
        crate::decision_provider::ProviderFailure,
    > {
        Err(crate::decision_provider::ProviderFailure {
            class: RetryClass::AuthenticationOrConfiguration,
            message: "offline capability construction".into(),
            retry_after_secs: None,
        })
    }
}
