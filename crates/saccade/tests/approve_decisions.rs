#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use saccade_core::evidence::{
    Artifact, Document,
    canonical::Digest,
    human::{Authority, AutomatedDecision},
};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "support/approval.rs"]
mod support;

fn save(dir: &Path, name: &str, v: u8) {
    std::fs::create_dir_all(dir).unwrap();
    RgbImage::from_pixel(8, 8, Rgb([v, v, v]))
        .save(dir.join(name))
        .unwrap();
}
fn fixture(root: &Path) -> PathBuf {
    for name in ["a.png", "b.png"] {
        save(&root.join("cap"), name, 200);
        save(&root.join("base"), name, 10);
    }
    compare(root)
}
fn compare(root: &Path) -> PathBuf {
    let out = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("compare")
        .arg(root.join("base"))
        .arg(root.join("cap"))
        .arg("--out")
        .arg(root.join("report"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    root.join("report/saccade-report.v1.json")
}
fn apply(report: &Path, decision: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["approve", "--report"])
        .arg(report)
        .arg("--decisions")
        .arg(decision)
        .arg("--out")
        .arg(out)
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn selected_cli_updates_are_hash_bound_and_unattested() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let report = fixture(root);
    let before = std::fs::read(root.join("base/a.png")).unwrap();
    let d = support::draft(&report, &root.join("plan"), &["a.png"], false);
    assert_eq!(
        before,
        std::fs::read(root.join("base/a.png")).unwrap(),
        "dry-run makes no baseline changes"
    );
    let out = apply(&report, &d, &root.join("applied"), &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        std::fs::read(root.join("base/a.png")).unwrap(),
        std::fs::read(root.join("cap/a.png")).unwrap()
    );
    assert_eq!(std::fs::read(root.join("base/b.png")).unwrap(), before);
    let doc = Document::read(&root.join("applied/receipt.json")).unwrap();
    assert_eq!(doc.authority().unwrap().label(), "cli");
    assert!(doc.require_human_authority().is_err());
    let Artifact::ApprovalReceipt(r) = doc.artifact else {
        panic!("receipt")
    };
    assert!(r.human_attestation.is_none());
    assert_eq!(r.applied.len(), 1);
    assert_eq!(r.applied[0].entry_id, "a.png");
    assert!(root.join("applied/.saccade-run").exists());
}
#[test]
fn legacy_promotions_and_forged_sources_require_new_review() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let report = fixture(root);
    let before = std::fs::read(root.join("base/a.png")).unwrap();
    let legacy = root.join("legacy.json");
    for proposal in [
        "",
        r#", "proposals":[{"question":"accept","answer":"accept","source":"human","prob":1,"proposed":false}]"#,
        r#", "proposals":[{"question":"accept","answer":"accept","source":"jev","prob":1,"proposed":true,"promoted":true}]"#,
    ] {
        let bytes = format!(
            r#"{{"schema":"saccade-decisions.v1","seed":1,"labels":["base","cap"],"sets":[{{"name":"a.png","decision":"accept"{proposal}}}]}}"#
        );
        std::fs::write(&legacy, &bytes).unwrap();
        assert!(saccade_core::view::read_decisions(&legacy).is_ok());
        let out = apply(&report, &legacy, &root.join("refused"), &[]);
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(std::fs::read(&legacy).unwrap(), bytes.as_bytes());
    }
    assert_eq!(std::fs::read(root.join("base/a.png")).unwrap(), before);
    assert!(!root.join("refused").exists());
}
#[test]
fn stale_inputs_report_and_sidecars_fail_before_any_update() {
    for change in [
        "candidate",
        "baseline",
        "absent-baseline",
        "report",
        "sidecar",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let report = fixture(root);
        std::fs::write(root.join("cap/saccade-meta.json"), b"{}").unwrap();
        let d = support::draft(&report, &root.join("plan"), &[], false);
        match change {
            "candidate" => save(&root.join("cap"), "b.png", 250),
            "baseline" => save(&root.join("base"), "b.png", 100),
            "absent-baseline" => std::fs::remove_file(root.join("base/b.png")).unwrap(),
            "report" => {
                let mut raw = std::fs::read(&report).unwrap();
                raw.push(b' ');
                std::fs::write(&report, raw).unwrap();
            }
            "sidecar" => {
                std::fs::write(root.join("cap/saccade-meta.json"), b"{\"seed\":2}").unwrap()
            }
            _ => unreachable!(),
        }
        let a_before = std::fs::read(root.join("base/a.png")).unwrap();
        let out = apply(&report, &d, &root.join("refused"), &[]);
        assert_eq!(out.status.code(), Some(2), "{change}: {out:?}");
        assert_eq!(std::fs::read(root.join("base/a.png")).unwrap(), a_before);
        assert!(!root.join("refused/receipt.json").exists());
    }
}
#[test]
fn deletions_require_exact_disposition_and_explicit_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    save(&root.join("base"), "gone.png", 10);
    let report = compare(root);
    let d = support::draft(&report, &root.join("plan"), &["gone.png"], true);
    let out = apply(&report, &d, &root.join("refused"), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(root.join("base/gone.png").exists());
    let out = apply(&report, &d, &root.join("applied"), &["--prune-missing"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(!root.join("base/gone.png").exists());
    let doc = Document::read(&root.join("applied/receipt.json")).unwrap();
    let Artifact::ApprovalReceipt(r) = doc.artifact else {
        panic!("receipt")
    };
    assert!(r.applied[0].before.is_some() && r.applied[0].after.is_none());
}
#[test]
fn reserved_automation_has_no_cli_writer_or_approval_path() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let report = fixture(root);
    let d = support::draft(&report, &root.join("plan"), &["a.png"], false);
    let doc = Document::read(&d).unwrap();
    let Artifact::HumanDecision(draft) = doc.artifact else {
        panic!("draft")
    };
    let mut automated = AutomatedDecision {
        decision_id: Digest::of_bytes(b""),
        authority: Authority::Automated {
            policy_id: "P5/reserved".into(),
            evidence_digest: draft.binding.case_id.clone(),
        },
        binding: draft.binding,
        disposition: draft.disposition,
    };
    automated.decision_id = automated.identity().unwrap();
    let doc = Document::new(Artifact::AutomatedDecision(Box::new(automated)));
    let schema: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/saccade-evidence.v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&serde_json::to_value(&doc).unwrap())
    );
    let reserved = root.join("plan/reserved.json");
    std::fs::write(&reserved, serde_json::to_vec(&doc).unwrap()).unwrap();
    let display = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("summary")
        .arg(&reserved)
        .args(["--format", "text"])
        .output()
        .unwrap();
    assert_eq!(display.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&display.stdout).contains("authority: automated;"));
    let display = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("summary")
        .arg(&reserved)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let envelope: saccade_core::evidence::action::ResultEnvelope =
        serde_json::from_slice(&display.stdout).unwrap();
    envelope.validate().unwrap();
    assert!(envelope.limits[0].starts_with("authority: automated;"));
    let out = apply(&report, &reserved, &root.join("refused"), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("automated"));
    assert!(!root.join("refused").exists());
}
#[cfg(unix)]
#[test]
fn approval_refuses_symlink_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let report = fixture(root);
    let d = support::draft(&report, &root.join("plan"), &[], false);
    std::fs::rename(root.join("base/b.png"), root.join("elsewhere.png")).unwrap();
    std::os::unix::fs::symlink(root.join("elsewhere.png"), root.join("base/b.png")).unwrap();
    let before = std::fs::read(root.join("base/a.png")).unwrap();
    let out = apply(&report, &d, &root.join("refused"), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("symlink"));
    assert_eq!(std::fs::read(root.join("base/a.png")).unwrap(), before);
}

#[test]
fn cli_cannot_claim_workbench_attestation() {
    use saccade_core::evidence::human::Channel;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let report = fixture(root);
    let path = support::draft(&report, &root.join("plan"), &["a.png"], false);
    let mut doc = Document::read(&path).unwrap();
    let Artifact::HumanDecision(d) = &mut doc.artifact else {
        panic!("draft")
    };
    d.channel = Channel::Workbench;
    d.refresh_id().unwrap();
    std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
    let before = std::fs::read(root.join("base/a.png")).unwrap();
    let out = apply(&report, &path, &root.join("refused"), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!root.join("refused/receipt.json").exists());
    assert_eq!(std::fs::read(root.join("base/a.png")).unwrap(), before);
}
