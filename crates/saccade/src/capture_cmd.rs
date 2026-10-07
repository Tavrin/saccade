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
        /// Emit the versioned conformance result, including stable failure codes.
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let Operation::Conform { record, json } = args.operation;
    let report = saccade_core::capture::conform_path(&record);
    if json {
        crate::emit(&format!("{}\n", serde_json::to_string(&report)?))?;
    } else {
        crate::emit(if report.conformant {
            "conformant\n"
        } else {
            "nonconformant\n"
        })?;
        for finding in &report.findings {
            crate::emit(&format!(
                "{} {}\n",
                serde_json::to_value(finding.code)?
                    .as_str()
                    .unwrap_or("invalid_record"),
                finding.id.as_deref().unwrap_or("record")
            ))?;
        }
    }
    Ok(report.exit_code())
}
