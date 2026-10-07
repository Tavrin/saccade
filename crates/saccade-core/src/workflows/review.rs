//! Explicitly authorized review and deterministic offline payload previews.
use super::{CommandError as CliError, support as local_cmd};
use crate::budget_ledger::{Caps, Ledger, Scope};
use crate::decision_provider::DecisionProvider;
use crate::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    case::EvidenceCase,
    request::ProviderIdentity,
};
use crate::judge_provider::{
    Keys,
    observations::JevAdapter,
    transport::{self, Authorization, UserConfig},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

/// Load user with explicit inputs and no console output.
pub fn load_user(file: &Path) -> Result<UserConfig, CliError> {
    UserConfig::load(file).map_err(|e| CliError::new("invalid_user_policy", e))
}
/// Authorization with explicit inputs and no console output.
pub fn authorization(enabled: bool, budget: u64, run: &str, user: &UserConfig) -> Authorization {
    let caps = Caps {
        total: budget,
        providers: BTreeMap::from([
            ("jev".into(), budget.min(6)),
            ("gemini".into(), budget.min(18)),
        ]),
    };
    let mut scopes = vec![Scope {
        id: format!("run/{run}"),
        caps,
    }];
    if let Some(cap) = &user.parent_budget {
        scopes.push(Scope {
            id: "parent/user".into(),
            caps: cap.clone(),
        });
    }
    Authorization { enabled, scopes }
}
fn case(file: &Path) -> Result<EvidenceCase, CliError> {
    let value = local_cmd::read_value(file)?;
    transport::reject_project_overrides(&value)
        .map_err(|e| CliError::new("invalid_project_policy", e))?;
    if crate::report_links::original_schema(value["schema"].as_str().unwrap_or_default())
        == crate::report::REPORT_SCHEMA
    {
        let report = super::read_report(file)?;
        let mut c = local_cmd::case_for_result(&report, file)?;
        if c.requests.is_empty() {
            crate::judge_evidence::report_features(&mut c, &report)?;
        }
        Ok(c)
    } else {
        let doc = Document::read(file)?;
        let Artifact::Case(c) = doc.artifact else {
            return Err(CliError::usage(
                "review requires a report or canonical case",
            ));
        };
        Ok(*c)
    }
}
fn verify_inputs(
    c: &EvidenceCase,
    file: &Path,
    roots: &crate::root_policy::RootPolicy,
) -> Result<(), CliError> {
    let document = if file
        .file_name()
        .is_some_and(|n| n == "saccade-report.v1.json")
    {
        file.with_file_name("evidence.json")
    } else {
        file.to_owned()
    };
    for input in &c.inputs {
        for reference in std::iter::once(&input.content).chain(&input.sidecars) {
            let resolved = crate::paths::resolve(&reference.path, &document);
            roots.read(&resolved)?;
            reference.verify(&document)?;
        }
    }
    Ok(())
}
/// Guard case inputs with explicit inputs and no console output.
pub fn guard_case_inputs(
    file: &Path,
    roots: &crate::root_policy::RootPolicy,
) -> Result<(), CliError> {
    let _scope = crate::root_policy::io::scope(roots);
    let file = roots.read(file)?;
    let file = file.as_path();
    let value = local_cmd::read_value(file)?;
    if crate::report_links::original_schema(value["schema"].as_str().unwrap_or_default())
        == crate::report::REPORT_SCHEMA
    {
        let report: crate::Report = serde_json::from_value(value)?;
        let evidence = file.with_file_name("evidence.json");
        if evidence.exists() {
            roots.read(&evidence)?;
        }
        for entry in &report.entries {
            for baseline in [true, false] {
                let recorded = if baseline {
                    &entry.paths.baseline
                } else {
                    &entry.paths.capture
                };
                if recorded.is_none() {
                    continue;
                }
                if let Some(input) = local_cmd::report_input(&report, file, &entry.name, baseline) {
                    roots.read(&input)?;
                    let source = if baseline {
                        &report.baseline_dir
                    } else {
                        &report.capture_dir
                    };
                    let source = source
                        .as_deref()
                        .map(|p| crate::paths::resolve(p, file))
                        .map(|p| {
                            if p.is_file() {
                                p.parent().unwrap_or(Path::new(".")).to_owned()
                            } else {
                                p
                            }
                        });
                    let mut directory = input.parent();
                    while let Some(dir) = directory {
                        if source.as_ref().is_none_or(|root| !dir.starts_with(root)) {
                            break;
                        }
                        let sidecar = dir.join(&report.config.meta.name);
                        if sidecar.exists() {
                            roots.read(&sidecar)?;
                        }
                        directory = dir.parent();
                    }
                    if let Some(stem) = input.file_stem().and_then(|s| s.to_str()) {
                        let sidecar =
                            input.with_file_name(format!("{stem}.{}", report.config.meta.name));
                        if sidecar.exists() {
                            roots.read(&sidecar)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn source_references(sources: &[String], roots: &crate::root_policy::RootPolicy) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            let path = crate::run::normalise_path(crate::paths::native(Path::new(source)).as_ref());
            if let Some(root) = roots.root_of(&path)
                && let Ok(relative) = path.strip_prefix(&root.path)
            {
                crate::paths::portable(&Path::new(&root.name).join(relative))
            } else {
                let canonical = crate::paths::portable(&path);
                format!("source:{}", Digest::of_bytes(canonical.as_bytes()).as_str())
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
/// Review with explicit inputs and no console output.
#[allow(clippy::too_many_arguments)]
pub fn review(
    file: &Path,
    out: Option<&Path>,
    run: bool,
    budget: u64,
    config: &Path,
    roots: &crate::root_policy::RootPolicy,
    auth: &Authorization,
    intent: Option<crate::evidence::case::Intent>,
) -> Result<Value, CliError> {
    if run {
        auth.check()
            .map_err(|e| CliError::new("network_authorization_required", e))?;
    }
    let user = load_user(config)?;
    let mut policy = roots.clone();
    user.apply(&mut policy)
        .map_err(|e| CliError::new("invalid_user_policy", e))?;
    let file = policy.read(file)?;
    let out = out.map(|p| policy.write(p)).transpose()?;
    let file = file.as_path();
    let out = out.as_deref();
    let _scope = crate::root_policy::io::scope(&policy);
    guard_case_inputs(file, &policy)?;
    // Authorize every companion before constructing a case or dispatching.
    if let Some(out) = out {
        let parent = out.parent().unwrap_or(Path::new("."));
        for p in [
            out.to_owned(),
            parent.join("review-plan.json"),
            parent.join(".saccade-run"),
        ] {
            policy.write(&p)?;
            if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(CliError::new(
                    "unsafe_path",
                    "refusing output companion symlink",
                ));
            }
        }
    }
    let mut c = case(file)?;
    apply_intent(&mut c, intent)?;
    verify_inputs(&c, file, &policy)?;
    if c.requests.is_empty() {
        crate::judge_evidence::prepare_context(&mut c)?;
    }
    let (mut requests, shortfalls) = prepare_requests(&c)?;
    let (project, project_scopes) = project_policy(file, &policy)?;
    let budget = budget.min(project.budget_calls.unwrap_or(budget));
    let mut authorization = auth.clone();
    authorization.scopes.extend(project_scopes);
    for scope in &mut authorization.scopes {
        scope.caps.total = scope.caps.total.min(budget);
        if !project.providers.is_empty() {
            scope
                .caps
                .providers
                .retain(|p, _| project.providers.contains(p));
        }
    }
    let mut sources = crate::judge_bench_sources(&c);
    if !sources.is_empty() {
        for input in &c.inputs {
            for reference in std::iter::once(&input.content).chain(&input.sidecars) {
                sources.extend(
                    crate::paths::source_paths(&crate::paths::resolve(&reference.path, file))
                        .map_err(|e| CliError::io(e.to_string()))?,
                );
            }
        }
    }
    let egress = user.authorize(&sources, &policy);
    let model = project.model.unwrap_or_else(|| "jev-latest".into());
    let keys = Keys::new(Some(config.parent().unwrap_or(Path::new(".")).into()));
    let ledger = Ledger::new(
        &config.parent().unwrap_or(Path::new(".")).join("attempts"),
        false,
    );
    let transport = transport::Transport {
        user: &user,
        roots: &policy,
        authorization: &authorization,
        ledger: &ledger,
        keys: &keys,
        http: &transport::Network,
    };
    let mut payloads = Vec::new();
    let mut proposals = Vec::new();
    let mut failures = Vec::new();
    for request in &requests {
        crate::questions::validate_request(request)?;
        let payload = crate::judge_provider::observations::jev_payload(request, &model)?;
        payloads.push(json!({"request_id":request.request_id,"question":request.question.id,"provider":"jev","model":model,"payload_sha256":Digest::of_bytes(&payload),"source_roots":sources,"policy":if egress.is_ok(){"allow"}else{"deny"}}));
        if run && project.profile.as_deref() != Some("lookdev") {
            egress
                .as_ref()
                .map_err(|e| CliError::new("egress_denied", e.clone()))?;
            let bridge = transport::JevTransport {
                transport: &transport,
                model: model.clone(),
                sources: sources.clone(),
                deadline: Duration::from_secs(30),
            };
            let adapter = JevAdapter {
                identity: ProviderIdentity {
                    provider: "jev".into(),
                    model: model.clone(),
                    revision: None,
                },
                transport: &bridge,
            };
            match adapter.answer(request){
                Ok(response)=>proposals.push(response.into_proposal(request,&adapter.capabilities())?),
                Err(e)=>failures.push(json!({"request_id":request.request_id,"class":e.class,"message":e.message,"retry_at":e.retry_after_secs.map(|s|crate::budget_ledger::now_ms().saturating_add(s.saturating_mul(1000)))})),
            }
        }
    }
    let wants_vision = project.profile.as_deref() == Some("lookdev")
        || proposals.iter().any(|p| {
            matches!(
                p.response.answer.as_str(),
                "needs_eyes" | "inspect_regions" | "inspect_full_frame"
            )
        });
    let mut visual_results = Vec::new();
    if run && wants_vision && project.profile.as_deref() != Some("ci") {
        egress
            .as_ref()
            .map_err(|e| CliError::new("egress_denied", e.clone()))?;
        let parent = out
            .and_then(Path::parent)
            .ok_or_else(|| CliError::usage("visual review requires an output artifact"))?;
        let models = if project.gemini_models.is_empty() {
            crate::review::GEMINI_MODELS
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
        } else {
            project.gemini_models.clone()
        };
        let vision_policy = crate::judge_provider::observations::FallbackPolicy {
            models: models.clone(),
            version: "budgeted-fallback/1".into(),
        };
        for (index, entry) in c.scope.entries.iter().take(3).enumerate() {
            let pair = match display_pair(&c, file, entry) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(json!({"entry":entry,"message":e.message}));
                    continue;
                }
            };
            let first = crate::judge_evidence::vision::prepare_visual(
                &c,
                entry,
                [&pair[0], &pair[1]],
                &[],
                crate::judge_evidence::vision::VisionTask::BlindPreference,
                false,
            )?;
            let first = transport::vision(
                &transport,
                &c,
                &first,
                &vision_policy,
                &sources,
                &parent.join(format!("vision-{index}-ab.json")),
                Duration::from_secs(30),
                None,
            );
            let result = match first {
                Ok(first) => {
                    // Both orders stay on the actual first model. No fallback may mix the pair.
                    let second = crate::judge_evidence::vision::prepare_visual(
                        &c,
                        entry,
                        [&pair[0], &pair[1]],
                        &[],
                        crate::judge_evidence::vision::VisionTask::BlindPreference,
                        true,
                    )?;
                    let model = first.context().extractor.model.clone();
                    match transport::vision(
                        &transport,
                        &c,
                        &second,
                        &vision_policy,
                        &sources,
                        &parent.join(format!("vision-{index}-ba.json")),
                        Duration::from_secs(30),
                        Some(&model),
                    ) {
                        Ok(second) => serde_json::to_value(crate::review::resolve_blind_orders(
                            &c, &first, &second,
                        )?)?,
                        Err(e) => json!({"outcome":"needs_human","reason":e}),
                    }
                }
                Err(e) => json!({"outcome":"needs_human","reason":e}),
            };
            visual_results.push(json!({"entry":entry,"blind_comparison":result}));
            let presentation = crate::judge_evidence::vision::prepare_visual(
                &c,
                entry,
                [&pair[0], &pair[1]],
                &[],
                crate::judge_evidence::vision::VisionTask::Observations,
                false,
            )?;
            match transport::vision(
                &transport,
                &c,
                &presentation,
                &vision_policy,
                &sources,
                &parent.join(format!("vision-{index}-observations.json")),
                Duration::from_secs(30),
                None,
            ) {
                Ok(completed) => {
                    match crate::review::prepare_enriched_question(
                        &c,
                        "intent.match.v1",
                        &completed,
                        BTreeMap::new(),
                    ) {
                        Ok(enriched) => {
                            let bridge = transport::JevTransport {
                                transport: &transport,
                                model: model.clone(),
                                sources: sources.clone(),
                                deadline: Duration::from_secs(30),
                            };
                            let adapter = JevAdapter {
                                identity: ProviderIdentity {
                                    provider: "jev".into(),
                                    model: model.clone(),
                                    revision: None,
                                },
                                transport: &bridge,
                            };
                            match adapter.answer(&enriched) {
                                Ok(response) => proposals.push(
                                    response.into_proposal(&enriched, &adapter.capabilities())?,
                                ),
                                Err(e) => failures.push(
                                    json!({"entry":entry,"class":e.class,"message":e.message}),
                                ),
                            };
                            requests.push(enriched);
                        }
                        Err(e) => failures.push(json!({"entry":entry,"message":e.to_string()})),
                    }
                }
                Err(e) => failures.push(json!({"entry":entry,"message":e})),
            }
        }
    }
    let dispatched_calls = if run {
        auth.scopes
            .first()
            .map(|s| ledger.used(&s.id))
            .transpose()
            .map_err(CliError::io)?
            .unwrap_or(0)
    } else {
        0
    };
    if let Some(out) = out {
        let plan = json!({"schema":"saccade-evaluation.v1","kind":"review_plan","case_id":c.case_id,"payloads":payloads,"shortfalls":shortfalls,"visual_results":visual_results,"failures":failures,"budget_calls":budget,"dispatched_calls":dispatched_calls});
        c.requests = requests.clone();
        c.proposals = proposals.clone();
        // Canonical case artifacts remain the source of requests/proposals. Rebase
        // portable file references without changing semantic evidence identities.
        let mut value = serde_json::to_value(Document::new(Artifact::Case(Box::new(c))))?;
        rebase(&mut value, file, out)?;
        local_cmd::write_value(out, &value)?;
        let parent = out.parent().unwrap_or(Path::new("."));
        local_cmd::write_value(&parent.join("review-plan.json"), &plan)?;
        crate::root_policy::io::write(parent.join(".saccade-run"), b"")
            .map_err(|e| CliError::io(e.to_string()))?;
    }
    let source_references = json!(source_references(&sources, &policy));
    for payload in &mut payloads {
        payload["source_roots"] = source_references.clone();
    }
    let mut result = local_cmd::base_result(if run { "review.run" } else { "review.preview" });
    result["review"] = json!("unresolved");
    if run && !failures.is_empty() {
        result["execution"] = json!("pending");
    }
    result["counts"] = json!({"budget_calls":budget,"scheduled_questions":requests.len(),"proposals":proposals.len(),"dispatched_calls":dispatched_calls});
    result["data"] = json!({"payloads":payloads,"policy":if egress.is_ok(){"allow"}else{"deny"},"provider_calls_authorized":auth.enabled,"shortfalls":shortfalls,"visual_results":visual_results,"failures":failures});
    result["limits"] = json!([
        "Model answers remain proposals. Human review is unresolved.",
        "Payload previews use root-relative sources or source IDs; full provenance stays in the local artifact."
    ]);
    if let Some(out) = out {
        result["artifact"] = local_cmd::reference(out)?;
    }
    local_cmd::bounded(result, 4096)
}
fn toml_policy(path: &Path) -> Result<transport::ProjectPolicy, CliError> {
    let text =
        crate::root_policy::io::read_to_string(path).map_err(|e| CliError::io(e.to_string()))?;
    // Deserialize with strict fields: URL/key/root/startup fields are rejected.
    crate::judge_provider::transport::parse_project_policy(&text)
        .map_err(|e| CliError::new("invalid_project_policy", e))
}
/// Preview policy with explicit inputs and no console output.
pub fn preview_policy(
    user: &UserConfig,
) -> Result<Option<crate::root_policy::RootPolicy>, CliError> {
    // A pricing-only/default configuration retains the local preview contract.
    // Once roots are declared, offline execution cannot bypass their containment.
    if user.roots.is_empty() {
        return Ok(None);
    }
    let roots = user
        .roots
        .iter()
        .map(|r| r.path.clone())
        .collect::<Vec<_>>();
    Ok(Some(crate::root_policy::RootPolicy::new(
        &roots,
        user.out_root.as_deref(),
        false,
        &[],
    )?))
}
/// Preview local with explicit inputs and no console output.
pub fn preview_local(
    file: &Path,
    budget: u64,
    intent: Option<crate::evidence::case::Intent>,
    out: Option<&Path>,
    absolute: bool,
    user_config: &Path,
) -> Result<Value, CliError> {
    let user = load_user(user_config)?;
    let policy = preview_policy(&user)?;
    let resolved_file = policy.as_ref().map(|p| p.read(file)).transpose()?;
    let resolved_out = out
        .map(|out| {
            policy
                .as_ref()
                .map_or_else(|| Ok(out.to_owned()), |p| p.write(out))
        })
        .transpose()?;
    let file = resolved_file.as_deref().unwrap_or(file);
    let out = resolved_out.as_deref();
    let _scope = policy.as_ref().map(crate::root_policy::io::scope);
    if let Some(policy) = &policy {
        guard_case_inputs(file, policy)?;
    }
    let mut c = case(file)?;
    if let Some(policy) = &policy {
        verify_inputs(&c, file, policy)?;
    }
    apply_intent(&mut c, intent)?;
    if c.requests.is_empty() {
        crate::judge_evidence::prepare_context(&mut c)?;
    }
    let (requests, shortfalls) = prepare_requests(&c)?;
    let sources = crate::judge_bench_sources(&c)
        .iter()
        .map(|s| {
            if absolute {
                s.clone()
            } else {
                crate::paths::cwd(Path::new(s), false)
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut saved = Vec::new();
    let payloads = requests.iter().map(|r| {
        let bytes = crate::judge_provider::observations::jev_payload(r,"jev-latest")?;
        let estimated_input_tokens = bytes.len().div_ceil(4);
        let estimated_output_tokens = 512usize;
        let price = user.pricing.get("jev/jev-latest");
        let estimated_cost_usd = price.map(|p| (estimated_input_tokens as f64*p.input_per_million_usd + estimated_output_tokens as f64*p.output_per_million_usd)/1_000_000.0);
        saved.push(json!({"request_id":r.request_id,"payload":serde_json::from_slice::<Value>(&bytes).unwrap_or_else(|_|json!(String::from_utf8_lossy(&bytes).to_string()))}));
        Ok(json!({"request_id":r.request_id,"question":r.question.id,"provider":"jev","model":"jev-latest","payload_sha256":Digest::of_bytes(&bytes),"source_roots":sources,"policy":"deny","request_bytes":bytes.len(),"estimated_input_tokens":estimated_input_tokens,"estimated_output_tokens":estimated_output_tokens,"estimated_cost_usd":estimated_cost_usd,"cost_reason":if price.is_some() {Value::Null} else {json!("no jev/jev-latest price configured in user.toml")}}))
    }).collect::<crate::evidence::Result<Vec<_>>>()?;
    if let Some(out) = out {
        local_cmd::write_value(&out.join("requests.json"), &json!(saved))?;
    }
    let mut value = local_cmd::base_result("review.preview");
    value["counts"] =
        json!({"budget_calls":budget,"scheduled_questions":requests.len(),"dispatched_calls":0});
    value["data"] =
        json!({"payloads":payloads,"provider_calls_authorized":false,"shortfalls":shortfalls});
    value["limits"] = json!([
        "Local preview; explicit execution, user root permissions and finite attempt budget are required.",
        "Every model answer is a proposal; human review remains unresolved."
    ]);
    let mut value = local_cmd::bounded(value, 4096)?;
    if let Some(out) = out {
        let summary = out.join("preview.json");
        value["paths"] = json!({"summary":crate::paths::cwd(&summary,absolute),"requests":crate::paths::cwd(&out.join("requests.json"),absolute)});
        local_cmd::write_value(&summary, &value)?;
    }
    Ok(value)
}
/// Rebase with explicit inputs and no console output.
pub fn rebase(value: &mut Value, source: &Path, destination: &Path) -> Result<(), CliError> {
    match value {
        Value::Object(fields) => {
            if fields.contains_key("sha256")
                && let Some(path) = fields.get_mut("path")
                && let Some(p) = path.as_str()
            {
                let original = crate::paths::resolve(p, source);
                *path = json!(crate::paths::record(
                    &original,
                    destination.parent().unwrap_or(Path::new(".")),
                    false
                ));
            }
            for v in fields.values_mut() {
                rebase(v, source, destination)?;
            }
        }
        Value::Array(values) => {
            for v in values {
                rebase(v, source, destination)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn display_pair(
    c: &EvidenceCase,
    file: &Path,
    entry: &str,
) -> Result<[crate::judge_evidence::vision::DisplayImage; 2], CliError> {
    use crate::judge_evidence::vision::{DisplayImage, DisplayTransform};
    let load = |roles: &[&str]| -> Result<DisplayImage, CliError> {
        let input = c
            .inputs
            .iter()
            .find(|i| {
                roles
                    .iter()
                    .any(|r| i.id == *r || i.id == format!("{r}:{entry}"))
            })
            .ok_or_else(|| {
                CliError::new(
                    "visual_evidence_unavailable",
                    "entry has no verified image pair",
                )
            })?;
        let path = crate::paths::resolve(&input.content.path, file);
        let transform = if crate::hdr::is_hdr_path(&path) {
            DisplayTransform::Hdr {
                version: "hdr-display/1".into(),
                tonemapper: "aces".into(),
                exposure_stops: 0.0,
            }
        } else {
            DisplayTransform::Srgb { background: 128 }
        };
        Ok(DisplayImage::load(&input.id, &path, transform)?)
    };
    Ok([
        load(&["baseline", "reference"])?,
        load(&["capture", "candidate"])?,
    ])
}
/// Prepare request with explicit inputs and no console output.
pub fn prepare_request(
    file: &Path,
    question: &str,
) -> Result<crate::evidence::request::DecisionRequest, CliError> {
    let mut c = case(file)?;
    if let Some(request) = c.requests.iter().find(|r| r.question.id == question) {
        request.validate_for(&c)?;
        return Ok(request.clone());
    }
    crate::judge_evidence::prepare_context(&mut c)?;
    Ok(crate::judge_evidence::encode(
        &c,
        question,
        Default::default(),
    )?)
}
/// Read intent with explicit inputs and no console output.
pub fn read_intent(
    text: Option<&str>,
    file: Option<&Path>,
    document: &Path,
) -> Result<Option<crate::evidence::case::Intent>, CliError> {
    use crate::evidence::case::{ArtifactRef, Intent, IntentAssurance, Provenance};
    if let Some(file) = file {
        let mut intent: Intent = serde_json::from_value(local_cmd::read_value(file)?)?;
        intent.assurance = IntentAssurance::Structured;
        intent.source = Some(ArtifactRef::from_file(file, document, false)?);
        intent.provenance.source_roots =
            crate::paths::source_paths(file).map_err(|e| CliError::io(e.to_string()))?;
        return Ok(Some(intent));
    }
    Ok(text.map(|objective| Intent {
        id: "cli-text-intent".into(),
        objective: objective.into(),
        assurance: IntentAssurance::Text,
        expected_changes: vec![],
        invariants: vec![],
        criteria: vec![],
        source: None,
        mask_sources: vec![],
        provenance: Provenance::default(),
    }))
}
fn apply_intent(
    c: &mut EvidenceCase,
    intent: Option<crate::evidence::case::Intent>,
) -> Result<(), CliError> {
    if let Some(mut intent) = intent {
        if intent.objective.trim().is_empty() {
            return Err(CliError::usage("intent objective must be nonempty"));
        }
        if intent.provenance.source_roots.is_empty() {
            intent.provenance.source_roots = c.provenance.source_roots.clone();
        }
        c.provenance
            .source_roots
            .extend(intent.provenance.source_roots.clone());
        c.intent = crate::evidence::case::Availability::Available { value: intent };
        c.requests.clear();
        c.proposals.clear();
        c.human_decisions.clear();
        c.next_actions.clear();
        c.refresh_id()?;
    }
    Ok(())
}
fn prepare_requests(
    case: &EvidenceCase,
) -> Result<(Vec<crate::evidence::request::DecisionRequest>, Vec<Value>), CliError> {
    if !case.requests.is_empty() {
        for request in &case.requests {
            crate::questions::validate_request(request)?;
        }
        return Ok((case.requests.clone(), vec![]));
    }
    let mut requests = Vec::new();
    let mut shortfalls = Vec::new();
    for question in crate::questions::CATALOG {
        match crate::judge_evidence::encode(case, question.id, Default::default()) {
            Ok(request) => requests.push(request),
            Err(error) => shortfalls.push(
                json!({"question":question.id,"status":"inapplicable","reason":error.to_string()}),
            ),
        }
    }
    Ok((requests, shortfalls))
}
fn project_policy(
    file: &Path,
    roots: &crate::root_policy::RootPolicy,
) -> Result<(transport::ProjectPolicy, Vec<Scope>), CliError> {
    let mut paths = roots
        .roots
        .iter()
        .map(|r| r.path.join("saccade-review.toml"))
        .collect::<Vec<_>>();
    paths.push(
        file.parent()
            .unwrap_or(Path::new("."))
            .join("saccade-review.toml"),
    );
    let mut seen = std::collections::BTreeSet::new();
    let mut effective = transport::ProjectPolicy::default();
    let mut scopes = Vec::new();
    for file in paths {
        if !file.is_file() {
            continue;
        }
        let file = roots.read(&file)?;
        let normalized =
            crate::paths::canonicalize(&file).map_err(|e| CliError::io(e.to_string()))?;
        if !seen.insert(normalized.clone()) {
            continue;
        }
        let project = toml_policy(&file)?;
        if let Some(cap) = project.budget_calls {
            effective.budget_calls = Some(effective.budget_calls.map_or(cap, |old| old.min(cap)));
            scopes.push(Scope {
                id: format!(
                    "project/{}",
                    canonical::digest(&crate::paths::portable(&normalized))?.as_str()
                ),
                caps: Caps {
                    total: cap,
                    providers: BTreeMap::from([("jev".into(), cap), ("gemini".into(), cap)]),
                },
            });
        }
        if !project.providers.is_empty() {
            if effective.providers.is_empty() {
                effective.providers = project.providers.clone();
            } else {
                effective
                    .providers
                    .retain(|p| project.providers.contains(p));
                if effective.providers.is_empty() {
                    return Err(CliError::new(
                        "invalid_project_policy",
                        "project provider restrictions have no common provider",
                    ));
                }
            }
        }
        if project.model.is_some() {
            effective.model = project.model;
        }
        if project.profile.is_some() {
            effective.profile = project.profile;
        }
        if !project.gemini_models.is_empty() {
            effective.gemini_models = project.gemini_models;
        }
    }
    Ok((effective, scopes))
}
/// Authorized review inputs. Preview never sends a provider request.
pub struct ReviewRun<'a> {
    /// Report or canonical case.
    pub file: &'a Path,
    /// Optional new canonical case output file.
    pub out: Option<&'a Path>,
    /// Dispatch only under explicit authorization.
    pub run: bool,
    /// Finite attempt allowance.
    pub budget: u64,
    /// User-owned provider configuration.
    pub config: &'a Path,
    /// Registered input/output roots and egress permissions.
    pub roots: &'a crate::root_policy::RootPolicy,
    /// Explicit caller authorization.
    pub authorization: &'a Authorization,
    /// Optional declared intent.
    pub intent: Option<crate::evidence::case::Intent>,
}
/// Execute an authorized command-level review and return its bounded typed result.
pub fn run_review(opts: ReviewRun<'_>) -> Result<super::WireResult, CliError> {
    Ok(super::WireResult {
        value: review(
            opts.file,
            opts.out,
            opts.run,
            opts.budget,
            opts.config,
            opts.roots,
            opts.authorization,
            opts.intent,
        )?,
    })
}

#[cfg(test)]
mod budget_scope_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn editing_project_policy_cannot_reset_its_consumed_budget() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("saccade-review.toml");
        let case = root.path().join("case.json");
        let roots =
            crate::root_policy::RootPolicy::new(&[root.path().to_owned()], None, false, &[])
                .unwrap();
        std::fs::write(&project, "budget_calls = 4\n").unwrap();
        let (_, first) = project_policy(&case, &roots).unwrap();
        let ledger = Ledger::new(&root.path().join("ledger"), false);
        let attempt = || crate::budget_ledger::Attempt {
            id: crate::budget_ledger::new_id(),
            provider: "jev".into(),
            model: "fixture".into(),
            payload_sha256: Digest::of_bytes(b"fixture"),
            source_roots: vec!["fixture".into()],
            batch_size: 1,
            started_ms: crate::budget_ledger::now_ms(),
            outcome: "reserved".into(),
        };
        for _ in 0..4 {
            ledger.reserve(&first, attempt(), false).unwrap();
        }
        std::fs::write(&project, "budget_calls = 8\nprofile = 'ci'\n").unwrap();
        let (_, edited) = project_policy(&case, &roots).unwrap();
        assert_eq!(first[0].id, edited[0].id);
        assert!(ledger.reserve(&edited, attempt(), false).is_err());
        assert_eq!(ledger.used(&first[0].id).unwrap(), 4);
    }
}
