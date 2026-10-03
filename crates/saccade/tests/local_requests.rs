//! Closed local request/proposal/queue workflows use canonical R4 contracts.
#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::evidence::{Artifact, Document};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
#[test]
fn request_proposals_and_human_items_remain_bound_and_unresolved() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../saccade-core/tests/fixtures/evidence");
    let mut case = Document::read(&fixtures.join("case.json")).unwrap();
    let request = Document::read(&fixtures.join("decision_request.json")).unwrap();
    if let (Artifact::Case(case), Artifact::DecisionRequest(request)) =
        (&mut case.artifact, request.artifact)
    {
        case.requests.push(*request);
    }
    case.validate().unwrap();
    std::fs::write(root.join("case.json"), serde_json::to_vec(&case).unwrap()).unwrap();
    let Artifact::Case(case) = case.artifact else {
        panic!("fixture");
    };
    let question = &case.requests[0].question.id;
    let output = Command::new(BIN)
        .current_dir(root)
        .args([
            "review",
            "request",
            "case.json",
            "--question",
            question,
            "--out",
            "request.json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let request = Document::read(&root.join("request.json")).unwrap();
    assert!(matches!(request.artifact, Artifact::DecisionRequest(_)));
    std::fs::copy(
        fixtures.join("decision_proposal.json"),
        root.join("answers.json"),
    )
    .unwrap();
    let output = Command::new(BIN)
        .current_dir(root)
        .args([
            "review",
            "propose",
            "request.json",
            "--answers",
            "answers.json",
            "--out",
            "proposed.json",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["review"], "unresolved");
    assert!(matches!(
        Document::read(&root.join("proposed.json"))
            .unwrap()
            .artifact,
        Artifact::DecisionProposal(_)
    ));
    for _ in 0..2 {
        let output = Command::new(BIN)
            .current_dir(root)
            .args([
                "review",
                "ask",
                "request.json",
                "--out",
                "human.json",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["execution"], "pending");
    }
    let mut doc = Document::read(&root.join("request.json")).unwrap();
    let Artifact::DecisionRequest(request) = &mut doc.artifact else {
        panic!("fixture");
    };
    request.question.text.push_str(" changed");
    request.refresh_id().unwrap();
    std::fs::write(root.join("changed.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
    let output = Command::new(BIN)
        .current_dir(root)
        .args([
            "review",
            "propose",
            "changed.json",
            "--answers",
            "answers.json",
            "--out",
            "rejected.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!root.join("rejected.json").exists());
}
