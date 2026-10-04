//! CLI-only warning for outputs placed next to capture metadata.

use saccade_core::meta::DEFAULT_META_NAME;

use crate::agent::CliError;
use crate::{Command, ProveOperation};

pub(crate) fn warn(command: &Command, allow: bool) -> Result<(), CliError> {
    if allow {
        return Ok(());
    }
    let out = match command {
        Command::Compare { out, .. }
        | Command::Identity { out, .. }
        | Command::View { out, .. } => Some(out),
        Command::Prove {
            operation: ProveOperation::Identity(args),
        } => Some(&args.out),
        #[cfg(feature = "graphics")]
        Command::Prove {
            operation: ProveOperation::Performance(args),
        } => Some(&args.out),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                crate::ExperimentOperation::Sequence { out, .. }
                | crate::ExperimentOperation::Rank { out, .. },
        } => Some(out),
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation: crate::ExperimentOperation::Ablate(args),
        } => Some(&args.out),
        _ => None,
    };
    let Some(out) = out else {
        return Ok(());
    };
    let configured = match command {
        Command::Compare { config, meta, .. }
        | Command::Identity { config, meta, .. }
        | Command::View { config, meta, .. } => {
            let mut options = crate::load_config(config.as_deref())?.meta;
            meta.apply(&mut options);
            Some(options.name)
        }
        Command::Prove {
            operation: ProveOperation::Identity(args),
        } => {
            let mut options = crate::load_config(args.config.as_deref())?.meta;
            args.meta.apply(&mut options);
            Some(options.name)
        }
        #[cfg(feature = "graphics")]
        Command::Experiment {
            operation:
                crate::ExperimentOperation::Sequence { config, meta, .. }
                | crate::ExperimentOperation::Rank { config, meta, .. },
        } => {
            let mut options = crate::load_config(config.as_deref())?.meta;
            meta.apply(&mut options);
            Some(options.name)
        }
        _ => None,
    };
    let meta_name = configured.as_deref().unwrap_or(DEFAULT_META_NAME);
    let out = saccade_core::explain::absolute(out);
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
