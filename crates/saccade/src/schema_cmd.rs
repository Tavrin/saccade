//! Embedded schema discovery and performance-sidecar validation.
use crate::agent::CliError;
use saccade_core::schema_catalog;
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// List all schema IDs shipped in this binary.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Print the exact embedded JSON Schema (or create a new file).
    Get {
        id: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Locate an installed copy, if available; use get for portable discovery.
    Path {
        id: String,
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn installed(id: &str) -> Option<PathBuf> {
    // Search install locations, never the compile-time source checkout.
    let exe = std::env::current_exe().ok()?;
    let prefix = exe.parent()?.parent()?;
    let mut roots = vec![prefix.join("share/saccade/schemas")];
    if let Some(root) = std::env::var_os("SACCADE_SCHEMA_DIR") {
        roots.insert(0, root.into());
    }
    let embedded = schema_text(id).ok()?;
    roots
        .into_iter()
        .map(|r| r.join(format!("{id}.schema.json")))
        .find(|p| {
            saccade_core::evidence_quality::read(p, 4 << 20).is_ok_and(|b| b == embedded.as_bytes())
        })
}
fn schema_text(id: &str) -> Result<&'static str, CliError> {
    #[cfg(feature = "print")]
    if id == saccade_print::SCHEMA {
        return Ok(saccade_print::JSON_SCHEMA);
    }
    Ok(schema_catalog::get(id)?)
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    match args.operation {
        Operation::List { json } => {
            #[allow(unused_mut)]
            let mut ids: Vec<_> = schema_catalog::DOCUMENTS
                .iter()
                .map(|(id, _)| *id)
                .collect();
            #[cfg(feature = "print")]
            ids.push(saccade_print::SCHEMA);
            if json {
                crate::emit(&format!(
                    "{}\n",
                    serde_json::json!({"schema":"saccade-schema-list.v1","ids":ids})
                ))?;
            } else {
                crate::emit(&format!("{}\n", ids.join("\n")))?;
            }
        }
        Operation::Get {
            id,
            out,
            json: _json,
        } => {
            let text = schema_text(&id)?;
            if let Some(out) = out {
                write_new(&out, text.as_bytes())?;
            } else {
                crate::emit(text)?;
            }
        }
        Operation::Path { id, json } => {
            schema_text(&id)?;
            let path = installed(&id);
            if json {
                crate::emit(&format!(
                    "{}\n",
                    serde_json::json!({"schema":"saccade-schema-path.v1","id":id,"path":path})
                ))?;
            } else if let Some(p) = &path {
                crate::emit(&format!("{}\n", p.display()))?;
            }
            return Ok(u8::from(path.is_none()));
        }
    }
    Ok(0)
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| CliError::io(e.to_string()))?;
    file.write_all(bytes)
        .map_err(|e| CliError::io(e.to_string()))
}
#[derive(clap::Args)]
pub(crate) struct PerfArgs {
    #[command(subcommand)]
    operation: PerfOperation,
}
#[derive(clap::Subcommand)]
enum PerfOperation {
    /// Validate a perf v1 or v2 sidecar against the exact embedded JSON Schema.
    Validate {
        sidecar: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn validate(value: &serde_json::Value) -> Result<Vec<String>, CliError> {
    let id = value["schema"]
        .as_str()
        .ok_or_else(|| CliError::usage("perf sidecar requires schema"))?;
    if !matches!(id, "saccade-perf.v1" | "saccade-perf.v2") {
        return Err(CliError::usage(
            "expected saccade-perf.v1 or saccade-perf.v2",
        ));
    }
    let schema: serde_json::Value = serde_json::from_str(schema_catalog::get(id)?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|e| CliError::new("invalid_schema", e.to_string()))?;
    Ok(validator
        .iter_errors(value)
        .take(32)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect())
}
pub(crate) fn perf(args: PerfArgs) -> Result<u8, CliError> {
    let PerfOperation::Validate { sidecar, json } = args.operation;
    let value = serde_json::from_slice(&saccade_core::evidence_quality::read(&sidecar, 4 << 20)?)?;
    let errors = validate(&value)?;
    let report = serde_json::json!({"schema":"saccade-perf-validation.v1","valid":errors.is_empty(),"sidecar_schema":value["schema"],"errors":errors});
    if json {
        crate::emit(&format!("{report}\n"))?;
    } else {
        crate::emit(&format!(
            "perf validation: {}\n{}",
            if errors.is_empty() {
                "valid"
            } else {
                "invalid"
            },
            errors.join("\n")
        ))?;
    }
    Ok(u8::from(!errors.is_empty()))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn perf_validation_matches_file_schema_for_valid_and_invalid_sidecars() {
        for id in ["saccade-perf.v1", "saccade-perf.v2"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../saccade-core/schemas/{id}.schema.json"));
            let schema: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            let file_validator = jsonschema::validator_for(&schema).unwrap();
            let mut valid = serde_json::json!({"schema":id,"unit":"ms","frame":{"value":1.0,"samples":8,"stat":"mean"},"terms":[],"counters":{}});
            if id.ends_with("v2") {
                valid["kind"] = serde_json::json!("measurement");
            }
            assert!(
                file_validator.is_valid(&valid),
                "valid producer fixture: {id}"
            );
            assert!(super::validate(&valid).unwrap().is_empty());
            let values = [
                valid,
                serde_json::json!({"schema":id}),
                serde_json::json!({"schema":id,"frames":[{"gpu_ms":-1}]}),
                serde_json::json!({"schema":id,"frames":[]}),
            ];
            for value in values {
                assert_eq!(
                    super::validate(&value).unwrap().is_empty(),
                    file_validator.is_valid(&value)
                );
            }
        }
        let schema = saccade_core::schema_catalog::get("saccade-perf.v2").unwrap();
        assert!(schema.contains("performance_noise"));
        assert!(
            !super::validate(&serde_json::json!({"schema":"saccade-perf.v2","kind":"wrong"}))
                .unwrap()
                .is_empty()
        );
    }
}
