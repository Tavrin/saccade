#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
#![cfg(unix)]
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
        secure(&f.root.path().join("signers"));
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
    assert_eq!(
        f.value("checked/saccade-report.v1.json")["baseline_dir"],
        "../base"
    );
    let anchor_hash = saccade_core::evidence::canonical::Digest::of_bytes(
        &fs::read(f.root.path().join("base/sample.png")).unwrap(),
    );
    f.write(
        "cases.json",
        &json!({"schema":"saccade-cases.v1", "axes":{"variant":["one"]},
        "cases":[{"case_id":"one", "variants":{"variant":"one"}, "required":true,
        "approved_anchor":{"path":"../base/sample.png", "sha256":anchor_hash.as_str().strip_prefix("sha256:").unwrap()}}]}),
    );
    ok(f.cli(&[
        "manifest",
        "build",
        "report",
        "--cases",
        "cases.json",
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
    assert!(
        Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(f.key.path().join("unauthorized"))
            .status()
            .unwrap()
            .success()
    );
    let other_public = fs::read_to_string(f.key.path().join("unauthorized.pub")).unwrap();
    fs::write(
        f.root.path().join("signers"),
        format!("reviewer namespaces=\"saccade-approval\" {other_public}"),
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
    secure(&config.join("approval-policy.json"));
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

fn secure(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}
impl Fixture {
    fn policy(&self, extra: Value) {
        let config = self.root.path().join("home/.config/saccade");
        fs::create_dir_all(&config).unwrap();
        let mut value = json!({"allowed_signers": self.root.path().join("signers")});
        for (key, val) in extra.as_object().unwrap() {
            value[key] = val.clone();
        }
        fs::write(
            config.join("approval-policy.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        secure(&config.join("approval-policy.json"));
    }
    fn replan(&self) {
        let plan = self.root.path().join("plan");
        fs::remove_dir_all(&plan).unwrap();
        ok(self.cli(&[
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
    }
}

#[test]
#[cfg(feature = "mcp")]
fn required_mcp_comparison_refuses_unsigned_and_changed_baselines() {
    use std::io::Write;
    let f = Fixture::new();
    f.policy(json!({"require_signed_approval": true}));
    code(
        f.cli(&[
            "identity",
            "base",
            "cap",
            "--out",
            "identity-unsigned",
            "--json",
        ]),
        "approval_signature_required",
    );
    #[cfg(feature = "graphics")]
    code(
        f.cli(&[
            "experiment",
            "sequence",
            "base",
            "cap",
            "--out",
            "sequence-unsigned",
            "--json",
        ]),
        "approval_signature_required",
    );
    let mcp = || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args([
                "mcp",
                "--root",
                "base",
                "--root",
                "cap",
                "--out-root",
                "outputs",
            ])
            .current_dir(f.root.path())
            .env("HOME", f.root.path().join("home"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_measure",
            "arguments":{"operation":"compare","baseline_dir":f.root.path().join("base"),"capture_dir":f.root.path().join("cap"),"out":"comparison"}}});
        let mut input = child.stdin.take().unwrap();
        writeln!(input, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"signed-test","version":"1"}}})).unwrap();
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        writeln!(input, "{request}").unwrap();
        drop(input);
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success(), "{result:?}");
        let responses: Vec<Value> = String::from_utf8(result.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        responses.last().unwrap()["result"].clone()
    };
    fs::create_dir(f.root.path().join("outputs")).unwrap();
    let result = mcp();
    assert_eq!(
        result["structuredContent"]["errors"][0]["code"], "approval_signature_required",
        "{result}"
    );
    assert_eq!(result["isError"], true);
    f.sign("saccade-approval");
    ok(f.apply(true));
    assert!(mcp()["isError"].as_bool() != Some(true));
    fs::write(f.root.path().join("base/sample.png"), b"replacement").unwrap();
    assert_eq!(
        mcp()["structuredContent"]["errors"][0]["code"],
        "approval_content_mismatch"
    );
}

#[test]
#[cfg(unix)]
fn replacement_between_verification_and_measurement_is_refused() {
    use std::{
        io::Write,
        os::unix::fs::OpenOptionsExt,
        time::{Duration, Instant},
    };
    let f = Fixture::new();
    f.sign("saccade-approval");
    ok(f.apply(true));
    let fifo = f.root.path().join("config.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args([
            "compare",
            "base",
            "cap",
            "--approved",
            "--approval-allowed-signers",
            "signers",
            "--config",
            "config.fifo",
            "--out",
            "seam",
            "--json",
            "--allow-out-near-captures",
        ])
        .current_dir(f.root.path())
        .env("HOME", f.root.path().join("home"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut reads = 0;
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "configuration seam timed out"
        );
        if child.try_wait().unwrap().is_some() {
            break;
        }
        match fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)
        {
            Ok(mut writer) => {
                if reads == 0 {
                    // Opening the FIFO writer proves the consumer passed signature
                    // verification and is blocked on its first config read.
                    image::RgbImage::from_pixel(8, 8, image::Rgb([42, 42, 42]))
                        .save(f.root.path().join("base/sample.png"))
                        .unwrap();
                }
                writer.write_all(b"threshold = 0.02\n").unwrap();
                drop(writer);
                reads += 1;
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert!(reads >= 1);
    code(
        child.wait_with_output().unwrap(),
        "approval_content_mismatch",
    );
    assert!(!f.root.path().join("seam/saccade-report.v1.json").exists());
}

#[test]
fn trusted_ledger_refuses_restored_old_approval_and_age_is_enforced() {
    let f = Fixture::new();
    let ledger = f.root.path().join("ledger.json");
    fs::write(&ledger, b"{}").unwrap();
    secure(&ledger);
    f.policy(json!({"ledger":ledger}));
    f.replan();
    assert_eq!(f.value("plan/approval.json")["sequence"], 1);
    f.sign("saccade-approval");
    ok(f.apply(true));
    ok(f.compare());
    let old_image = fs::read(f.root.path().join("base/sample.png")).unwrap();
    let old_envelope = fs::read(f.root.path().join("base/.saccade-approval.json")).unwrap();
    image::RgbImage::from_pixel(8, 8, image::Rgb([250, 250, 250]))
        .save(f.root.path().join("cap/sample.png"))
        .unwrap();
    assert_eq!(
        f.cli(&["compare", "base", "cap", "--out", "report", "--json"])
            .status
            .code(),
        Some(1)
    );
    f.replan();
    assert_eq!(f.value("plan/approval.json")["sequence"], 2);
    f.sign("saccade-approval");
    fs::remove_dir_all(f.root.path().join("applied")).unwrap();
    ok(f.apply(true));
    ok(f.compare());
    fs::write(f.root.path().join("base/sample.png"), old_image).unwrap();
    fs::write(
        f.root.path().join("base/.saccade-approval.json"),
        old_envelope,
    )
    .unwrap();
    code(f.compare(), "approval_replayed");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ledger, fs::Permissions::from_mode(0o666)).unwrap();
        code(f.compare(), "approval_trust_unsafe");
        secure(&ledger);
    }
    // Max age is separate from rollback state, and is checked on genuine signatures.
    f.policy(json!({"max_approval_age_seconds":0}));
    code(f.compare(), "approval_expired");
}

#[test]
#[cfg(unix)]
fn unsafe_signers_are_refused_and_path_cannot_replace_verifier() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.sign("saccade-approval");
    fs::set_permissions(
        f.root.path().join("signers"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    code(f.apply(true), "approval_trust_unsafe");
    secure(&f.root.path().join("signers"));
    let fake = f.root.path().join("fake-bin");
    fs::create_dir(&fake).unwrap();
    fs::write(fake.join("ssh-keygen"), b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(fake.join("ssh-keygen"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        f.root.path().join("plan/approval.json.sig"),
        b"forged signature",
    )
    .unwrap();
    code(
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args([
                "approve",
                "--report",
                "report/saccade-report.v1.json",
                "--decisions",
                "plan/decision.json",
                "--out",
                "forged",
                "--json",
                "--require-signed-approval",
                "--approval-allowed-signers",
                "signers",
                "--approval-record",
                "plan/approval.json",
                "--approval-signature",
                "plan/approval.json.sig",
            ])
            .current_dir(f.root.path())
            .env("HOME", f.root.path().join("home"))
            .env("PATH", &fake)
            .output()
            .unwrap(),
        "approval_signature_invalid",
    );
    assert!(!f.root.path().join("forged").exists());
}
