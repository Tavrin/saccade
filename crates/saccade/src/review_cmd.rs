//! Authorized canonical review shared by CLI and the final MCP tool.
use crate::{agent::CliError, local_cmd};
use saccade_core::budget_ledger::{Caps, Ledger, Scope};
use saccade_core::evidence::canonical;
use saccade_core::judge_provider::{
    Keys,
    transport::{self, Authorization, UserConfig},
};
use serde_json::Value;
use std::path::{Path, PathBuf};
/// Human startup configuration, kept outside tool input schemas.
#[derive(clap::Args, Clone, Default)]
pub(crate) struct Startup {
    /// Explicitly authorize provider calls for this MCP server lifetime.
    #[arg(long)]
    pub allow_provider_calls: bool,
    /// Finite startup attempt cap; no implicit MCP allowance.
    #[arg(long, value_parser=clap::value_parser!(u64).range(1..))]
    pub budget_calls: Option<u64>,
    /// Human-owned endpoints, credential bindings and root egress policy.
    #[arg(long)]
    pub user_config: Option<PathBuf>,
}
pub(crate) fn user_file(selected: Option<&Path>) -> PathBuf {
    selected.map_or_else(|| Keys::default_dir().join("user.toml"), Path::to_owned)
}
pub(crate) fn load_user(file: &Path) -> Result<UserConfig, CliError> {
    Ok(saccade_core::workflows::review::load_user(file)?)
}
pub(crate) fn authorization(
    enabled: bool,
    budget: u64,
    run: &str,
    user: &UserConfig,
) -> Authorization {
    saccade_core::workflows::review::authorization(enabled, budget, run, user)
}

// Case construction may read an adjacent evidence document and hash sidecars.
// Check those routes before construction, not merely before provider dispatch.
pub(crate) fn guard_case_inputs(
    file: &Path,
    roots: &saccade_core::root_policy::RootPolicy,
) -> Result<(), CliError> {
    Ok(saccade_core::workflows::review::guard_case_inputs(
        file, roots,
    )?)
}
// Presentation references never replace the full provenance used by egress
// checks, provider dispatch, or the saved review plan.

/// Prepare exact payload hashes and decisions, without creating credentials or dispatching.
#[allow(clippy::too_many_arguments)]
pub(crate) fn review(
    file: &Path,
    out: Option<&Path>,
    run: bool,
    budget: u64,
    config: &Path,
    roots: &saccade_core::root_policy::RootPolicy,
    auth: &Authorization,
    intent: Option<saccade_core::evidence::case::Intent>,
) -> Result<Value, CliError> {
    Ok(saccade_core::workflows::review::review(
        file, out, run, budget, config, roots, auth, intent,
    )?)
}

pub(crate) fn cli(args: &local_cmd::ReviewArgs) -> Result<Value, CliError> {
    let file = args
        .report
        .as_deref()
        .ok_or_else(|| CliError::usage("review requires REPORT"))?;
    let config = user_file(args.user_config.as_deref());
    let user = load_user(&config)?;
    if user.roots.is_empty() {
        return Err(CliError::new(
            "egress_denied",
            "review execution requires user-owned source roots and export permission",
        ));
    }
    let mut roots = saccade_core::root_policy::RootPolicy::new(
        &user
            .roots
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        user.out_root.as_deref(),
        false,
        &[],
    )?;
    user.apply(&mut roots)
        .map_err(|e| CliError::new("invalid_user_policy", e))?;
    let budget = args.budget_calls.unwrap_or(24);
    let auth = authorization(
        args.run,
        budget,
        &saccade_core::budget_ledger::new_id(),
        &user,
    );
    let input = roots.read(file)?;
    let out = if let Some(out) = args.out.as_deref() {
        Some(roots.write(out)?)
    } else if args.run {
        Some(roots.write(Path::new(&format!(
            "review-{}.json",
            saccade_core::budget_ledger::new_id()
        )))?)
    } else {
        None
    };
    let intent_file = args
        .intent_file
        .as_deref()
        .map(|p| roots.read(p))
        .transpose()?;
    let _scope = saccade_core::root_policy::io::scope(&roots);
    let intent = read_intent(args.intent.as_deref(), intent_file.as_deref(), &input)?;
    review(
        &input,
        out.as_deref(),
        args.run,
        budget,
        &config,
        &roots,
        &auth,
        intent,
    )
}
#[cfg(feature = "evaluation")]
pub(crate) fn evaluate(
    manifest: &Path,
    run: bool,
    user_config: Option<&Path>,
) -> Result<Value, CliError> {
    let config = user_file(user_config);
    let user = load_user(&config)?;
    let mut roots = saccade_core::root_policy::RootPolicy::new(
        &user
            .roots
            .iter()
            .map(|r| r.path.clone())
            .collect::<Vec<_>>(),
        user.out_root.as_deref(),
        false,
        &[],
    )?;
    user.apply(&mut roots)
        .map_err(|e| CliError::new("invalid_user_policy", e))?;
    let manifest = roots.read(manifest)?;
    let job = saccade_core::judge_bench::evaluator::read(&manifest)?;
    // Every nested immutable reference receives the same containment check.
    for task in &job.tasks {
        for reference in [&task.case, &task.request]
            .into_iter()
            .chain(task.labels.iter())
            .chain(task.human_decision.iter())
        {
            roots.read(&saccade_core::paths::resolve(&reference.path, &manifest))?;
        }
    }
    let id = canonical::digest(&job)?;
    let mut auth = authorization(run, job.budget_calls, id.as_str(), &user);
    // Evaluation budgets/providers are separate from production caps.
    auth.scopes[0] = Scope {
        id: format!("evaluation/{}", id.as_str()),
        caps: Caps {
            total: job.budget_calls,
            providers: job
                .tasks
                .iter()
                .map(|t| (t.provider.clone(), job.budget_calls))
                .collect(),
        },
    };
    let ledger = Ledger::new(
        &config.parent().unwrap_or(Path::new(".")).join("attempts"),
        true,
    );
    let keys = Keys::new(Some(config.parent().unwrap_or(Path::new(".")).into()));
    let transport = transport::Transport {
        user: &user,
        roots: &roots,
        authorization: &auth,
        ledger: &ledger,
        keys: &keys,
        http: &transport::Network,
    };
    let mut result = local_cmd::base_result("review.eval");
    if run {
        let state = roots.write(Path::new(&format!(
            "eval-{}.json",
            id.as_str().trim_start_matches("sha256:")
        )))?;
        result["data"] =
            saccade_core::judge_bench::evaluator::run(&job, &manifest, &state, &transport)?;
    } else {
        result["data"] = saccade_core::judge_bench::evaluator::plan(&job, &manifest, &transport)?;
    }
    local_cmd::bounded(result, 4096)
}
/// Offline default preview also works before the user configures export permissions.
pub(crate) fn preview_policy(
    user: &UserConfig,
) -> Result<Option<saccade_core::root_policy::RootPolicy>, CliError> {
    Ok(saccade_core::workflows::review::preview_policy(user)?)
}

/// Prepare a local preview under any configured path policy; never dispatch.
pub(crate) fn preview_local(
    file: &Path,
    budget: u64,
    intent: Option<saccade_core::evidence::case::Intent>,
    out: Option<&Path>,
    absolute: bool,
    user_config: Option<&Path>,
) -> Result<Value, CliError> {
    Ok(saccade_core::workflows::review::preview_local(
        file,
        budget,
        intent,
        out,
        absolute,
        &user_file(user_config),
    )?)
}

pub(crate) fn rebase(value: &mut Value, source: &Path, destination: &Path) -> Result<(), CliError> {
    Ok(saccade_core::workflows::review::rebase(
        value,
        source,
        destination,
    )?)
}

/// Generate a fixed catalog request locally; request creation never enables transport.
pub(crate) fn prepare_request(
    file: &Path,
    question: &str,
) -> Result<saccade_core::evidence::request::DecisionRequest, CliError> {
    Ok(saccade_core::workflows::review::prepare_request(
        file, question,
    )?)
}

pub(crate) fn read_intent(
    text: Option<&str>,
    file: Option<&Path>,
    document: &Path,
) -> Result<Option<saccade_core::evidence::case::Intent>, CliError> {
    Ok(saccade_core::workflows::review::read_intent(
        text, file, document,
    )?)
}
