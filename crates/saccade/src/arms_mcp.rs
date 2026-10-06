//! Root-contained arm validity mirror on the measurement tool.
use crate::agent::CliError;
use saccade_core::root_policy::RootPolicy;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    operation: String,
    a: PathBuf,
    b: PathBuf,
    fingerprint_map: Option<PathBuf>,
    max_record_bytes: Option<u64>,
    compare: Option<saccade_core::arms::CompareMode>,
    #[serde(default)]
    vary: Vec<String>,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default)]
    allow_unreached: Vec<String>,
    meta_name: Option<String>,
}
pub(crate) fn validate_map(
    policy: &RootPolicy,
    path: &Path,
    inputs: &[&Path],
) -> Result<(), CliError> {
    let map = saccade_core::arms::FingerprintMap::read(&policy.read(path)?)?;
    for input in inputs {
        let root = if input.is_dir() {
            *input
        } else {
            input.parent().unwrap_or(Path::new("."))
        };
        for file in &map.record_files {
            let record = root.join(file);
            if record.exists() {
                policy.read(&record)?;
            }
        }
        for s in map
            .fields
            .values()
            .chain(map.readiness.iter().flat_map(|r| [&r.reached, &r.observed]))
        {
            if let Some(file) = &s.file {
                let sibling = root.join(file);
                if sibling.exists() {
                    policy.read(&sibling)?;
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn call(policy: &RootPolicy, value: Value) -> Result<Value, CliError> {
    let args: Args = serde_json::from_value(value)?;
    if args.operation != "arms_check" {
        return Err(CliError::usage("expected arms_check"));
    }
    let a = policy.read(&args.a)?;
    let b = policy.read(&args.b)?;
    let opts = saccade_core::meta::MetaOptions {
        max_record_bytes: args.max_record_bytes,
        compare: args.compare,
        intended: args.vary,
        allow_unreached: args.allow_unreached,
        ignore: args.ignore,
        fingerprint_map: args.fingerprint_map.map(|p| policy.read(&p)).transpose()?,
        name: args
            .meta_name
            .unwrap_or_else(|| saccade_core::meta::DEFAULT_META_NAME.into()),
        ..Default::default()
    };
    if let Some(map) = &opts.fingerprint_map {
        validate_map(policy, map, &[&a, &b])?;
    }
    let cfg = saccade_core::config::RunConfig {
        meta: opts,
        ..Default::default()
    };
    Ok(serde_json::to_value(saccade_core::arms::validate_paths(
        &a, &b, &cfg,
    )?)?)
}
pub(crate) fn schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["operation","a","b"],"properties":{"operation":{"const":"arms_check"},"a":{"type":"string"},"b":{"type":"string"},"vary":{"type":"array","items":{"type":"string"}},"ignore":{"type":"array","items":{"type":"string"}},"compare":{"enum":["mapped_only","all"]},"fingerprint_map":{"type":"string"},"max_record_bytes":{"type":"integer","minimum":1,"maximum":saccade_core::arms::HARD_MAX_RECORD_BYTES},"meta_name":{"type":"string"},"allow_unreached":{"type":"array","items":{"type":"string"}}}})
}
