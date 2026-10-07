//! Public asynchronous assist Batch commands, shared by CLI and MCP.
use crate::{
    agent::CliError,
    assist_cmd::{self, Common, Routing, VisibleKind},
    local_cmd,
};
use clap::{Args, Subcommand};
use saccade_core::{
    assist::{
        self,
        batch::{self, FrozenPlan, State},
        execution::{self, Executor},
        schema::Task,
        workflow,
    },
    budget_ledger::{Ledger, MoneyScope},
    judge_provider::{
        batch::DeadlineBatchNetwork,
        transport::{self, Authorization},
    },
    root_policy::RootPolicy,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Args)]
pub(crate) struct AssistArgs {
    #[command(subcommand)]
    pub operation: AssistOperation,
}
#[derive(Subcommand)]
pub(crate) enum AssistOperation {
    /// Asynchronous frozen evaluation jobs; never used by interactive advice.
    Batch(BatchArgs),
}
#[derive(Args)]
pub(crate) struct BatchArgs {
    #[command(subcommand)]
    pub operation: BatchOperation,
}
#[derive(Subcommand)]
pub(crate) enum BatchOperation {
    /// Verify and submit once; ambiguous submissions cannot repeat.
    Submit(BatchCommon),
    /// Read local status, or poll once with --run.
    Status(BatchCommon),
    /// Collect once and settle terminal known usage; never wait.
    Collect(BatchCommon),
}
#[derive(Args)]
pub(crate) struct BatchCommon {
    /// Explicitly acknowledge a plan allowance above the default 25 USD ceiling.
    #[arg(long)]
    pub allow_spend_above_25_usd: bool,
    /// Source-bound saccade-assist-batch-plan.v1 artifact.
    #[arg(long)]
    pub plan: PathBuf,
    /// Durable receipt under the output root.
    #[arg(long)]
    pub job: PathBuf,
    #[arg(long)]
    pub experimental: bool,
    /// Authorize one live submission or one poll; default local only.
    #[arg(long, conflicts_with = "response")]
    pub run: bool,
    /// Recorded collection fixture; cannot settle a live reservation.
    #[arg(long)]
    pub response: Option<PathBuf>,
    /// Maximum provider calls, 1-128 (default 8); the run stops when reached.
    #[arg(long,default_value_t=8,value_parser=clap::value_parser!(u64).range(1..=128))]
    pub budget_calls: u64,
    /// Wall-clock limit in seconds, 1-300 (default 300).
    #[arg(long,default_value_t=300,value_parser=clap::value_parser!(u64).range(1..=300))]
    pub deadline_secs: u64,
}
pub(crate) fn run(
    args: AssistArgs,
    json_output: bool,
    user_file: Option<&Path>,
) -> Result<u8, CliError> {
    let AssistOperation::Batch(args) = args.operation;
    let (operation, args) = match args.operation {
        BatchOperation::Submit(a) => ("submit", a),
        BatchOperation::Status(a) => ("status", a),
        BatchOperation::Collect(a) => ("collect", a),
    };
    let (value, exit) = execute(operation, args, user_file, None, None)?;
    local_cmd::print(&value, json_output)?;
    Ok(exit)
}
fn common(task: &batch::SourceTask, user_file: Option<&Path>) -> Common {
    Common {
        #[cfg(feature = "vision-providers")]
        vision_provider: None,
        #[cfg(feature = "vision-providers")]
        vision_response: None,
        user_policy_file: user_file.map(Path::to_owned),
        experimental: true,
        out: PathBuf::new(),
        offline: true,
        replay: None,
        run: false,
        route: Routing::AllVision,
        budget_calls: 8,
        max_spend_usd: 0.15,
        deadline_secs: 300,
        gemini_revision: None,
        jev_revision: saccade_core::assist::schema::JEV.into(),
        bypass_cache: true,
        source_evidence: task
            .source_evidence
            .iter()
            .map(|r| PathBuf::from(&r.path))
            .collect(),
        incomplete_capture: task.incomplete_capture,
        pre_masked: task.pre_masked,
        jev_routing: false,
    }
}
pub(crate) fn verify_sources(
    frozen: &FrozenPlan,
    roots: Option<&RootPolicy>,
    user_file: Option<&Path>,
) -> Result<Vec<String>, CliError> {
    frozen.validate()?;
    let user = crate::review_cmd::load_user(&crate::review_cmd::user_file(user_file))?;
    let mut sources = Vec::new();
    for task in &frozen.tasks {
        for source in &task.transitive {
            let path = if let Some(roots) = roots {
                roots.read(Path::new(&source.path))?
            } else {
                PathBuf::from(&source.path)
            };
            if source.sha256
                == saccade_core::evidence::canonical::Digest::of_bytes(&assist::read_bytes(
                    &path,
                    32 * 1024 * 1024,
                )?)
            {
                sources.push(saccade_core::paths::portable(&path));
            } else {
                return Err(CliError::new(
                    "invalid_evidence",
                    "frozen transitive source bytes changed",
                ));
            }
        }
        let args = common(task, user_file);
        let input = if task.task == Task::CheckUi {
            if task.entry.is_some() || task.mask_manifest.is_some() {
                return Err(CliError::usage(
                    "check-ui Batch descriptor contains report fields",
                ));
            }
            let kind = match task.condition_kind.as_deref() {
                Some("label-visible") => VisibleKind::LabelVisible,
                Some("banner-absent") => VisibleKind::BannerAbsent,
                Some("not-clipped") => VisibleKind::NotClipped,
                Some("non-overlap") => VisibleKind::NonOverlap,
                _ => return Err(CliError::usage("Batch visible condition kind")),
            };
            assist_cmd::check_input(
                &assist_cmd::CheckArgs {
                    condition: task
                        .condition_text
                        .clone()
                        .ok_or_else(|| CliError::usage("Batch condition required"))?,
                    image: PathBuf::from(&task.artifact.path),
                    r#box: task
                        .box_px
                        .ok_or_else(|| CliError::usage("Batch box required"))?,
                    kind,
                    target: task.target.clone(),
                    second_target: task.second_target.clone(),
                    grounding: Default::default(),
                    common: args,
                },
                roots,
            )?
        } else {
            if task.condition_text.is_some()
                || task.condition_kind.is_some()
                || task.box_px.is_some()
                || task.target.is_some()
                || task.second_target.is_some()
            {
                return Err(CliError::usage(
                    "report Batch descriptor contains check-ui fields",
                ));
            }
            assist_cmd::report_input(
                &assist_cmd::ReportArgs {
                    report: PathBuf::from(&task.artifact.path),
                    entry: task.entry.clone(),
                    mask_manifest: task.mask_manifest.as_ref().map(|r| PathBuf::from(&r.path)),
                    common: args,
                },
                task.task,
                roots,
            )?
        };
        let mut reproduced = input.source.clone();
        reproduced.job_ids = task.job_ids.clone();
        if reproduced != *task {
            return Err(CliError::new(
                "invalid_evidence",
                "Batch transitive source closure or descriptor differs",
            ));
        }
        if matches!(
            workflow::evidence_need(&input.catalog, input.condition.as_ref(), input.task)?,
            workflow::Need::Unavailable(_)
        ) || (input.task == Task::AuditMask && input.catalog.exclusions.is_empty())
        {
            return Err(CliError::new(
                "invalid_evidence",
                "Batch required evidence unavailable",
            ));
        }
        let identity = input.catalog.identity(
            input.task,
            input.report_hash.clone(),
            input.condition.as_ref(),
        )?;
        let mut ids = Vec::new();
        for reverse in if input.catalog.images.len() == 2 {
            vec![false, true]
        } else {
            vec![false]
        } {
            let request = workflow::prepare(
                &input.catalog,
                identity.clone(),
                input.condition.as_ref(),
                &input.pngs,
                reverse,
                &frozen.plan.revision,
                assist::digest(&user)?,
            )?;
            let id = request.key.payload_hash.as_str();
            ids.push(id.to_owned());
            let expected = saccade_core::judge_provider::batch::inline_request(
                id,
                assist::decode(&request.payload)?,
            )
            .map_err(|_| CliError::usage("Batch reproduction request"))?;
            if !frozen.plan.requests.contains(&expected) {
                return Err(CliError::new(
                    "invalid_evidence",
                    "Batch request does not reproduce from source files",
                ));
            }
        }
        if ids != task.job_ids {
            return Err(CliError::new(
                "invalid_evidence",
                "Batch order binding differs",
            ));
        }
    }
    sources.sort();
    sources.dedup();
    Ok(sources)
}
pub(crate) fn execute(
    operation: &str,
    args: BatchCommon,
    user_file: Option<&Path>,
    roots: Option<&RootPolicy>,
    startup: Option<&Authorization>,
) -> Result<(Value, u8), CliError> {
    if !args.experimental
        || args.budget_calls == 0
        || args.budget_calls > 128
        || args.deadline_secs == 0
        || args.deadline_secs > 300
        || (args.run && args.response.is_some())
        || (args.response.is_some() && operation != "collect")
    {
        return Err(CliError::usage(
            "Batch requires experimental and bounded mutually exclusive controls",
        ));
    }
    let user = crate::review_cmd::load_user(&crate::review_cmd::user_file(user_file))?;
    let mut policy;
    let roots = if let Some(roots) = roots {
        Some(roots)
    } else if args.run {
        policy = RootPolicy::new(
            &user
                .roots
                .iter()
                .map(|r| r.path.clone())
                .collect::<Vec<_>>(),
            user.out_root.as_deref(),
            false,
            &[],
        )?;
        user.apply(&mut policy)
            .map_err(|_| CliError::new("egress_denied", "Batch user root policy"))?;
        Some(&policy)
    } else {
        None
    };
    let plan_path = if let Some(roots) = roots {
        roots.read(&args.plan)?
    } else {
        args.plan.clone()
    };
    let frozen: FrozenPlan = assist::decode(&assist::read_bytes(&plan_path, 32 * 1024 * 1024)?)?;
    if frozen.plan.max_spend_nano_usd > 25_000_000_000 && !args.allow_spend_above_25_usd {
        return Err(CliError::usage("spend_cap_above_25_requires_explicit_flag"));
    }
    let sources = verify_sources(&frozen, roots, user_file)?;
    let job_path = if let Some(roots) = roots {
        roots.write(&args.job)?
    } else {
        args.job.clone()
    };
    if operation == "submit" {
        batch::plan(&job_path, &frozen.plan)?;
    }
    let preview = json!({"schema":crate::advice_cmd::PREVIEW_SCHEMA,"provider":"gemini","model":saccade_core::assist::schema::GEMINI,"required_revision":frozen.plan.revision,"payload_file":saccade_core::paths::portable(&plan_path),"request_bytes":serde_json::to_vec(&frozen.plan.requests)?.len(),"estimated_cost_usd":frozen.plan.max_spend_nano_usd as f64 / 1e9,"cost_basis":"frozen plan maximum spend allowance","dispatch_requested":args.run,"authority":{"approve_baselines":false,"create_exclusions":false,"qualify_timing":false}});
    // Preview the frozen request set before submission or status HTTP.
    if args.run {
        eprintln!("{}", serde_json::to_string(&preview)?);
    }
    batch::bind_sources(&job_path, &frozen)?;
    let mut job = batch::receipt(&job_path, &frozen.plan)?;
    let ledger = Ledger::new(
        &saccade_core::judge_provider::Keys::default_dir().join("attempts"),
        false,
    );
    let mut remote = Value::Null;
    if let Some(response) = &args.response {
        if job.money_id.is_some() {
            return Err(CliError::usage(
                "recorded response cannot settle a live Batch reservation",
            ));
        }
        let path = if let Some(roots) = roots {
            roots.read(response)?
        } else {
            response.clone()
        };
        job = batch::collect(
            &job_path,
            &frozen.plan,
            &assist::decode(&assist::read_bytes(&path, 32 * 1024 * 1024)?)?,
        )?;
    } else if args.run {
        let roots = roots.ok_or_else(|| CliError::usage("Batch roots required"))?;
        let auth = if let Some(startup) = startup {
            if args.budget_calls > startup.scopes.first().map_or(0, |s| s.caps.total) {
                return Err(CliError::usage(
                    "Batch budget exceeds startup authorization",
                ));
            }
            let mut restricted = startup.clone();
            for scope in &mut restricted.scopes {
                scope.caps.total = scope.caps.total.min(args.budget_calls);
            }
            restricted
        } else {
            crate::review_cmd::authorization(
                true,
                args.budget_calls,
                &assist::digest(&frozen)?.as_str()[7..],
                &user,
            )
        };
        let keys = execution::fixed_keys()?;
        let network = transport::Network;
        let transport = transport::Transport {
            user: &user,
            roots,
            authorization: &auth,
            ledger: &ledger,
            keys: &keys,
            http: &network,
        };
        let executor = Executor {
            transport: &transport,
            ledger: &ledger,
            money_scopes: vec![MoneyScope {
                id: format!("assist/batch/{}", assist::digest(&frozen)?.as_str()),
                cap_nano_usd: frozen.plan.max_spend_nano_usd,
            }],
            sources,
            deadline: Instant::now() + Duration::from_secs(args.deadline_secs),
        };
        let batch_http = DeadlineBatchNetwork {
            deadline: executor.deadline,
        };
        if operation == "submit" {
            job = batch::submit_authorized(&job_path, &frozen.plan, &executor, &batch_http)?;
        } else if matches!(job.state, State::Completed | State::Partial | State::Failed) {
            if operation == "collect" {
                job = batch::settle(&job_path, &frozen.plan, &ledger)?;
            }
        } else {
            let response = batch::poll_authorized(&job_path, &frozen.plan, &executor, &batch_http)?;
            remote = json!(
                response["metadata"]["state"]
                    .as_str()
                    .or_else(|| response["state"].as_str())
                    .filter(
                        |s| s.len() <= 64 && s.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
                    )
                    .unwrap_or("UNKNOWN")
            );
            if operation == "collect" {
                job = batch::collect(&job_path, &frozen.plan, &response)?;
                if matches!(job.state, State::Completed | State::Partial | State::Failed) {
                    job = batch::settle(&job_path, &frozen.plan, &ledger)?;
                }
            }
        }
    }
    let mut value = local_cmd::base_result("review.assist.batch");
    let exit = if matches!(
        job.state,
        State::SubmissionUnknown | State::Partial | State::Failed
    ) {
        4
    } else {
        0
    };
    value["execution"] = json!(if exit == 0 { "complete" } else { "incomplete" });
    value["artifact"] = local_cmd::reference(&job_path)?;
    value["data"] = json!({"operation":operation,"state":job.state,"remote_state":remote,"provider_operation":job.operation,"failed_items":job.failed_items,"experimental":true,"egress_preview":preview});
    value["limits"] = json!([
        "Asynchronous experimental advice; job completion does not establish semantic accuracy."
    ]);
    Ok((local_cmd::bounded(value, 4096)?, exit))
}
