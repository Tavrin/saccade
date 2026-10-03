//! Local 1.0 inspection and proposal operations. No provider dispatch.
use crate::agent::CliError;
use clap::{Args, Subcommand, ValueEnum};
use saccade_core::evidence::canonical::{self, Digest};
use saccade_core::evidence::case::*;
use saccade_core::evidence::{self, Artifact, Document};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

impl From<evidence::ContractError> for CliError {
    fn from(e: evidence::ContractError) -> Self {
        Self::new("invalid_evidence", e.to_string())
    }
}
#[derive(Args)]
pub(crate) struct InspectArgs {
    pub artifact: Option<PathBuf>,
    #[command(subcommand)]
    pub operation: Option<InspectOperation>,
    #[arg(long)]
    pub entry: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub status: Vec<String>,
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    #[arg(long)]
    pub cursor: Option<String>,
    #[arg(long)]
    pub expected_case_id: Option<String>,
    #[arg(long)]
    pub json: bool,
}
#[derive(Subcommand)]
pub(crate) enum InspectOperation {
    /// Prepare context, crops, facts and references without a provider.
    Evidence {
        report: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "entry")]
        entries: Vec<String>,
        #[arg(long, default_value_t = 5)]
        top: usize,
        #[arg(long)]
        stretch: bool,
        #[arg(long)]
        blind: bool,
        #[arg(long, requires = "blind")]
        key_out: Option<PathBuf>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        json: bool,
    },
    /// Export an existing artifact or selected entry.
    Export {
        artifact: PathBuf,
        #[arg(long, value_enum)]
        format: ExportFormat,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long)]
        state: Option<String>,
        #[arg(long, default_value_t = 1024)]
        width: u32,
        #[arg(long)]
        artifact_url: Option<String>,
        #[arg(long)]
        comment_key: Option<String>,
    },
    /// Explain effective measurement settings and their sources.
    Config(crate::f1::ConfigArgs),
    /// List compiled modules, operations and contracts.
    Capabilities {
        #[arg(long)]
        json: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ExportFormat {
    Json,
    Markdown,
    Junit,
    Png,
    Labels,
}
#[derive(Args)]
pub(crate) struct ReviewArgs {
    pub report: Option<PathBuf>,
    #[command(subcommand)]
    pub operation: Option<ReviewOperation>,
    #[arg(long)]
    pub run: bool,
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    pub budget_calls: Option<u64>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long, global = true)]
    pub user_config: Option<PathBuf>,
    #[arg(long)]
    pub intent_file: Option<PathBuf>,
    #[arg(long, conflicts_with = "intent_file")]
    pub intent: Option<String>,
    #[arg(long, global = true)]
    pub json: bool,
}
#[derive(Subcommand)]
pub(crate) enum ReviewOperation {
    /// Prepare a closed request from an existing canonical case, locally.
    Request {
        report: PathBuf,
        #[arg(long)]
        question: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate and record proposed answers against the exact request.
    Propose {
        request: PathBuf,
        #[arg(long)]
        answers: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Create or retrieve a local human review item for an unresolved request.
    Ask {
        request: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Plan or run a resumable evaluation manifest.
    Eval {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        run: bool,
    },
}
pub(crate) fn migration(args: &[std::ffi::OsString]) -> Option<String> {
    if args
        .iter()
        .any(|a| a == "--compat" || a.to_string_lossy().starts_with("--compat="))
    {
        return Some("--compat was removed; use the current commands and --json envelope".into());
    }
    if args
        .iter()
        .any(|a| a.to_string_lossy().starts_with("--json="))
    {
        return Some("use --json; export full artifacts with inspect export --format json".into());
    }
    let command = args.get(1)?.to_str()?;
    let replacement = match command {
        "ablate" | "sequence" | "rank" | "bisect" | "safety" | "a11y" => {
            format!("experiment {command}")
        }
        "config" => "inspect config".into(),
        "entries" | "summary" => "inspect ARTIFACT".into(),
        "explain" => "inspect evidence REPORT --out DIR".into(),
        "snapshot" => "inspect export ARTIFACT --format png --entry NAME --out FILE".into(),
        "decision-request" => "review request CASE --question ID --out FILE".into(),
        "decide" => "review propose REQUEST --answers FILE".into(),
        "ask" => "review ask REQUEST".into(),
        "judge" => "review REPORT or review eval --manifest FILE".into(),
        "runs" => "view CAPTURE... --reference DIR".into(),
        "unblind" => "view --unblind DECISIONS --key FILE".into(),
        "watch" => "the capture producer's scheduling workflow".into(),
        _ => return None,
    };
    Some(format!("{command} was removed; use saccade {replacement}"))
}
pub(crate) fn read_value(path: &Path) -> Result<Value, CliError> {
    Ok(canonical::decode(
        &std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?,
    )?)
}
pub(crate) fn write_value(path: &Path, value: &Value) -> Result<(), CliError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| CliError::io(e.to_string()))?;
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::new("unsafe_path", "refusing output symlink"));
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .map_err(|e| CliError::io(e.to_string()))
}
pub(crate) fn base_result(operation: &str) -> Value {
    json!({"schema":"saccade-result.v2", "operation":operation,"execution":"complete","measurement":"unknown","validity":"unknown","validity_reasons":[],"review":"pending","artifact":null,"counts":{},"entries":[],"next_actions":[],"limits":[],"errors":[],"page":{"omitted":0,"next_cursor":null}})
}
pub(crate) fn reference(path: &Path) -> Result<Value, CliError> {
    Ok(serde_json::to_value(ArtifactRef::from_file(
        path,
        &std::env::current_dir()
            .map_err(|e| CliError::io(e.to_string()))?
            .join("result.json"),
        false,
    )?)?)
}
#[cfg(any(feature = "graphics", feature = "prechecks"))]
pub(crate) fn analysis_result(value: &Value, out: &Path) -> Result<Value, CliError> {
    let mut result = base_result(value["schema"].as_str().unwrap_or("experiment"));
    result["limits"] = json!([
        "Full analysis is recorded in the referenced artifact; completion supplies no acceptance authority."
    ]);
    if let Some(schema) = value["schema"].as_str() {
        let file = out.join(format!("{schema}.json"));
        if file.is_file() {
            result["artifact"] = reference(&file)?;
        }
    }
    Ok(result)
}
pub(crate) fn inspect(args: InspectArgs, absolute: bool) -> Result<u8, CliError> {
    if let Some(operation) = args.operation {
        return match operation {
            InspectOperation::Capabilities { json } => crate::capabilities(json),
            InspectOperation::Config(args) => crate::f1::config(args),
            InspectOperation::Evidence {
                report,
                out,
                entries,
                top,
                stretch,
                blind,
                key_out,
                seed,
                json,
            } => {
                let value = prepare_evidence(
                    &report, &out, &entries, top, stretch, blind, key_out, seed, absolute,
                )?;
                print(&value, json)?;
                Ok(0)
            }
            InspectOperation::Export {
                artifact,
                format,
                out,
                entry,
                state,
                width,
                artifact_url,
                comment_key,
            } => {
                export(
                    &artifact,
                    format,
                    &out,
                    entry.as_deref(),
                    state.as_deref(),
                    width,
                    artifact_url,
                    comment_key,
                )?;
                Ok(0)
            }
        };
    }
    let path = args
        .artifact
        .ok_or_else(|| CliError::usage("inspect requires ARTIFACT or a named operation"))?;
    if let Some(expected) = args.expected_case_id {
        let report = crate::read_report(&path)?;
        let case = case_for_result(&report, &path)?;
        if case.case_id.as_str() != expected {
            return Err(CliError::new(
                "stale_action",
                "expected_case_id does not match current evidence",
            ));
        }
        verify_case_files(&case, &path.with_file_name("evidence.json"))?;
    }
    let value = inspect_page(
        &path,
        args.entry.as_deref(),
        &args.status,
        args.limit,
        args.cursor.as_deref(),
    )?;
    print(&value, args.json)?;
    Ok(0)
}
pub(crate) fn print(value: &Value, json: bool) -> Result<(), CliError> {
    if json {
        return crate::emit(&format!("{}\n", serde_json::to_string(value)?));
    }
    let mut text = format!(
        "{}: {}\n",
        value["operation"].as_str().unwrap_or("operation"),
        value["execution"].as_str().unwrap_or("unknown")
    );
    for entry in value["entries"].as_array().into_iter().flatten() {
        text.push_str(&format!(
            "{}: {}\n",
            entry["entry_id"].as_str().unwrap_or("unknown"),
            entry["measurement"].as_str().unwrap_or("unknown")
        ));
    }
    if let Some(path) = value["artifact"]["path"].as_str() {
        text.push_str(&format!("artifact: {path}\n"));
    }
    for limit in value["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        text.push_str(&format!("limit: {limit}\n"));
    }
    crate::emit(&text)
}
pub(crate) fn failing_cursor(
    path: &Path,
    offset: usize,
    total: usize,
) -> Result<Option<String>, CliError> {
    let digest = canonical::digest(
        &json!({"sha256":reference(path)?["sha256"],"selection":"failing","limit":10}),
    )?;
    Ok((offset < total).then(|| format!("failing:{}:{offset}", digest.as_str())))
}

pub(crate) fn inspect_page(
    path: &Path,
    entry: Option<&str>,
    status: &[String],
    limit: usize,
    cursor: Option<&str>,
) -> Result<Value, CliError> {
    if !(1..=10).contains(&limit) {
        return Err(CliError::usage("--limit must be 1..10"));
    }
    let doc = read_value(path)?;
    let mut result = base_result("inspect");
    let budget = if limit <= 5 { 4096 } else { 8192 };
    result["artifact"] = reference(path)?;
    if doc["schema"] != saccade_core::report::REPORT_SCHEMA {
        let document: Document = serde_json::from_value(doc.clone()).map_err(|_| {
            CliError::usage("inspect expects a measured report or canonical evidence artifact")
        })?;
        document.validate()?;
        result["data"] = json!({"kind":doc["kind"],"authority":document.authority()});
        if let Some(authority) = document.authority() {
            result["limits"] = json!([format!(
                "authority: {authority}; no human approval is inferred."
            )]);
        }
        result["counts"]["records"] = json!(1);
        if let Artifact::Case(case) = document.artifact {
            result["validity"] = serde_json::to_value(case.validity.status)?;
            result["validity_reasons"] = validity_summary(&case.validity.reasons);
            result["counts"]["validity_reasons"] = json!(case.validity.reasons.len());
            result["counts"]["requests"] = json!(case.requests.len());
            result["counts"]["proposals"] = json!(case.proposals.len());
        }
        return bounded(result, budget);
    }
    let report: saccade_core::Report = serde_json::from_value(doc)?;
    if let Some(cursor) = cursor.and_then(|c| c.strip_prefix("failing:")) {
        if entry.is_some() || !status.is_empty() || limit != 10 {
            return Err(CliError::new(
                "stale_cursor",
                "summary continuation requires the original failing selection and page limit 10",
            ));
        }
        let (_, offset) = cursor
            .rsplit_once(':')
            .ok_or_else(|| CliError::usage("invalid cursor"))?;
        let offset = offset
            .parse::<usize>()
            .map_err(|_| CliError::usage("invalid cursor offset"))?;
        let entries = crate::agent::failing_entries(&report);
        let expected = failing_cursor(path, offset, entries.len())?;
        if expected.as_deref() != Some(&format!("failing:{cursor}")) {
            return Err(CliError::new(
                "stale_cursor",
                "cursor does not match artifact or selected records",
            ));
        }
        result["counts"] = json!({"total":entries.len()});
        result["validity"] = json!(report.capture_validity().status);
        result["validity_reasons"] = validity_summary(&report.capture_validity().reasons);
        result["entries"] = json!(entries.iter().skip(offset).take(limit).map(|e| json!({"entry_id":e.name,"measurement":if e.status==saccade_core::Status::Fail{"regression"}else{"unknown"},"error":e.error.as_ref().map(|s|short(s,256))})).collect::<Vec<_>>());
        result["page"]["omitted"] = json!(entries.len().saturating_sub(offset + limit));
        let mut result = bounded(result, budget)?;
        let shown = result["entries"].as_array().map_or(0, Vec::len);
        result["page"] = json!({"omitted":entries.len().saturating_sub(offset+shown),"next_cursor":failing_cursor(path,offset+shown,entries.len())?});
        if shown == 0 && offset < entries.len() {
            return Err(CliError::new(
                "output_budget",
                "entry ID exceeds page budget; export full artifact",
            ));
        }
        return Ok(result);
    }
    let binding = canonical::digest(
        &json!({"sha256":reference(path)?["sha256"],"entry":entry,"status":status,"limit":limit}),
    )?;
    let offset = match cursor {
        None => 0,
        Some(c) => {
            let (digest, offset) = c
                .rsplit_once(':')
                .ok_or_else(|| CliError::usage("invalid cursor"))?;
            if digest != binding.as_str() {
                return Err(CliError::new(
                    "stale_cursor",
                    "cursor does not match this artifact and selection",
                ));
            }
            offset
                .parse::<usize>()
                .map_err(|_| CliError::usage("invalid cursor offset"))?
        }
    };
    let page = saccade_core::ergonomics::entries(&report, status, entry, offset, limit)?;
    result["counts"] = json!({"total":page.total});
    result["validity"] = serde_json::to_value(report.capture_validity().status)?;
    result["validity_reasons"] =
        crate::local_cmd::validity_summary(&report.capture_validity().reasons);
    // Retain invariant statuses and references. Large strings stay in the full artifact.
    let summaries = page.entries.iter().map(|e| json!({"entry_id":e.name,"measurement":match e.status {saccade_core::Status::Pass=>"pass",saccade_core::Status::Fail=>"regression",_=>"unknown"},"error":e.error.as_ref().map(|s|short(s,256))})).collect::<Vec<_>>();
    result["entries"] = json!(summaries);
    result["page"] = json!({"omitted":page.total.saturating_sub(offset+page.entries.len()),"next_cursor":page.next_cursor.map(|n|format!("{}:{n}",binding.as_str()))});
    let value = bounded(result, budget)?;
    let mut value = value;
    let shown = value["entries"].as_array().map_or(0, Vec::len);
    let next = offset + shown;
    value["page"] = json!({"omitted":page.total.saturating_sub(next),"next_cursor":(next<page.total).then(||format!("{}:{next}",binding.as_str()))});
    if shown == 0 && next < page.total {
        return Err(CliError::new(
            "output_budget",
            "one entry ID exceeds the page budget; export the full artifact",
        ));
    }
    Ok(value)
}
pub(crate) fn bounded(mut value: Value, bytes: usize) -> Result<Value, CliError> {
    while serde_json::to_vec(&value)?.len() > bytes {
        if let Some(entries) = value["entries"].as_array_mut()
            && !entries.is_empty()
        {
            entries.pop();
            if let Some(failing) = value.get_mut("failing").and_then(Value::as_array_mut) {
                failing.pop();
            }
            let omitted = value["page"]["omitted"].as_u64().unwrap_or(0) + 1;
            value["page"]["omitted"] = json!(omitted);
        } else {
            return Err(CliError::new(
                "output_budget",
                "invariants exceed the text budget; inspect the full artifact",
            ));
        }
    }
    Ok(value)
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_evidence(
    report: &Path,
    out: &Path,
    entries: &[String],
    top: usize,
    stretch: bool,
    blind: bool,
    key_out: Option<PathBuf>,
    seed: Option<u64>,
    absolute: bool,
) -> Result<Value, CliError> {
    if top > 5 {
        return Err(CliError::usage("evidence supports at most five hotspots"));
    }
    let opts = saccade_core::explain::ExplainOptions {
        top,
        stretch,
        blind,
        key_out,
        seed,
        entries: entries.to_vec(),
        record_absolute_paths: absolute,
        ..Default::default()
    };
    saccade_core::explain::explain(report, out, &opts)?;
    let mut result = base_result("evidence");
    result["artifact"] = reference(&out.join("explain.json"))?;
    result["limits"] = json!([
        "The agent preparing this evidence has implementation context; independent blind review must exclude the key and that context."
    ]);
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
fn export(
    artifact: &Path,
    format: ExportFormat,
    out: &Path,
    entry: Option<&str>,
    state: Option<&str>,
    width: u32,
    artifact_url: Option<String>,
    comment_key: Option<String>,
) -> Result<(), CliError> {
    match format {
        ExportFormat::Png => {
            if entry.is_none() {
                return Err(CliError::usage("PNG export requires --entry"));
            }
            let snapshot = crate::agent_ui::render_snapshot(artifact, entry, state, width, out)?;
            crate::emit(&format!(
                "snapshot: {}x{}, {} frame(s)\n",
                snapshot.width,
                snapshot.height,
                snapshot.paths.len()
            ))?;
        }
        ExportFormat::Json => write_value(out, &read_value(artifact)?)?,
        ExportFormat::Markdown => {
            if comment_key
                .as_deref()
                .is_some_and(|s| !saccade_core::render::is_valid_comment_key(s))
            {
                return Err(CliError::usage("invalid --comment-key"));
            }
            let text = saccade_core::render::render_markdown(
                &crate::read_report(artifact)?,
                &saccade_core::render::MarkdownOptions {
                    artifact_url,
                    comment_key,
                    max_bytes: None,
                },
            );
            std::fs::write(out, text).map_err(|e| CliError::io(e.to_string()))?;
        }
        ExportFormat::Junit => {
            saccade_core::ergonomics::junit(&crate::read_report(artifact)?, out)?
        }
        ExportFormat::Labels => {
            let case_path = if artifact.is_dir() {
                let direct = artifact.join("evidence.json");
                if direct.is_file() {
                    direct
                } else {
                    artifact.join("reviewed/evidence.json")
                }
            } else {
                artifact.to_owned()
            };
            let doc = Document::read(&case_path)?;
            let Artifact::Case(mut case) = doc.artifact else {
                return Err(CliError::usage(
                    "label export requires an evidence case or review collection",
                ));
            };
            let collection = if artifact.is_dir() {
                artifact
            } else {
                artifact.parent().unwrap_or(Path::new("."))
            };
            if collection.join("decision.json").is_file() {
                let decision = Document::read(&collection.join("decision.json"))?;
                if let Artifact::HumanDecision(decision) = decision.artifact {
                    decision.validate_for(&case)?;
                    if !case
                        .human_decisions
                        .iter()
                        .any(|d| d.decision_id == decision.decision_id)
                    {
                        case.human_decisions.push(*decision);
                    }
                }
            }
            let records = collection.join("human-label-records.json");
            let labels = if records.is_file() {
                saccade_core::labels::QuestionLabels::read(
                    &records,
                    &case.requests,
                    &case.human_decisions,
                )?
                .eligible_labels(&case.requests, &case.human_decisions)?
            } else {
                saccade_core::evidence::human::Labels {
                    schema: "saccade-labels.v2".into(),
                    items: Vec::new(),
                }
            };
            write_value(out, &serde_json::to_value(labels)?)?;
        }
    }
    Ok(())
}
pub(crate) fn review(args: ReviewArgs, _absolute: bool) -> Result<u8, CliError> {
    let value = if let Some(operation) = args.operation {
        match operation {
            ReviewOperation::Request {
                report,
                question,
                out,
            } => request(&report, &question, &out)?,
            ReviewOperation::Propose {
                request,
                answers,
                out,
            } => propose(&request, &answers, out.as_deref())?,
            ReviewOperation::Ask { request, out } => ask(&request, out.as_deref())?,
            ReviewOperation::Eval { manifest, run } => {
                #[cfg(feature = "evaluation")]
                {
                    crate::review_cmd::evaluate(&manifest, run, args.user_config.as_deref())?
                }
                #[cfg(not(feature = "evaluation"))]
                {
                    let _ = (manifest, run);
                    return Err(CliError::new(
                        "feature_unavailable",
                        "evaluation requires evaluation",
                    ));
                }
            }
        }
    } else {
        #[cfg(feature = "ai")]
        if args.run || args.user_config.is_some() {
            let value = crate::review_cmd::cli(&args)?;
            print(&value, args.json)?;
            return Ok(0);
        }
        #[cfg(not(feature = "ai"))]
        if args.run {
            return Err(CliError::new(
                "feature_unavailable",
                "review execution requires ai",
            ));
        }

        let report = args
            .report
            .ok_or_else(|| CliError::usage("review requires REPORT or a named operation"))?;
        preview(
            &report,
            args.budget_calls,
            args.intent.as_deref(),
            args.intent_file.as_deref(),
        )?
    };
    print(&value, args.json)?;
    Ok(0)
}
pub(crate) fn preview(
    report: &Path,
    budget: Option<u64>,
    intent: Option<&str>,
    intent_file: Option<&Path>,
) -> Result<Value, CliError> {
    #[cfg(feature = "ai")]
    {
        if let Some(p) = intent_file {
            read_value(p)?;
        }
        let intent = crate::review_cmd::read_intent(intent, intent_file, report)?;
        crate::review_cmd::preview_local(report, budget.unwrap_or(24), intent)
    }
    #[cfg(not(feature = "ai"))]
    let mut value = inspect_page(report, None, &[], 5, None)?;
    #[cfg(not(feature = "ai"))]
    {
        value["operation"] = json!("review.preview");
        if let Some(p) = intent_file {
            read_value(p)?;
        }
        value["counts"]["budget_calls"] = json!(budget.unwrap_or(0));
        value["counts"]["dispatched_calls"] = json!(0);
        value["limits"] = json!([
            "Local plan only. External payload preparation and execution require R9–R11.",
            if intent.is_some() {
                "Text intent has lower assurance."
            } else {
                "No structured intent was supplied."
            }
        ]);
        bounded(value, 4096)
    }
}
pub(crate) fn request(case_file: &Path, question_id: &str, out: &Path) -> Result<Value, CliError> {
    #[cfg(feature = "ai")]
    {
        let request = crate::review_cmd::prepare_request(case_file, question_id)?;
        let mut document =
            serde_json::to_value(Document::new(Artifact::DecisionRequest(Box::new(request))))?;
        crate::review_cmd::rebase(&mut document, case_file, out)?;
        write_value(out, &document)?;
        let mut value = base_result("review.request");
        value["artifact"] = reference(out)?;
        Ok(value)
    }
    #[cfg(not(feature = "ai"))]
    {
        let doc = Document::read(case_file)?;
        let Artifact::Case(case) = doc.artifact else {
            return Err(CliError::new(
                "operation_unavailable",
                "R5 can export an existing closed request from a case; report question generation arrives in R9",
            ));
        };
        let request = case
            .requests
            .iter()
            .find(|r| r.question.id == question_id)
            .ok_or_else(|| {
                CliError::new(
                    "operation_unavailable",
                    "question is not present in this case; the question catalog arrives in R9",
                )
            })?;
        request.validate_for(&case)?;
        write_value(
            out,
            &serde_json::to_value(Document::new(Artifact::DecisionRequest(Box::new(
                request.clone(),
            ))))?,
        )?;
        let mut value = base_result("review.request");
        value["artifact"] = reference(out)?;
        Ok(value)
    }
}
fn read_request(path: &Path) -> Result<saccade_core::evidence::request::DecisionRequest, CliError> {
    let document = Document::read(path)?;
    let Artifact::DecisionRequest(request) = document.artifact else {
        return Err(CliError::usage("expected a canonical decision_request"));
    };
    Ok(*request)
}
pub(crate) fn propose(
    request: &Path,
    answers: &Path,
    out: Option<&Path>,
) -> Result<Value, CliError> {
    let request = read_request(request)?;
    let doc = Document::read(answers)?;
    let Artifact::DecisionProposal(proposal) = doc.artifact else {
        return Err(CliError::usage(
            "answers must be a canonical decision_proposal with adapter audit identity",
        ));
    };
    proposal.validate_for(&request)?;
    let destination = out.map(Path::to_path_buf).unwrap_or_else(|| {
        answers.with_file_name(format!("proposal-{}.json", proposal.proposal_id.as_str()))
    });
    write_value(
        &destination,
        &serde_json::to_value(Document::new(Artifact::DecisionProposal(proposal)))?,
    )?;
    let mut value = base_result("review.propose");
    value["artifact"] = reference(&destination)?;
    value["review"] = json!("unresolved");
    Ok(value)
}
pub(crate) fn ask(request_file: &Path, out: Option<&Path>) -> Result<Value, CliError> {
    let request = read_request(request_file)?;
    let destination = out.map(Path::to_path_buf).unwrap_or_else(|| {
        request_file.with_file_name(format!("human-{}.json", request.request_id.as_str()))
    });
    let doc = serde_json::to_value(Document::new(Artifact::DecisionRequest(Box::new(request))))?;
    if destination.exists() && read_value(&destination)? != doc {
        return Err(CliError::new(
            "invalid_evidence",
            "human review item has changed",
        ));
    }
    if !destination.exists() {
        write_value(&destination, &doc)?;
    }
    let mut value = base_result("review.ask");
    value["artifact"] = reference(&destination)?;
    value["execution"] = json!("pending");
    value["review"] = json!("unresolved");
    Ok(value)
}
pub(crate) fn view_artifact(path: &Path, out: &Path, json: bool) -> Result<u8, CliError> {
    if path.is_dir() && path.join(".saccade-demo").is_file() {
        return view_artifact(&path.join("report"), out, json);
    }
    let parent = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new("."))
    };
    let index = parent.join("index.html");
    if !index.is_file() {
        return Err(CliError::usage("artifact has no rendered index.html"));
    }
    let mut value = base_result("view");
    value["artifact"] = reference(&index)?;
    let _ = out;
    print(&value, json)?;
    Ok(0)
}
pub(crate) fn unblind(
    decisions: &Path,
    key: &Path,
    out: &Path,
    absolute: bool,
) -> Result<u8, CliError> {
    let decisions = saccade_core::view::read_decisions(decisions)?;
    let mapping = saccade_core::view::read_blind_key(key)?;
    let mut resolved = saccade_core::view::unblind(&decisions, &mapping)?;
    saccade_core::paths::rebase_decisions(&mut resolved, key, out, absolute);
    write_value(out, &serde_json::to_value(resolved)?)?;
    Ok(0)
}

pub(crate) fn report_input(
    report: &saccade_core::Report,
    report_file: &Path,
    name: &str,
    baseline: bool,
) -> Option<PathBuf> {
    let root = if baseline {
        report.baseline_dir.as_ref()
    } else {
        report.capture_dir.as_ref()
    }?;
    let root = saccade_core::paths::resolve(root, report_file);
    Some(if root.is_file() {
        root
    } else {
        root.join(name)
    })
}

pub(crate) fn case_from_report(
    report: &saccade_core::Report,
    report_file: &Path,
) -> Result<EvidenceCase, CliError> {
    let document = report_file.with_file_name("evidence.json");
    let entries = report
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    let measurement = Measurement::from_report(
        report_file,
        &document,
        entries.clone(),
        "native+flip.v1".into(),
        "saccade-config.v1".into(),
    )?;
    let mut inputs = Vec::new();
    for entry in &report.entries {
        for (role, path, hash) in [
            ("baseline", &entry.paths.baseline, &entry.baseline_sha256),
            ("capture", &entry.paths.capture, &entry.capture_sha256),
        ] {
            if let (Some(_path), Some(hash)) = (path, hash) {
                let input_path = report_input(report, report_file, &entry.name, role == "baseline")
                    .ok_or_else(|| {
                        CliError::new("invalid_evidence", "measurement has no source directory")
                    })?;
                let root = if role == "baseline" {
                    report.baseline_dir.as_deref()
                } else {
                    report.capture_dir.as_deref()
                }
                .map(|p| saccade_core::paths::resolve(p, report_file))
                .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new(".")).to_owned());
                let root = if root.is_file() {
                    root.parent().unwrap_or(Path::new(".")).to_owned()
                } else {
                    root
                };
                let mut sidecars = Vec::new();
                let mut directories = Vec::new();
                let mut current = input_path.parent();
                while let Some(dir) = current {
                    if !dir.starts_with(&root) {
                        break;
                    }
                    directories.push(dir.to_owned());
                    if dir == root {
                        break;
                    }
                    current = dir.parent();
                }
                directories.reverse();
                for dir in directories {
                    let file = dir.join(&report.config.meta.name);
                    if file.is_file() {
                        sidecars.push(ArtifactRef {
                            path: lexical_record(
                                &file,
                                document.parent().unwrap_or(Path::new(".")),
                            ),
                            sha256: Digest::of_bytes(
                                &std::fs::read(&file).map_err(|e| CliError::io(e.to_string()))?,
                            ),
                        });
                    }
                }
                if let Some(stem) = input_path.file_stem().and_then(|s| s.to_str()) {
                    let file =
                        input_path.with_file_name(format!("{stem}.{}", report.config.meta.name));
                    if file.is_file() {
                        sidecars.push(ArtifactRef {
                            path: lexical_record(
                                &file,
                                document.parent().unwrap_or(Path::new(".")),
                            ),
                            sha256: Digest::of_bytes(
                                &std::fs::read(&file).map_err(|e| CliError::io(e.to_string()))?,
                            ),
                        });
                    }
                }
                inputs.push(Input {
                    id: format!("{role}:{}", entry.name),
                    content: ArtifactRef {
                        path: lexical_record(
                            &input_path,
                            document.parent().unwrap_or(Path::new(".")),
                        ),
                        sha256: Digest::parse(format!("sha256:{hash}"))?,
                    },
                    sidecars,
                    native_samples: Availability::missing(
                        "Native interpretation is recorded in the authoritative measurement.",
                    ),
                    capture: Availability::missing("No typed capture context was supplied."),
                    build: Availability::missing("No build identity was supplied."),
                    provenance: Provenance {
                        source_roots: saccade_core::paths::source_paths(&root)
                            .map_err(|e| CliError::io(e.to_string()))?,
                        ..Default::default()
                    },
                });
            }
        }
    }
    let valid = report.capture_validity();
    let validity = Validity {
        status: match valid.status {
            saccade_core::meta::Validity::Valid => ValidityStatus::Valid,
            saccade_core::meta::Validity::Invalid => ValidityStatus::Invalid,
            _ => ValidityStatus::Unknown,
        },
        reasons: valid.reasons,
    };
    let source_roots = inputs
        .iter()
        .flat_map(|i| i.provenance.source_roots.clone())
        .collect();
    let mut case = EvidenceCase {
        case_id: Digest::of_bytes(b""),
        inputs,
        measurement,
        scope: Scope {
            entries,
            exclusions: report.config.ignore.clone(),
        },
        effective_config: serde_json::from_value(serde_json::to_value(&report.config)?)?,
        calibration: Availability::missing("No calibration input was supplied."),
        validity,
        facts: Vec::new(),
        intent: Availability::missing("No declared intent was supplied."),
        requests: Vec::new(),
        proposals: Vec::new(),
        human_decisions: Vec::new(),
        next_actions: Vec::new(),
        limits: vec!["Measurement does not confer human approval.".into()],
        provenance: Provenance {
            source_roots,
            ..Default::default()
        },
    };
    for entry in &report.entries {
        case.facts.push(Fact {
            id: format!("native-equality:{}", entry.name),
            name: "native decoded-sample equality".into(),
            units: "boolean".into(),
            scope: Scope {
                entries: vec![entry.name.clone()],
                exclusions: Vec::new(),
            },
            source: FactSource::Measured,
            artifact: case.measurement.report.clone(),
            source_identity: case.measurement.semantic_sha256.clone(),
            value: entry.bit_identical.map_or_else(
                || Availability::missing("No complete decoded pair was measured."),
                |v| Availability::Available {
                    value: FactValue::Boolean(v),
                },
            ),
            depends_on_model_observation: false,
            observation_refs: Vec::new(),
        });
    }
    case.refresh_id()?;
    Ok(case)
}

pub(crate) fn short(text: &str, chars: usize) -> String {
    if text.chars().count() > chars {
        format!(
            "{}… (full text in artifact)",
            text.chars().take(chars).collect::<String>()
        )
    } else {
        text.into()
    }
}
pub(crate) fn validity_summary(reasons: &[String]) -> Value {
    if reasons.iter().map(String::len).sum::<usize>() <= 512 {
        json!(reasons)
    } else {
        json!([format!(
            "{} failed or unknown capture requirements; all reasons remain in the hashed artifact.",
            reasons.len()
        )])
    }
}
#[cfg(feature = "mcp")]
pub(crate) fn measure_noise(
    dirs: &[PathBuf],
    out: &Path,
    kind: &str,
    margin: f64,
    metric: saccade_core::Metric,
    perf: &saccade_core::perf::PerfOptions,
) -> Result<Value, CliError> {
    let mut value = base_result(&format!("noise.{kind}"));
    match kind {
        "image" => {
            let report =
                saccade_core::ergonomics::noise_with_perf_options(dirs, margin, metric, out, perf)?;
            let full = serde_json::to_value(report)?;
            let artifact = out.with_extension("json");
            write_value(&artifact, &full)?;
            value["artifact"] = reference(&artifact)?;
        }
        "performance" => {
            return crate::perf_cmd::noise(dirs, out, perf, false);
        }
        _ => return Err(CliError::usage("noise kind must be image or performance")),
    }
    value["counts"] = json!({"runs":dirs.len()});
    value["data"] = json!({"kind":"image_noise","unit":"FLIP"});
    Ok(value)
}

pub(crate) fn persist_case(
    report: &saccade_core::Report,
    report_file: &Path,
    args: &crate::IntentArgs,
) -> Result<(), CliError> {
    if report.entries.is_empty() {
        return Ok(());
    }
    let mut case = case_from_report(report, report_file)?;
    if case.inputs.is_empty() {
        return Ok(());
    }
    let source = report_file.with_file_name("evidence.json");
    let dir = report_file.parent().unwrap_or(Path::new("."));
    if let Some(bundled) = saccade_core::render::bundle::measured_case(report, dir)? {
        for (index, input) in case.inputs.iter_mut().enumerate() {
            let copy = bundled
                .inputs
                .iter()
                .find(|i| i.id == input.id)
                .ok_or_else(|| CliError::new("invalid_evidence", "missing portable input"))?;
            input.content = copy.content.clone();
            for (sidecar_index, sidecar) in input.sidecars.iter_mut().enumerate() {
                sidecar.verify(&source)?;
                let bytes = std::fs::read(saccade_core::paths::resolve(&sidecar.path, &source))
                    .map_err(|e| CliError::io(e.to_string()))?;
                let target = dir.join(format!("assets/input-{index}-sidecar-{sidecar_index}.json"));
                std::fs::write(&target, bytes).map_err(|e| CliError::io(e.to_string()))?;
                *sidecar = ArtifactRef::from_file(&target, &source, false)?;
            }
        }
    }
    if let Some(file) = &args.intent_file {
        let mut intent: Intent = serde_json::from_value(read_value(file)?)?;
        intent.assurance = IntentAssurance::Structured;
        let target = dir.join("assets/intent.json");
        std::fs::copy(file, &target).map_err(|e| CliError::io(e.to_string()))?;
        intent.source = Some(ArtifactRef::from_file(&target, &source, false)?);
        case.intent = Availability::Available { value: intent };
    } else if let Some(text) = &args.intent {
        case.intent = Availability::Available {
            value: Intent {
                id: "declared-intent".into(),
                objective: text.clone(),
                assurance: IntentAssurance::Text,
                expected_changes: Vec::new(),
                invariants: Vec::new(),
                criteria: Vec::new(),
                source: None,
                provenance: Provenance::default(),
            },
        };
    }
    if let Some(file) = &args.changes_file {
        let changes: Vec<DeclaredChange> = serde_json::from_value(read_value(file)?)?;
        if let Availability::Available { value: intent } = &mut case.intent {
            intent.expected_changes = changes;
        } else {
            return Err(CliError::usage(
                "--changes-file requires --intent or --intent-file",
            ));
        }
    }
    case.refresh_id()?;
    case.validate()?;
    write_value(
        &source,
        &serde_json::to_value(Document::new(Artifact::Case(Box::new(case))))?,
    )?;
    saccade_core::render::render_html(report, dir)?;
    Ok(())
}
pub(crate) fn case_for_result(
    report: &saccade_core::Report,
    report_file: &Path,
) -> Result<EvidenceCase, CliError> {
    let file = report_file.with_file_name("evidence.json");
    if file.is_file() {
        let doc = Document::read(&file)?;
        if let Artifact::Case(case) = doc.artifact {
            if case.measurement.semantic_sha256 != Measurement::report_identity(report)? {
                return Err(CliError::new("stale_evidence", "case and report differ"));
            }
            return Ok(*case);
        }
    }
    case_from_report(report, report_file)
}

pub(crate) fn lexical_record(path: &Path, base: &Path) -> String {
    let source = path.components().collect::<Vec<_>>();
    let root = base.components().collect::<Vec<_>>();
    let shared = source.iter().zip(&root).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return saccade_core::paths::portable(path);
    }
    let mut parts = vec!["..".to_owned(); root.len() - shared];
    parts.extend(
        source[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

fn verify_case_files(case: &EvidenceCase, document: &Path) -> Result<(), CliError> {
    case.measurement.report.verify(document)?;
    for input in &case.inputs {
        input.content.verify(document)?;
        for sidecar in &input.sidecars {
            sidecar.verify(document)?;
        }
    }
    if let Availability::Available { value: intent } = &case.intent
        && let Some(source) = &intent.source
    {
        source.verify(document)?;
    }
    Ok(())
}
