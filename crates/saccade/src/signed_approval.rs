//! CLI startup policy and presentation adapter.
use crate::agent::CliError;
use saccade_core::workflows::signed_approval::Policy;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};
static POLICY: OnceLock<Policy> = OnceLock::new();
fn system_policy_path() -> Result<PathBuf, CliError> {
    #[cfg(target_os = "macos")]
    {
        // /etc is a system symlink on macOS; use its real directory.
        Ok(PathBuf::from("/private/etc/saccade/approval-policy.json"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        Ok(PathBuf::from("/etc/saccade/approval-policy.json"))
    }
    #[cfg(windows)]
    {
        // Discovery only: no ACL trust boundary is implemented on Windows.
        let directory = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        if !directory.is_absolute() {
            return Err(error(
                "approval_policy_invalid",
                "ProgramData must be absolute",
            ));
        }
        Ok(directory.join("saccade").join("approval-policy.json"))
    }
}
pub(crate) fn policy() -> &'static Policy {
    POLICY.get_or_init(Policy::default)
}
pub(crate) fn init(required: bool, signers: Option<PathBuf>) -> Result<(), CliError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let loaded = Policy::load(
        &system_policy_path()?,
        home.as_deref(),
        required,
        signers,
        &std::env::current_dir().map_err(|e| CliError::io(e.to_string()))?,
    )?;
    POLICY.set(loaded).map_err(|_| {
        CliError::new(
            "approval_policy_invalid",
            "approval policy already initialized",
        )
    })?;
    if self::required() {
        saccade_core::run::set_approval_guard(|baseline| {
            check_baseline(baseline).map_err(|e| saccade_core::Error::ApprovalRefused {
                code: e.code,
                message: e.message,
            })
        })?;
    }
    Ok(())
}
#[cfg(windows)]
fn error(code: &'static str, message: &str) -> CliError {
    CliError::new(code, message)
}
pub(crate) fn required() -> bool {
    policy().require_signed_approval
}
#[derive(clap::Args, Default)]
pub(crate) struct SigningArgs {
    /// Human principal listed in the external OpenSSH allowed-signers file (plan only).
    #[arg(long)]
    pub approver: Option<String>,
    /// Exact approval.json from the reviewed dry run, signed externally.
    #[arg(long)]
    pub approval_record: Option<PathBuf>,
    /// Detached OpenSSH signature produced with namespace saccade-approval.
    #[arg(long, requires = "approval_record")]
    pub approval_signature: Option<PathBuf>,
}
impl SigningArgs {
    pub(crate) fn options(&self) -> saccade_core::workflows::signed_approval::SigningArgs {
        saccade_core::workflows::signed_approval::SigningArgs {
            approver: self.approver.clone(),
            approval_record: self.approval_record.clone(),
            approval_signature: self.approval_signature.clone(),
        }
    }
}
pub(crate) fn check_baseline(baseline: &Path) -> Result<BTreeMap<String, String>, CliError> {
    Ok(policy().check_baseline(baseline)?)
}
pub(crate) fn run(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    config: &saccade_core::config::RunConfig,
    verified: Option<&BTreeMap<String, String>>,
) -> Result<saccade_core::Report, CliError> {
    {
        let report = policy().run(baseline, capture, out, config, verified)?;
        for warning in saccade_core::workflows::run_warnings(config, &report, baseline) {
            eprintln!("{warning}");
        }
        Ok(report)
    }
}
