//! CLI-only warning for outputs placed next to capture metadata.

use std::path::PathBuf;

use saccade_core::meta::DEFAULT_META_NAME;

use crate::Command;
use crate::agent::CliError;
use crate::judge_cmd::JudgeSub;

pub(crate) fn warn(command: &Command, allow: bool) -> Result<(), CliError> {
    if allow {
        return Ok(());
    }
    let out = match command {
        Command::Compare { out, .. }
        | Command::Identity { out, .. }
        | Command::View { out, .. }
        | Command::Sequence { out, .. }
        | Command::Rank { out, .. } => Some(out.clone()),
        Command::Runs { out, json, .. } => out
            .clone()
            .or_else(|| (!json).then(|| PathBuf::from(crate::runs_cmd::DEFAULT_OUT))),
        Command::Explain {
            out, report_json, ..
        } => Some(out.clone().unwrap_or_else(|| {
            report_json
                .parent()
                .map_or_else(|| PathBuf::from("explain"), |p| p.join("explain"))
        })),
        Command::Unblind { out, .. } => out.clone(),
        Command::Snapshot(args) => Some(args.out.clone()),
        Command::Ablate(args) => Some(args.out.clone()),
        Command::Bisect(args) => Some(args.out.clone()),
        Command::Watch(args) => Some(args.out.clone()),
        Command::Judge(args) => match &args.sub {
            Some(JudgeSub::Calibrate(args)) => Some(args.out.clone()),
            Some(JudgeSub::Selftest(args)) => args.out.clone(),
            None => args.run.out.clone().or_else(|| {
                args.run.target.as_ref().map(|target| {
                    target
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(saccade_core::judge::JUDGE_FILE_NAME)
                })
            }),
        },
        // MCP has its own root confinement; commands without --out do not warn.
        Command::Init(_)
        | Command::Config(_)
        | Command::Entries(_)
        | Command::Noise(_)
        | Command::Demo(_)
        | Command::Mcp { .. }
        | Command::Approve { .. }
        | Command::Serve { .. }
        | Command::Summary { .. }
        | Command::DecisionRequest(_)
        | Command::Decide(_)
        | Command::Ask(_) => None,
    };
    let Some(out) = out else {
        return Ok(());
    };
    let configured = match command {
        Command::Compare { config, meta, .. }
        | Command::Identity { config, meta, .. }
        | Command::View { config, meta, .. }
        | Command::Sequence { config, meta, .. }
        | Command::Rank { config, meta, .. }
        | Command::Runs { config, meta, .. } => {
            let mut options = crate::load_config(config.as_deref())?.meta;
            meta.apply(&mut options);
            Some(options.name)
        }
        Command::Watch(args) => Some(crate::load_config(args.config.as_deref())?.meta.name),
        Command::Judge(args) if args.sub.is_none() => {
            Some(crate::load_config(args.run.config.as_deref())?.meta.name)
        }
        _ => None,
    };
    let meta_name = configured.as_deref().unwrap_or(DEFAULT_META_NAME);
    let out = saccade_core::explain::absolute(&out);
    if let Some(parent) = out.parent() {
        for name in [meta_name, "capture.json"] {
            if parent.join(name).is_file() {
                eprintln!(
                    "saccade: warning: --out is inside a capture directory (found {} next to it); \
                     outputs there may be indexed as captures. Use --allow-out-near-captures to silence.",
                    crate::escape_control(name)
                );
                break;
            }
        }
    }
    Ok(())
}
