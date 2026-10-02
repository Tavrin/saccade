//! CLI-only warning for outputs placed next to capture metadata.

use std::path::PathBuf;

use saccade_core::meta::DEFAULT_META_NAME;

use crate::Command;
use crate::agent::CliError;
#[cfg(all(feature = "ai", feature = "evaluation"))]
use crate::judge_cmd::JudgeSub;

pub(crate) fn warn(command: &Command, allow: bool) -> Result<(), CliError> {
    if allow {
        return Ok(());
    }
    let out = match command {
        Command::Compare { out, .. }
        | Command::Identity { out, .. }
        | Command::View { out, .. } => Some(out.clone()),
        #[cfg(feature = "graphics")]
        Command::Sequence { out, .. } | Command::Rank { out, .. } => Some(out.clone()),
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
        #[cfg(feature = "graphics")]
        Command::Ablate(args) => Some(args.out.clone()),
        #[cfg(feature = "prechecks")]
        Command::Safety(args) => Some(args.out.clone()),
        #[cfg(feature = "prechecks")]
        Command::A11y(args) => Some(args.out.clone()),
        #[cfg(feature = "graphics")]
        Command::Bisect(args) => Some(args.out.clone()),
        #[cfg(feature = "ai")]
        Command::Judge(args) => match &args.sub {
            #[cfg(feature = "evaluation")]
            Some(JudgeSub::Bench(args)) => Some(args.out.clone()),
            #[cfg(feature = "evaluation")]
            Some(JudgeSub::CollectLabels(args)) => Some(args.out.clone()),
            #[cfg(feature = "evaluation")]
            Some(JudgeSub::Calibrate(args)) => Some(args.out.clone()),
            #[cfg(feature = "evaluation")]
            Some(JudgeSub::Selftest(args)) => args.out.clone(),
            #[cfg(not(feature = "evaluation"))]
            Some(_) => None,
            None => args.run.out.clone().or_else(|| {
                args.run.target.as_ref().map(|target| {
                    target
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(saccade_core::judge::JUDGE_FILE_NAME)
                })
            }),
        },
        _ => None,
    };
    let Some(out) = out else {
        return Ok(());
    };
    let configured = match command {
        Command::Compare { config, meta, .. }
        | Command::Identity { config, meta, .. }
        | Command::View { config, meta, .. }
        | Command::Runs { config, meta, .. } => {
            let mut options = crate::load_config(config.as_deref())?.meta;
            meta.apply(&mut options);
            Some(options.name)
        }
        #[cfg(feature = "graphics")]
        Command::Sequence { config, meta, .. } | Command::Rank { config, meta, .. } => {
            let mut options = crate::load_config(config.as_deref())?.meta;
            meta.apply(&mut options);
            Some(options.name)
        }
        #[cfg(feature = "ai")]
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
