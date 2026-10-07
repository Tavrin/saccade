#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
//! Original CC0-1.0 procedural images and ephemeral OpenSSH keys; no downloads.
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

struct Fixture {
    root: tempfile::TempDir,
    key: tempfile::TempDir,
}
impl Fixture {
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(args)
            .current_dir(self.root.path())
            .env("HOME", self.root.path().join("home"))
            .output()
            .unwrap()
    }
    fn new() -> Self {
        let f = Self {
            root: tempfile::tempdir().unwrap(),
            key: tempfile::tempdir().unwrap(),
        };
        for (dir, v) in [("base", 20), ("cap", 200)] {
            fs::create_dir(f.root.path().join(dir)).unwrap();
            image::RgbImage::from_pixel(8, 8, image::Rgb([v, v, v]))
                .save(f.root.path().join(dir).join("sample.png"))
                .unwrap();
        }
        assert_eq!(
            f.cli(&["compare", "base", "cap", "--out", "report", "--json"])
                .status
                .code(),
            Some(1)
        );
        assert!(
            Command::new("ssh-keygen")
                .args(["-q", "-t", "ed25519", "-N", "", "-f"])
                .arg(f.key.path().join("key"))
                .status()
                .unwrap()
                .success()
        );
        let public = fs::read_to_string(f.key.path().join("key.pub")).unwrap();
        fs::write(
            f.root.path().join("signers"),
            format!("reviewer namespaces=\"saccade-approval\" {public}"),
        )
        .unwrap();
        ok(f.cli(&[
            "approve",
            "--report",
            "report/saccade-report.v1.json",
            "--entry",
            "sample.png",
            "--approver",
            "reviewer",
            "--dry-run",
            "--out",
            "plan",
            "--json",
        ]));
        f
    }
    fn sign(&self, namespace: &str) {
        let sig = self.root.path().join("plan/approval.json.sig");
        if sig.exists() {
            fs::remove_file(sig).unwrap();
        }
        assert!(
            Command::new("ssh-keygen")
                .args(["-Y", "sign", "-f"])
                .arg(self.key.path().join("key"))
                .args(["-n", namespace])
                .arg(self.root.path().join("plan/approval.json"))
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fn apply(&self, signed: bool) -> Output {
        let mut args = vec![
            "approve",
            "--report",
            "report/saccade-report.v1.json",
            "--decisions",
            "plan/decision.json",
            "--out",
            "applied",
            "--json",
        ];
        if signed {
            args.extend([
                "--require-signed-approval",
                "--approval-allowed-signers",
                "signers",
                "--approval-record",
                "plan/approval.json",
                "--approval-signature",
                "plan/approval.json.sig",
            ]);
        }
        self.cli(&args)
    }
    fn compare(&self) -> Output {
        self.cli(&[
            "compare",
            "base",
            "cap",
            "--approved",
            "--approval-allowed-signers",
            "signers",
            "--out",
            "checked",
            "--json",
        ])
    }
    fn value(&self, path: &str) -> Value {
        serde_json::from_slice(&fs::read(self.root.path().join(path)).unwrap()).unwrap()
    }
    fn write(&self, path: &str, value: &Value) {
        fs::write(
            self.root.path().join(path),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
fn ok(out: Output) -> Value {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}
fn code(out: Output, expected: &str) {
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["errors"][0]["code"], expected, "{value}");
}
fn schema(path: &Path, name: &str) {
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get(name).unwrap()).unwrap();
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
}

#[test]
fn signed_update_and_all_anchor_consumers_verify_content() {
    let f = Fixture::new();
    code(f.compare(), "approval_signature_required");
    f.sign("saccade-approval");
    assert_eq!(ok(f.apply(true))["data"]["authority"], "signed_cli");
    schema(
        &f.root.path().join("plan/approval.json"),
        "saccade-approval-record.v1",
    );
    schema(
        &f.root.path().join("base/.saccade-approval.json"),
        "saccade-signed-approval.v1",
    );
    ok(f.compare());
    ok(f.cli(&[
        "manifest",
        "build",
        "report",
        "--approved-anchor",
        "base/.saccade-approval.json",
        "--require-signed-approval",
        "--approval-allowed-signers",
        "signers",
        "--json",
    ]));
    for op in ["verify", "classify"] {
        ok(f.cli(&[
            "manifest",
            op,
            "report",
            "--require-signed-approval",
            "--approval-allowed-signers",
            "signers",
            "--json",
        ]));
    }
    let report_id = f.value("report/saccade-manifest.json")["reports"][0]["report_id"]
        .as_str()
        .unwrap()
        .to_owned();
    ok(f.cli(&[
        "manifest",
        "link",
        "report",
        "--report-id",
        &report_id,
        "--out",
        "link.json",
        "--require-signed-approval",
        "--approval-allowed-signers",
        "signers",
        "--json",
    ]));
    ok(f.cli(&[
        "manifest",
        "verify",
        "link.json",
        "--require-signed-approval",
        "--approval-allowed-signers",
        "signers",
        "--json",
    ]));
    // Any extra file, including metadata, invalidates the approval inventory.
    fs::write(f.root.path().join("base/new.txt"), b"extra").unwrap();
    code(f.compare(), "approval_content_mismatch");
    code(
        f.cli(&[
            "manifest",
            "verify",
            "link.json",
            "--require-signed-approval",
            "--approval-allowed-signers",
            "signers",
            "--json",
        ]),
        "approval_content_mismatch",
    );

    for op in ["verify", "classify"] {
        code(
            f.cli(&[
                "manifest",
                op,
                "report",
                "--require-signed-approval",
                "--approval-allowed-signers",
                "signers",
                "--json",
            ]),
            "approval_content_mismatch",
        );
    }
    fs::remove_file(f.root.path().join("base/new.txt")).unwrap();
    #[cfg(unix)]
    {
        let ambiguous = f.root.path().join("base/nested\\sample.png");
        fs::write(&ambiguous, b"ambiguous").unwrap();
        code(f.compare(), "unsafe_path");
        fs::remove_file(ambiguous).unwrap();
        std::os::unix::fs::symlink("sample.png", f.root.path().join("base/alias.png")).unwrap();
        code(f.compare(), "unsafe_path");
        fs::remove_file(f.root.path().join("base/alias.png")).unwrap();
    }

    fs::write(f.root.path().join("base/sample.png"), b"tampered").unwrap();
    code(f.compare(), "approval_content_mismatch");
}

#[test]
fn missing_wrong_key_namespace_and_tampered_record_cannot_mutate() {
    let f = Fixture::new();
    let before = fs::read(f.root.path().join("base/sample.png")).unwrap();
    code(
        f.cli(&[
            "approve",
            "--report",
            "report/saccade-report.v1.json",
            "--decisions",
            "plan/decision.json",
            "--require-signed-approval",
            "--out",
            "refused",
            "--json",
        ]),
        "approval_signature_required",
    );
    assert!(!f.root.path().join("refused").exists());
    f.sign("wrong-domain");
    code(f.apply(true), "approval_signature_invalid");
    f.sign("saccade-approval");
    let original = fs::read(f.root.path().join("plan/approval.json")).unwrap();
    for field in [
        "report_id",
        "report_sha256",
        "decision_id",
        "approver",
        "timestamp_unix_ms",
        "baseline",
        "scope",
        "baseline_files",
    ] {
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        value[field] = match field {
            "timestamp_unix_ms" => json!(1),
            "scope" => json!([]),
            "baseline_files" => json!({}),
            "approver" => json!("other"),
            "baseline" => json!("/other"),
            _ => json!(format!("sha256:{}", "0".repeat(64))),
        };
        f.write("plan/approval.json", &value);
        code(f.apply(true), "approval_signature_invalid");
    }
    fs::write(f.root.path().join("plan/approval.json"), original).unwrap();
    fs::write(
        f.root.path().join("signers"),
        "reviewer ssh-ed25519 invalid\n",
    )
    .unwrap();
    code(f.apply(true), "approval_signature_invalid");
    assert_eq!(
        fs::read(f.root.path().join("base/sample.png")).unwrap(),
        before
    );
    assert!(!f.root.path().join("applied").exists());
}

#[test]
fn a_valid_signature_cannot_authorize_another_scope_or_destination() {
    let f = Fixture::new();
    let original = f.value("plan/approval.json");
    for field in [
        "report_id",
        "report_sha256",
        "decision_id",
        "timestamp_unix_ms",
        "baseline",
        "scope",
        "baseline_files",
    ] {
        let mut v = original.clone();
        match field {
            "timestamp_unix_ms" => v[field] = json!(1),
            "baseline" => v[field] = json!("/other"),
            "scope" => v[field][0]["entry_id"] = json!("other.png"),
            "baseline_files" => v[field] = json!({}),
            _ => v[field] = json!(format!("sha256:{}", "0".repeat(64))),
        }
        f.write("plan/approval.json", &v);
        f.sign("saccade-approval");
        code(f.apply(true), "approval_content_mismatch");
    }
    assert!(!f.root.path().join("applied").exists());
}

#[test]
fn required_user_policy_cannot_be_overridden_and_default_stays_unattested() {
    let f = Fixture::new();
    let config = f.root.path().join("home/.config/saccade");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("approval-policy.json"), serde_json::to_vec(&json!({"require_signed_approval":true,"allowed_signers":f.root.path().join("signers")})).unwrap()).unwrap();
    code(f.apply(false), "approval_signature_required");
    code(
        f.cli(&["doctor", "--approval-allowed-signers", "other", "--json"]),
        "approval_policy_invalid",
    );
    code(
        f.cli(&["compare", "base", "cap", "--out", "denied", "--json"]),
        "approval_signature_required",
    );
    code(
        f.cli(&[
            "manifest",
            "build",
            "report",
            "--approved-anchor",
            "plan/decision.json",
            "--json",
        ]),
        "approval_signature_invalid",
    );
    fs::remove_file(config.join("approval-policy.json")).unwrap();
    assert_eq!(ok(f.apply(false))["data"]["authority"], "cli");
    assert!(!f.root.path().join("base/.saccade-approval.json").exists());
}
