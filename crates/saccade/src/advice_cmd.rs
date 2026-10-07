//! Additive task navigation; all authority remains in the existing assist layer.
use crate::{agent::CliError, assist_cmd};
use saccade_core::assist::{
    execution::CacheKey,
    schema::{Task, Usage},
};
use serde_json::{Value, json};
use std::{io::Write, path::Path};
#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    user_config: Option<std::path::PathBuf>,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// Prepare timestamped advisory video requests offline.
    VideoJudge(crate::video_judge_cmd::Args),
    /// Explain visible changes; cannot alter the measured verdict.
    Explain(assist_cmd::ReportArgs),
    /// Audit declared masks; cannot create or apply exclusions.
    AuditMask(assist_cmd::ReportArgs),
    /// Check a bounded visible condition; cannot approve a baseline.
    CheckUi(assist_cmd::CheckArgs),
    /// Existing experimental frozen evaluation lifecycle.
    Batch(crate::assist_batch_cmd::BatchArgs),
}
pub(crate) fn run(a: Args) -> Result<u8, CliError> {
    match a.operation {
        Operation::VideoJudge(args) => crate::video_judge_cmd::run(args, a.json),
        Operation::Explain(args) => {
            assist_cmd::run_report(args, Task::Explain, a.json, a.user_config.as_deref())
        }
        Operation::AuditMask(args) => {
            assist_cmd::run_report(args, Task::AuditMask, a.json, a.user_config.as_deref())
        }
        Operation::CheckUi(args) => assist_cmd::run_check(args, a.json, a.user_config.as_deref()),
        Operation::Batch(args) => crate::assist_batch_cmd::run(
            crate::assist_batch_cmd::AssistArgs {
                operation: crate::assist_batch_cmd::AssistOperation::Batch(args),
            },
            a.json,
            a.user_config.as_deref(),
        ),
    }
}
pub(crate) const PREVIEW_SCHEMA: &str = "saccade-egress-preview.v1";
/// Exact payload sidecar plus a conservative, credential-free preview before dispatch.
pub(crate) fn preview(
    key: &CacheKey,
    payload: &[u8],
    out: &Path,
    dispatch: bool,
) -> Result<Value, CliError> {
    let input = payload.len().div_ceil(4) as u64;
    let bounds = if key.provider == "gemini" {
        saccade_core::assist::price::gemini_bounds(payload)
            .ok()
            .map(|b| (b.input, b.output))
    } else if key.provider == "jev" {
        Some((saccade_core::assist::execution::INPUT_LIMIT, 0))
    } else {
        None
    };
    let at = saccade_core::budget_ledger::now_ms();
    let cost = bounds
        .and_then(|(input, output)| {
            saccade_core::assist::execution::cost_nano(
                &key.provider,
                &Usage {
                    input_tokens: Some(input),
                    candidate_tokens: Some(output),
                    thinking_tokens: Some(0),
                    total_tokens: Some(input + output),
                    ..Default::default()
                },
                at,
                false,
            )
        })
        .map(|n| n as f64 / 1e9);
    let value = json!({"schema":PREVIEW_SCHEMA,"provider":key.provider,"model":key.model,"required_revision":key.revision,"request_bytes":payload.len(),"request_sha256":key.payload_hash,"estimated_text_input_tokens":input,"estimated_cost_usd":cost,"cost_basis":"conservative local ceiling; unknown when pricing expired or unsupported","dispatch_requested":dispatch,"authority":{"approve_baselines":false,"create_exclusions":false,"qualify_timing":false},"payload_file":format!("egress-{}.json",key.payload_hash.as_str().replace(':',"-"))});
    std::fs::create_dir_all(out).map_err(|e| CliError::io(e.to_string()))?;
    let path = out.join(
        value["payload_file"]
            .as_str()
            .ok_or_else(|| CliError::io("preview payload path"))?,
    );
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::io("preview artifact is a symlink"));
    }
    let mut exact =
        tempfile::NamedTempFile::new_in(out).map_err(|e| CliError::io(e.to_string()))?;
    exact
        .write_all(payload)
        .map_err(|e| CliError::io(e.to_string()))?;
    exact
        .as_file()
        .sync_all()
        .map_err(|e| CliError::io(e.to_string()))?;
    exact
        .persist(&path)
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("egress-preview.jsonl"))
        .map_err(|e| CliError::io(e.to_string()))?;
    writeln!(file, "{}", serde_json::to_string(&value)?)
        .map_err(|e| CliError::io(e.to_string()))?;
    file.sync_all().map_err(|e| CliError::io(e.to_string()))?;
    if dispatch {
        eprintln!("{}", serde_json::to_string(&value)?);
    }
    Ok(value)
}
