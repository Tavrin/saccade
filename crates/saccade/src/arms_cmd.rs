//! Standalone arm checks and shared strict flags.
use crate::agent::CliError;
use std::path::PathBuf;
#[derive(clap::Args, Clone, Default)]
pub(crate) struct StrictArgs {
    /// Permit both unreached arms only under an identical criterion and observation.
    #[arg(long = "allow-unreached")]
    pub allow_unreached: Vec<String>,
    /// Refuse verdicts for incomplete or mismatched producer identity.
    #[arg(long)]
    pub require_valid_arms: bool,
    /// Generic TOML/JSON mapping from producer fields and sibling records.
    #[arg(long)]
    pub fingerprint_map: Option<PathBuf>,
    /// Explicit ignored metadata tokens, echoed in arm validation output.
    #[arg(long = "arm-ignore")]
    pub arm_ignore: Vec<String>,
}
impl StrictArgs {
    pub(crate) fn apply(&self, opts: &mut saccade_core::meta::MetaOptions) {
        opts.require_valid_arms |= self.require_valid_arms;
        if let Some(map) = &self.fingerprint_map {
            opts.fingerprint_map = Some(map.clone());
        }
        opts.ignore.extend(self.arm_ignore.iter().cloned());
        opts.allow_unreached
            .extend(self.allow_unreached.iter().cloned());
    }
}
#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// Check two capture records, sidecars, images or capture directories.
    Check {
        a: PathBuf,
        b: PathBuf,
        /// Allowed difference: exact key, dotted prefix, suffix or explicit glob.
        #[arg(long)]
        vary: Vec<String>,
        /// Permit intentionally unreached captures with exactly matching observations.
        #[arg(long = "allow-unreached")]
        allow_unreached: Vec<String>,
        /// Explicit exception, echoed even when no keys match it.
        #[arg(long)]
        ignore: Vec<String>,
        #[arg(long)]
        fingerprint_map: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        meta_name: Option<String>,
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn emit(check: &saccade_core::arms::Check, json: bool) -> Result<(), CliError> {
    let text = if json {
        serde_json::to_string(check)?
    } else {
        serde_json::to_string_pretty(check)?
    };
    crate::emit(&format!("{text}\n"))
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    match args.operation {
        Operation::Check {
            a,
            b,
            vary,
            allow_unreached,
            ignore,
            fingerprint_map,
            config,
            meta_name,
            json,
        } => {
            let mut cfg = crate::load_config(config.as_deref())?;
            cfg.meta.intended.extend(vary);
            cfg.meta.allow_unreached.extend(allow_unreached);
            cfg.meta.ignore.extend(ignore);
            if let Some(map) = fingerprint_map {
                cfg.meta.fingerprint_map = Some(map);
            }
            if let Some(name) = meta_name {
                cfg.meta.name = name;
            }
            let result = saccade_core::arms::validate_paths(&a, &b, &cfg)?;
            emit(&result, json)?;
            Ok(result.exit_code)
        }
    }
}

pub(crate) fn annotate(value: &mut serde_json::Value, opts: &saccade_core::meta::MetaOptions) {
    if opts.require_valid_arms {
        value["arm_validation"] = serde_json::json!({"result":"valid_comparison","ignore":opts.ignore,"vary":opts.intended});
    }
}
pub(crate) fn success_text(opts: &saccade_core::meta::MetaOptions) -> Result<(), CliError> {
    if opts.require_valid_arms {
        crate::emit(&format!(
            "valid_comparison: ignored {}; intended {}\n",
            serde_json::to_string(&opts.ignore)?,
            serde_json::to_string(&opts.intended)?
        ))?;
    }
    Ok(())
}
