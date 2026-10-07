//! Baseline approval presentation adapter.
use crate::agent::CliError;
use saccade_core::paths;
use std::path::{Path, PathBuf};
pub(crate) struct Options {
    pub signing: crate::signed_approval::SigningArgs,
    pub names: Vec<String>,
    pub all_failing: bool,
    pub include_errors: bool,
    pub prune_missing: bool,
    pub dry_run: bool,
    pub out: Option<PathBuf>,
    pub json: bool,
    pub absolute: bool,
}

pub(crate) fn run(
    report_path: &Path,
    capture: &Path,
    baseline: &Path,
    decision_path: Option<&Path>,
    opts: Options,
) -> Result<u8, CliError> {
    let json = opts.json;
    let absolute = opts.absolute;
    let result = saccade_core::workflows::approval::run_with_plan_observer(
        report_path,
        capture,
        baseline,
        decision_path,
        saccade_core::workflows::approval::Options {
            policy: crate::signed_approval::policy().clone(),
            signing: opts.signing.options(),
            names: opts.names,
            all_failing: opts.all_failing,
            include_errors: opts.include_errors,
            prune_missing: opts.prune_missing,
            dry_run: opts.dry_run,
            out: opts.out,
            absolute: opts.absolute,
        },
        |plan| {
            if json {
                return Ok(());
            }
            let dry_run = plan.dry_run;
            let out = plan.out;
            let applied = plan.entries;
            let print = || -> Result<(), CliError> {
                crate::emit(&format!(
                    "{} manifest: {}\n",
                    if dry_run { "dry-run" } else { "reviewed" },
                    crate::escape_control(&paths::cwd(&out.join("manifest.json"), absolute))
                ))?;
                for a in applied {
                    crate::emit(&format!(
                        "  {} {}\n",
                        if a.after.is_some() { "copy" } else { "remove" },
                        crate::escape_control(&a.entry_id)
                    ))?;
                }

                Ok(())
            };
            print().map_err(|e| saccade_core::workflows::CommandError {
                code: e.code,
                message: e.message,
                hint: e.hint,
                arm_check: e.arm_check,
            })
        },
    )?;
    let out = result.out;
    let dry_run = result.dry_run;
    if json {
        crate::local_cmd::print(&result.value, true)?;
    } else {
        crate::emit(&format!(
            "{}: {}\n",
            if dry_run {
                "review decision"
            } else if result.signed {
                "signed cli receipt"
            } else {
                "unattested cli receipt"
            },
            crate::escape_control(&saccade_core::paths::cwd(
                &out.join(if dry_run {
                    "decision.json"
                } else {
                    "receipt.json"
                }),
                absolute
            ))
        ))?;
    }
    Ok(0)
}
