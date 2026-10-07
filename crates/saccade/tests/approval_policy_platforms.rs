#![allow(clippy::expect_used, missing_docs)]
//! Policy discovery must not require signed approvals on an unconfigured host.
use std::process::{Command, Output};

fn cli(home: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command.args(args).env("HOME", home).current_dir(home);
    #[cfg(windows)]
    command.env("ProgramData", home.join("program-data"));
    command.output().expect("CLI")
}

#[test]
fn absent_policy_compare_succeeds_without_attestation() {
    let temp = tempfile::tempdir().expect("fixture");
    for directory in ["base", "capture"] {
        let path = temp.path().join(directory);
        std::fs::create_dir(&path).expect("directory");
        image::RgbImage::from_pixel(8, 8, image::Rgb([20, 20, 20]))
            .save(path.join("sample.png"))
            .expect("image");
    }
    let output = cli(
        temp.path(),
        &["compare", "base", "capture", "--out", "report", "--json"],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result");
    assert_ne!(value["execution"], "error", "{value}");
    assert!(!temp.path().join("base/.saccade-approval.json").exists());
}

#[test]
#[cfg(windows)]
fn windows_refuses_enablement_and_installed_policies() {
    let temp = tempfile::tempdir().expect("fixture");
    let refuses = |args: &[&str]| {
        let output = cli(temp.path(), args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result");
        assert_eq!(
            value["errors"][0]["code"], "approval_trust_unsafe",
            "{value}"
        );
    };
    refuses(&["doctor", "--require-signed-approval", "--json"]);
    refuses(&["doctor", "--approval-allowed-signers", "signers", "--json"]);
    for directory in ["program-data/saccade", ".config/saccade"] {
        let path = temp.path().join(directory);
        std::fs::create_dir_all(&path).expect("directory");
        let policy = path.join("approval-policy.json");
        // Even an explicit policy-off file needs a trust boundary to be accepted.
        std::fs::write(&policy, b"{\"require_signed_approval\":false}").expect("policy");
        refuses(&["doctor", "--json"]);
        std::fs::remove_file(policy).expect("remove policy");
    }
}

#[test]
#[cfg(unix)]
fn required_policy_accepts_directory_alias_but_rejects_different_signers() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().expect("fixture");
    let real = temp.path().join("real");
    let alias = temp.path().join("alias");
    std::fs::create_dir(&real).expect("directory");
    symlink(&real, &alias).expect("directory alias");
    std::fs::write(real.join("signers"), b"unused for startup").expect("signers");
    let config = temp.path().join(".config/saccade");
    std::fs::create_dir_all(&config).expect("config");
    let policy = config.join("approval-policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&serde_json::json!({
            "require_signed_approval": true,
            "allowed_signers": alias.join("signers")
        }))
        .expect("policy JSON"),
    )
    .expect("policy");
    std::fs::set_permissions(&policy, std::fs::Permissions::from_mode(0o600)).expect("mode");
    // current_dir() resolves the symlink; the selected policy retains the alias.
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["doctor", "--approval-allowed-signers", "signers", "--json"])
        .env("HOME", temp.path())
        .current_dir(&alias)
        .output()
        .expect("CLI");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let output = cli(
        temp.path(),
        &[
            "doctor",
            "--approval-allowed-signers",
            "real/other",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("result");
    assert_eq!(value["errors"][0]["code"], "approval_policy_invalid");
}
