//! Capture-record conformance transport; no acquisition or pixel verdict.
use crate::agent::CliError;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(clap::Subcommand)]
enum Operation {
    /// Check a versioned receipt, all planned slots and exact image hashes.
    Conform {
        record: PathBuf,
        /// Explicit versioned mapping of retained historical fields (TOML or JSON).
        #[arg(long)]
        legacy_map: Option<PathBuf>,
        /// Check an archive tree with bounded deterministic rows; requires --legacy-map.
        #[arg(long, requires = "legacy_map")]
        archive: bool,
        /// Emit the versioned conformance result, including stable failure codes.
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let Operation::Conform {
        record,
        json,
        legacy_map,
        archive,
    } = args.operation;
    let rows = if let Some(map) = legacy_map {
        if archive {
            saccade_core::capture_legacy::archive(&record, &map)
        } else {
            vec![(
                record.clone(),
                saccade_core::capture_legacy::conform_legacy(&record, &map),
            )]
        }
    } else {
        vec![(record.clone(), saccade_core::capture::conform_path(&record))]
    };
    let mut exit = 0;
    for (path, report) in rows {
        // Explicit severity order: invalid, contradictory, unavailable, adapted pass, native pass.
        let code = report.exit_code();
        let rank = |c| match c {
            2 => 4,
            1 => 3,
            4 => 2,
            3 => 1,
            _ => 0,
        };
        if rank(code) > rank(exit) {
            exit = code;
        }
        if json {
            let value = if archive {
                // JSON archive rows use portable separators on every platform.
                let directory = saccade_core::paths::portable(&path);
                serde_json::json!({"directory":directory,"report":report})
            } else {
                serde_json::to_value(&report)?
            };
            crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
        } else {
            crate::emit(&format!(
                "{} {} {}\n",
                path.display(),
                report.provenance,
                if report.conformant {
                    "conformant"
                } else {
                    "nonconformant"
                }
            ))?;
            for finding in &report.findings {
                crate::emit(&format!(
                    "{} {}\n",
                    serde_json::to_value(finding.code)?
                        .as_str()
                        .unwrap_or("invalid_record"),
                    finding.id.as_deref().unwrap_or("record")
                ))?;
            }
            for field in &report.fields {
                crate::emit(&format!("{} {}\n", field.state, field.field))?;
            }
            for missing in &report.unavailable {
                crate::emit(&format!("{} {}\n", missing.code, missing.field))?;
            }
        }
    }
    Ok(exit)
}
