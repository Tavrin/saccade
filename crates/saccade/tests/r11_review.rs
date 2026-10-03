//! Final six-tool and explicit startup authorization contract, entirely offline.
#![cfg(all(feature = "ai", feature = "mcp"))]
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
fn tool(id: u64, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}
fn run(root: &Path, out: &Path, extra: &[&str], messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .arg("--root")
        .arg(root)
        .arg("--out-root")
        .arg(out)
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    for m in messages {
        writeln!(child.stdin.as_mut().unwrap(), "{m}").unwrap();
    }
    child.stdin.take();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
#[test]
fn mcp_startup_requires_flag_and_budget_and_tools_cannot_raise_authority() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("root");
    let out = t.path().join("out");
    std::fs::create_dir(&root).unwrap();
    for args in [vec!["--allow-provider-calls"], vec!["--budget-calls", "2"]] {
        let o = Command::new(BIN)
            .arg("mcp")
            .arg("--root")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(2));
    }
    let reply = run(
        &root,
        &out,
        &[],
        &[
            tool(
                1,
                "saccade_review",
                json!({"operation":"run","artifact":"missing","out":"proposal"}),
            ),
            tool(
                2,
                "saccade_review",
                json!({"operation":"run","artifact":"missing","out":"proposal","allow_provider_calls":true}),
            ),
        ],
    );
    assert_eq!(
        reply[0]["result"]["structuredContent"]["errors"][0]["code"],
        "network_authorization_required"
    );
    assert_eq!(reply[1]["result"]["isError"], true);
    let reply = run(
        &root,
        &out,
        &["--allow-provider-calls", "--budget-calls", "1"],
        &[tool(
            1,
            "saccade_review",
            json!({"operation":"run","artifact":"missing","out":"proposal","budget_calls":2}),
        )],
    );
    assert_eq!(reply[0]["result"]["isError"], true);
    assert!(
        reply[0]["result"]["structuredContent"]["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("startup")
    );
}
#[test]
fn six_tools_share_local_evidence_requests_proposals_and_human_escalation() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("root");
    let out = t.path().join("out");
    std::fs::create_dir(&root).unwrap();
    for name in ["base", "capture"] {
        std::fs::create_dir(root.join(name)).unwrap();
        image::RgbImage::from_pixel(
            12,
            12,
            image::Rgb([if name == "base" { 40 } else { 50 }; 3]),
        )
        .save(root.join(name).join("a.png"))
        .unwrap();
    }
    let report = out
        .join("report/saccade-report.v1.json")
        .to_string_lossy()
        .into_owned();
    let request = out.join("request.json").to_string_lossy().into_owned();
    let replies = run(
        &root,
        &out,
        &[],
        &[
            json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
            tool(
                1,
                "saccade_measure",
                json!({"operation":"compare","baseline_dir":"base","capture_dir":"capture","out":"report"}),
            ),
            tool(
                2,
                "saccade_inspect",
                json!({"operation":"summary","artifact":report}),
            ),
            tool(
                3,
                "saccade_evidence",
                json!({"operation":"request","artifact":report,"question":"triage.route.v1","out":"request.json"}),
            ),
            tool(
                4,
                "saccade_review",
                json!({"operation":"preview","artifact":report}),
            ),
            tool(
                5,
                "saccade_ask_human",
                json!({"operation":"request","artifact":request,"out":"human.json"}),
            ),
        ],
    );
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 6);
    for name in [
        "saccade_measure",
        "saccade_inspect",
        "saccade_evidence",
        "saccade_review",
        "saccade_propose",
        "saccade_ask_human",
    ] {
        assert!(tools.iter().any(|t| t["name"] == name));
    }
    for r in &replies[1..] {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }
    assert_eq!(
        replies[4]["result"]["structuredContent"]["counts"]["dispatched_calls"],
        0
    );
    let doc = saccade_core::evidence::Document::read(Path::new(&request)).unwrap();
    let saccade_core::evidence::Artifact::DecisionRequest(r) = doc.artifact else {
        panic!("request")
    };
    let answer = json!({"request_id":r.request_id,"answer":"abstain","probabilities":null,"reason_codes":[],"evidence_ids":[],"missing_evidence":[],"depends_on_model_observation":false,"note":"offline proposal"});
    let answers = out.join("answers.json");
    let proposal = saccade_core::evidence::proposal::DecisionProposal::from_response(
        &r,
        serde_json::from_value(answer).unwrap(),
        saccade_core::evidence::proposal::ProviderAudit {
            identity: saccade_core::evidence::request::ProviderIdentity {
                provider: "offline-fixture".into(),
                model: "fixture/1".into(),
                revision: None,
            },
            payload_sha256: saccade_core::evidence::canonical::Digest::of_bytes(b"payload"),
            response_sha256: saccade_core::evidence::canonical::Digest::of_bytes(b"response"),
            execution_ref: None,
            timestamp_unix_ms: None,
        },
    )
    .unwrap();
    std::fs::write(
        &answers,
        saccade_core::evidence::canonical::bytes(&saccade_core::evidence::Document::new(
            saccade_core::evidence::Artifact::DecisionProposal(Box::new(proposal)),
        ))
        .unwrap(),
    )
    .unwrap();
    let replies = run(
        &root,
        &out,
        &[],
        &[tool(
            6,
            "saccade_propose",
            json!({"operation":"answers","artifact":request,"answers":answers.to_string_lossy(),"out":"proposal.json"}),
        )],
    );
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
}
#[test]
fn denied_root_still_blocks_execution_after_explicit_startup_authorization() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("root");
    let out = t.path().join("out");
    std::fs::create_dir(&root).unwrap();
    for name in ["base", "capture"] {
        std::fs::create_dir(root.join(name)).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([80; 3]))
            .save(root.join(name).join("a.png"))
            .unwrap();
    }
    let report = out
        .join("report/saccade-report.v1.json")
        .to_string_lossy()
        .into_owned();
    let config = t.path().join("user.toml");
    std::fs::write(
        &config,
        format!(
            "[[roots]]\nid='private'\npath={:?}\negress='deny'\n",
            root.to_string_lossy()
        ),
    )
    .unwrap();
    let reply = run(
        &root,
        &out,
        &[
            "--allow-provider-calls",
            "--budget-calls",
            "2",
            "--user-config",
            config.to_str().unwrap(),
        ],
        &[
            tool(
                1,
                "saccade_measure",
                json!({"operation":"compare","baseline_dir":"base","capture_dir":"capture","out":"report"}),
            ),
            tool(
                2,
                "saccade_review",
                json!({"operation":"run","artifact":report,"out":"review.json"}),
            ),
        ],
    );
    assert_eq!(
        reply[1]["result"]["structuredContent"]["errors"][0]["code"],
        "egress_denied"
    );
    assert!(!t.path().join("attempts/production.json").exists());
}
