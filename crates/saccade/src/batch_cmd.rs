//! Lane D local batch transport. Analysis remains in existing commands.
use crate::agent::CliError;
use saccade_core::batch::{self, Options, Section};
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct Args {
    /// Folder or saccade-batch-input.v1 JSON manifest.
    source: PathBuf,
    /// Dedicated output directory; rerunning resumes immutable receipts.
    #[arg(long)]
    out: PathBuf,
    /// Existing commands to apply (repeatable); default analyze-media.
    #[arg(long,value_parser=["analyze-media","inspect","compare","text-quality","tofu","watermark","mask-metrics"],conflicts_with="options")]
    section: Vec<String>,
    /// Options JSON: sections with command/args, concurrency and timeout_ms.
    #[arg(long)]
    options: Option<PathBuf>,
    /// Match relative paths in this reference folder for pair commands.
    #[arg(long)]
    reference_dir: Option<PathBuf>,
    #[arg(long, default_value_t=2,value_parser=clap::value_parser!(u64).range(1..=16),conflicts_with="options")]
    concurrency: u64,
    /// Whole-item timeout, including input decoding, in milliseconds.
    #[arg(long, default_value_t=30000,value_parser=clap::value_parser!(u64).range(1..=300000),conflicts_with="options")]
    timeout_ms: u64,
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
pub(crate) struct ProbeArgs {
    path: PathBuf,
    thumbnail: PathBuf,
    #[arg(long)]
    json: bool,
}
pub(crate) fn probe(a: ProbeArgs) -> Result<u8, CliError> {
    crate::media_cmd::emit(&batch::probe(&a.path, &a.thumbnail)?, a.json)
}
pub(crate) fn run(a: Args) -> Result<u8, CliError> {
    if crate::signed_approval::required() {
        return Err(CliError::new(
            "approval_consumer_unsupported",
            "signed policy refuses subprocess measurement workflows without policy propagation",
        ));
    }

    let options = if let Some(p) = a.options {
        serde_json::from_slice(&saccade_core::evidence_quality::read(&p, 65536)?)?
    } else {
        Options {
            sections: if a.section.is_empty() {
                Options::default().sections
            } else {
                a.section
                    .into_iter()
                    .map(|command| Section {
                        command,
                        args: vec![],
                    })
                    .collect()
            },
            concurrency: a.concurrency as usize,
            timeout_ms: a.timeout_ms,
        }
    };

    let executable = std::env::current_exe().map_err(|e| CliError::io(e.to_string()))?;
    let result =
        saccade_core::workflows::batch::run_batch(&saccade_core::workflows::batch::BatchRun {
            source: &a.source,
            reference_dir: a.reference_dir.as_deref(),
            out: &a.out,
            options: &options,
            executable: &executable,
            policy: crate::signed_approval::policy(),
        })?;
    crate::media_cmd::emit(&result.value, a.json)?;
    Ok(if result.failed { 4 } else { 0 })
}
