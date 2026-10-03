//! R8 portable report, anonymous export and explicit human-label contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use saccade_core::evidence::{Artifact, Document};
use std::path::Path;
use std::process::Command;
fn png(path: &Path, value: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(16, 16, image::Rgb([value, value, value]))
        .save(path)
        .unwrap();
}
fn run(args: &[&str], root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}
#[test]
fn relocated_offline_bundles_explain_identity_and_change() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    png(&root.join("base/scene.png"), 20);
    png(&root.join("cap/scene.png"), 200);
    for (operation, exit) in [("compare", 1), ("identity", 1)] {
        let result = run(&[operation, "base", "cap", "--out", operation], root);
        assert_eq!(result.status.code(), Some(exit), "{result:?}");
        let relocated = root.join(format!("moved-{operation}"));
        std::fs::rename(root.join(operation), &relocated).unwrap();
        let document = relocated.join("evidence.json");
        let Artifact::Case(case) = Document::read(&document).unwrap().artifact else {
            panic!("case")
        };
        case.measurement.report.verify(&document).unwrap();
        for input in &case.inputs {
            input.content.verify(&document).unwrap();
            for sidecar in &input.sidecars {
                sidecar.verify(&document).unwrap();
            }
        }
        let html = std::fs::read_to_string(relocated.join("index.html")).unwrap();
        for phrase in [
            "Claim",
            "Comparison validity",
            "What changed",
            "Acceptance criteria",
            "Model proposals",
            "Human disposition",
            "What remains unproven",
            "Next action",
            "Exact input hashes",
        ] {
            assert!(html.contains(phrase), "missing {phrase}");
        }
        assert!(html.contains(if operation == "identity" {
            "Exact native decoded-sample equality"
        } else {
            "Image change against"
        }));
        assert!(relocated.join(".saccade-run").is_file());
        assert!(relocated.join("assets").is_dir());
        assert!(relocated.join("decisions.json").is_file());
    }
    std::fs::remove_dir_all(root.join("base")).unwrap();
    std::fs::remove_dir_all(root.join("cap")).unwrap();
    for operation in ["compare", "identity"] {
        let doc = root.join(format!("moved-{operation}/evidence.json"));
        let Artifact::Case(case) = Document::read(&doc).unwrap().artifact else {
            panic!("case")
        };
        for input in case.inputs {
            input.content.verify(&doc).unwrap();
        }
    }
}
#[test]
fn anonymous_bundle_hides_names_metadata_and_source_hashes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let secret = "private-candidate-scene.png";
    png(&root.join("private-baseline").join(secret), 20);
    png(&root.join("private-candidate").join(secret), 200);
    std::fs::write(
        root.join("private-candidate/saccade-meta.json"),
        br#"{"implementation":"private-renderer-change"}"#,
    )
    .unwrap();
    let result = run(
        &[
            "view",
            "private-baseline",
            "private-candidate",
            "--blind",
            "--seed",
            "7",
            "--key-out",
            "anonymous-blind-key.json",
            "--out",
            "anonymous",
        ],
        root,
    );
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    let out = root.join("anonymous");
    assert!(!out.join("blind-key.json").exists());
    let key =
        saccade_core::view::read_blind_key(&saccade_core::view::private_key_path(&out)).unwrap();
    let hashes = key
        .hashes
        .values()
        .flatten()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    for entry in walkdir_entries(&out) {
        let name = entry.strip_prefix(&out).unwrap().to_string_lossy();
        assert!(!name.contains(secret));
        let bytes = std::fs::read(entry).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for private in [
            "private-baseline",
            "private-candidate",
            "private-renderer-change",
            secret,
        ] {
            assert!(!text.contains(private));
        }
        for hash in &hashes {
            assert!(!text.contains(hash));
        }
    }
    let bad = run(
        &[
            "view",
            "private-baseline",
            "private-candidate",
            "--blind",
            "--key-out",
            "bad/key.json",
            "--out",
            "bad",
        ],
        root,
    );
    assert_eq!(bad.status.code(), Some(2));
}
#[test]
fn anonymous_export_refuses_files_from_a_previous_named_view() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    png(&root.join("base/private-scene.png"), 20);
    png(&root.join("cap/private-scene.png"), 200);
    let named = run(&["view", "base", "cap", "--out", "view"], root);
    assert_eq!(named.status.code(), Some(0), "{named:?}");
    let original = std::fs::read(root.join("view/index.html")).unwrap();
    let blind = run(
        &[
            "view",
            "base",
            "cap",
            "--blind",
            "--key-out",
            "private-key.json",
            "--out",
            "view",
        ],
        root,
    );
    assert_eq!(blind.status.code(), Some(2), "{blind:?}");
    assert!(String::from_utf8_lossy(&blind.stderr).contains("empty or new"));
    assert_eq!(
        std::fs::read(root.join("view/index.html")).unwrap(),
        original
    );
    assert!(!root.join("private-key.json").exists());
}
fn walkdir_entries(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files.extend(walkdir_entries(&p));
        } else {
            files.push(p);
        }
    }
    files
}
#[cfg(feature = "ai")]
#[test]
fn human_label_export_never_infers_answers_from_baseline_disposition() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    png(&root.join("base/scene.png"), 20);
    png(&root.join("cap/scene.png"), 200);
    assert_eq!(
        run(&["compare", "base", "cap", "--out", "report"], root)
            .status
            .code(),
        Some(1)
    );
    let path = root.join("report/evidence.json");
    let Artifact::Case(mut case) = Document::read(&path).unwrap().artifact else {
        panic!("case");
    };
    let report: saccade_core::Report =
        serde_json::from_slice(&std::fs::read(root.join("report/saccade-report.v1.json")).unwrap())
            .unwrap();
    saccade_core::judge_evidence::report_features(&mut case, &report).unwrap();
    saccade_core::judge_evidence::prepare_context(&mut case).unwrap();
    let request = Box::new(
        saccade_core::judge_evidence::encode(&case, "triage.route.v1", Default::default()).unwrap(),
    );
    case.requests.push(*request.clone());
    use saccade_core::evidence::{
        canonical::Digest,
        case::{ArtifactRef, Availability},
        human::{Channel, Disposition, HumanDecision, Label, ReviewBinding, ReviewerExposure},
    };
    let mut decision = HumanDecision {
        decision_id: Digest::of_bytes(b""),
        binding: ReviewBinding::for_case(
            &case,
            ArtifactRef::from_file(&root.join("report/index.html"), &path, false).unwrap(),
        )
        .unwrap(),
        disposition: Disposition::Accept,
        approve_deletions: false,
        channel: Channel::Workbench,
        exposure: ReviewerExposure {
            mapping_access: Availability::Available { value: false },
            implementation_context: Availability::Available { value: false },
            reviewer: Some("reviewer".into()),
        },
        note: "Human accepted this baseline change, without answering every question".into(),
        timestamp_unix_ms: Some(1),
    };
    decision.refresh_id().unwrap();
    case.human_decisions.push(decision.clone());
    std::fs::write(
        &path,
        serde_json::to_vec(&Document::new(Artifact::Case(case))).unwrap(),
    )
    .unwrap();
    let out = run(
        &[
            "inspect",
            "export",
            "report",
            "--format",
            "labels",
            "--out",
            "labels.json",
        ],
        root,
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("labels.json")).unwrap()).unwrap();
    assert_eq!(value["schema"], "saccade-labels.v2");
    assert_eq!(value["items"], serde_json::json!([]));
    let mut item = saccade_core::labels::QuestionLabel {
        label_id: Digest::of_bytes(b""),
        label: Label {
            case_id: request.case_id.clone(),
            request_id: request.request_id.clone(),
            question_id: request.question.id.clone(),
            answer: "needs_eyes".into(),
            human_decision_id: decision.decision_id.clone(),
            exposure: decision.exposure.clone(),
        },
        saw_model_proposals: Availability::Available { value: false },
        vision_rubric: None,
        supersedes: None,
        test_retest_of: None,
    };
    item.label_id = item.identity().unwrap();
    let mut records = saccade_core::labels::QuestionLabels {
        schema: saccade_core::labels::LABEL_RECORDS_SCHEMA.into(),
        kind: "human_label_records".into(),
        items: vec![item],
    };
    let records_path = root.join("report/human-label-records.json");
    std::fs::write(&records_path, serde_json::to_vec(&records).unwrap()).unwrap();
    let exported = run(
        &[
            "inspect",
            "export",
            "report",
            "--format",
            "labels",
            "--out",
            "explicit.json",
        ],
        root,
    );
    assert_eq!(exported.status.code(), Some(0), "{exported:?}");
    let labels: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("explicit.json")).unwrap()).unwrap();
    assert_eq!(labels["items"][0]["answer"], "needs_eyes");
    records.items[0].saw_model_proposals = Availability::Available { value: true };
    records.items[0].label_id = records.items[0].identity().unwrap();
    std::fs::write(&records_path, serde_json::to_vec(&records).unwrap()).unwrap();
    assert_eq!(
        run(
            &[
                "inspect",
                "export",
                "report",
                "--format",
                "labels",
                "--out",
                "exposed.json"
            ],
            root
        )
        .status
        .code(),
        Some(0)
    );
    let exposed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("exposed.json")).unwrap()).unwrap();
    assert_eq!(exposed["items"], serde_json::json!([]));
}
