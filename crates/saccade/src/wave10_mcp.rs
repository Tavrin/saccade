//! Root-contained producer schema and repeat-noise mirrors.
use crate::agent::CliError;
use saccade_core::root_policy::RootPolicy;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Args {
    #[serde(rename = "schema_list")]
    List,
    #[serde(rename = "schema_get")]
    Get { id: String, out: Option<PathBuf> },
    #[serde(rename = "schema_path")]
    Path { id: String },
    #[serde(rename = "perf_validate")]
    Validate { sidecar: PathBuf },
    #[serde(rename = "noise_build")]
    Noise {
        repeats: Vec<PathBuf>,
        out: PathBuf,
        #[serde(default = "tile_size")]
        tile_size: u32,
        #[serde(default)]
        require_valid_arms: bool,
        fingerprint_map: Option<PathBuf>,
        #[serde(default)]
        allow_unreached: Vec<String>,
        #[serde(default)]
        arm_ignore: Vec<String>,
    },
}
fn tile_size() -> u32 {
    32
}
pub(crate) fn handles(name: &str, op: &str) -> bool {
    name == "saccade_measure"
        && matches!(
            op,
            "schema_list" | "schema_get" | "schema_path" | "perf_validate" | "noise_build"
        )
}
pub(crate) fn call(
    policy: &RootPolicy,
    args: &serde_json::Map<String, Value>,
) -> Result<Value, CliError> {
    match serde_json::from_value(Value::Object(args.clone()))? {
        Args::List => Ok(
            json!({"schema":"saccade-schema-list.v1","ids":saccade_core::schema_catalog::DOCUMENTS.iter().map(|(id,_)|*id).collect::<Vec<_>>()}),
        ),
        Args::Get { id, out } => {
            let schema = saccade_core::schema_catalog::get(&id)?;
            let value = serde_json::from_str(schema)?;
            if let Some(out) = out {
                crate::schema_cmd::write_new(&policy.write(&out)?, schema.as_bytes())?;
            }
            Ok(value)
        }
        Args::Path { id } => {
            saccade_core::schema_catalog::get(&id)?;
            Ok(
                json!({"schema":"saccade-schema-path.v1","id":id,"path":crate::schema_cmd::installed(&id)}),
            )
        }
        Args::Validate { sidecar } => {
            let value: Value = serde_json::from_slice(&saccade_core::evidence_quality::read(
                &policy.read(&sidecar)?,
                4 << 20,
            )?)?;
            let errors = crate::schema_cmd::validate(&value)?;
            Ok(
                json!({"schema":"saccade-perf-validation.v1","valid":errors.is_empty(),"sidecar_schema":value["schema"],"errors":errors}),
            )
        }
        Args::Noise {
            repeats,
            out,
            tile_size,
            require_valid_arms,
            fingerprint_map,
            allow_unreached,
            arm_ignore,
        } => {
            let repeats = repeats
                .iter()
                .map(|p| policy.read(p))
                .collect::<Result<Vec<_>, _>>()?;
            if repeats.len() < 2 {
                return Err(CliError::usage("noise needs two repeats"));
            }
            let mut cfg = saccade_core::config::RunConfig::default();
            cfg.meta.require_valid_arms = require_valid_arms;
            cfg.meta.allow_unreached = allow_unreached;
            cfg.meta.ignore = arm_ignore;
            cfg.meta.fingerprint_map = fingerprint_map.map(|p| policy.read(&p)).transpose()?;
            if let Some(map) = &cfg.meta.fingerprint_map {
                crate::arms_mcp::validate_map(
                    policy,
                    map,
                    &repeats.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
                )?;
            }
            for repeat in repeats.iter().skip(1) {
                saccade_core::arms::enforce(&repeats[0], repeat, &cfg)?;
            }
            let value = serde_json::to_value(
                saccade_core::evidence_quality::repeat_noise::build_runs(&repeats, tile_size)?,
            )?;
            crate::general_cmd::write_new(&policy.write(&out)?, &value)?;
            Ok(value)
        }
    }
}
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","additionalProperties":false,"required":["operation"],"properties":{"operation":{"const":"schema_list"}}}),
        json!({"type":"object","additionalProperties":false,"required":["operation","id"],"properties":{"operation":{"const":"schema_get"},"id":{"type":"string"},"out":{"type":"string"}}}),
        json!({"type":"object","additionalProperties":false,"required":["operation","id"],"properties":{"operation":{"const":"schema_path"},"id":{"type":"string"}}}),
        json!({"type":"object","additionalProperties":false,"required":["operation","sidecar"],"properties":{"operation":{"const":"perf_validate"},"sidecar":{"type":"string"}}}),
        json!({"type":"object","additionalProperties":false,"required":["operation","repeats","out"],"properties":{"operation":{"const":"noise_build"},"repeats":{"type":"array","minItems":2,"maxItems":32,"items":{"type":"string"}},"out":{"type":"string"},"tile_size":{"type":"integer","minimum":4,"maximum":4096},"require_valid_arms":{"type":"boolean"},"fingerprint_map":{"type":"string"},"allow_unreached":{"type":"array","items":{"type":"string"}},"arm_ignore":{"type":"array","items":{"type":"string"}}}}),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn producer_mirrors_preserve_schema_bytes_and_refuse_outside_roots() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outputs = tempfile::tempdir().unwrap();
        let policy =
            RootPolicy::new(&[root.path().into()], Some(outputs.path()), false, &[]).unwrap();
        let out = outputs.path().join("schema.json");
        let args = json!({"operation":"schema_get","id":"saccade-perf.v2","out":out});
        call(&policy, args.as_object().unwrap()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            saccade_core::schema_catalog::get("saccade-perf.v2").unwrap()
        );
        let refused = json!({"operation":"schema_get","id":"saccade-perf.v2","out":outside.path().join("schema.json")});
        assert!(call(&policy, refused.as_object().unwrap()).is_err());
        let path = root.path().join("perf.json");
        std::fs::write(&path, br#"{"schema":"saccade-perf.v2","kind":"wrong"}"#).unwrap();
        let validation = json!({"operation":"perf_validate","sidecar":path});
        assert_eq!(
            call(&policy, validation.as_object().unwrap()).unwrap()["valid"],
            false
        );
    }
}
