//! Screenshot adapter presentation.
use crate::agent::CliError;
pub(crate) use saccade_core::workflows::ingest::Format;
use std::path::Path;
pub(crate) fn run(
    format: Format,
    out: &Path,
    json_output: bool,
    absolute: bool,
) -> Result<u8, CliError> {
    let result = saccade_core::workflows::ingest::run_engine(
        format,
        out,
        crate::signed_approval::policy(),
        absolute,
    )?;
    for warning in &result.warnings {
        eprintln!("{warning}");
    }
    crate::emit_run(&result.report, &result.report_dir, json_output, absolute)?;
    Ok(u8::from(result.report.is_regression()))
}
