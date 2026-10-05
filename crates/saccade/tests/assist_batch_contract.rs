//! Coordinator-only offline Batch and optional-routing CLI/MCP fixtures.
#![cfg(all(feature = "assist", feature = "mcp"))]
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use saccade_core::assist::{
    self, batch,
    execution::{CacheKey, Cached},
    schema::*,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn cli(args: &[&str], tail: &[(&str, &Path)]) -> (i32, Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command.args(args);
    for (flag, path) in tail {
        command.arg(flag).arg(path);
    }
    let output = command.arg("--json").output().unwrap();
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
fn mcp(root: &Path, out: &Path, args: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("mcp")
        .arg("--root")
        .arg(root)
        .arg("--out-root")
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.as_mut().unwrap(),"{}",json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_review","arguments":args}})).unwrap();
    drop(child.stdin.take());
    serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap()
}
fn fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let inputs = temp.path().join("inputs");
    let outputs = temp.path().join("outputs");
    std::fs::create_dir(&inputs).unwrap();
    std::fs::create_dir(&outputs).unwrap();
    let image = inputs.join("capture.png");
    image::RgbImage::from_pixel(20, 20, image::Rgb([128, 128, 128]))
        .save(&image)
        .unwrap();
    (temp, inputs, outputs, image)
}
fn prepare(image: &Path, out: &Path) {
    let (exit, _) = cli(
        &[
            "review",
            "check-ui",
            "Continuer",
            "--box",
            "0,0,20,20",
            "--experimental",
            "--offline",
            "--bypass-cache",
            "--gemini-revision",
            "r1",
        ],
        &[("--image", image), ("--out", out)],
    );
    assert_eq!(exit, 4);
    assert!(out.join("batch-plan.json").is_file());
}
#[test]
#[ignore = "heavy: wave4-batch-routing"]
fn wave4_public_batch_sources_status_collection_and_mcp_agree() {
    let (_temp, inputs, outputs, image) = fixture();
    let prepared = outputs.join("prepared");
    prepare(&image, &prepared);
    let plan = prepared.join("batch-plan.json");
    let job = outputs.join("job.json");
    let (exit, submitted) = cli(
        &["review", "assist", "batch", "submit", "--experimental"],
        &[("--plan", &plan), ("--job", &job)],
    );
    assert_eq!(exit, 0);
    assert_eq!(submitted["data"]["state"], "planned");
    let (exit, status) = cli(
        &["review", "assist", "batch", "status", "--experimental"],
        &[("--plan", &plan), ("--job", &job)],
    );
    assert_eq!(exit, 0);
    let mirrored = mcp(
        &inputs,
        &outputs,
        json!({"operation":"batch-status","artifact":plan,"out":job,"experimental":true}),
    );
    assert_eq!(
        mirrored["result"]["structuredContent"]["data"],
        status["data"]
    );
    let frozen: batch::FrozenPlan = assist::decode(&std::fs::read(&plan).unwrap()).unwrap();
    batch::begin_submit(&job, &frozen.plan).unwrap();
    batch::submitted(&job, &frozen.plan, "batches/fixture").unwrap();
    let response = inputs.join("response.json");
    assist::write(&response,&json!({"name":"batches/fixture","state":"BATCH_STATE_SUCCEEDED","response":{"inlinedResponses":[{"metadata":frozen.plan.requests[0]["metadata"],"response":{"modelVersion":"r1","usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":20,"thoughtsTokenCount":10,"totalTokenCount":130}}}]}})).unwrap();
    let (exit, collected) = cli(
        &["review", "assist", "batch", "collect", "--experimental"],
        &[
            ("--plan", &plan),
            ("--job", &job),
            ("--response", &response),
        ],
    );
    assert_eq!(exit, 0);
    assert_eq!(collected["data"]["state"], "completed");
    std::fs::write(&image, b"changed pixels").unwrap();
    let (exit, result) = cli(
        &["review", "assist", "batch", "status", "--experimental"],
        &[("--plan", &plan), ("--job", &job)],
    );
    assert_ne!(exit, 0);
    assert_eq!(result["execution"], "error");
}
#[test]
#[ignore = "heavy: wave4-batch-routing"]
fn wave4_batch_refuses_payload_forgery_missing_closure_and_mcp_network_authority() {
    let (_temp, inputs, outputs, image) = fixture();
    let prepared = outputs.join("prepared");
    prepare(&image, &prepared);
    let plan = prepared.join("batch-plan.json");
    let job = outputs.join("job.json");
    let refused = mcp(
        &inputs,
        &outputs,
        json!({"operation":"batch-submit","artifact":plan,"out":job,"experimental":true,"run":true}),
    );
    assert_eq!(refused["result"]["isError"], true);
    let mut frozen: batch::FrozenPlan = assist::decode(&std::fs::read(&plan).unwrap()).unwrap();
    let mut request = frozen.plan.requests[0]["request"].clone();
    request["contents"][0]["parts"][0]["text"] = json!("unrelated evidence");
    let id = assist::digest(&request).unwrap();
    frozen.tasks[0].job_ids = vec![id.as_str().into()];
    frozen.plan.requests =
        vec![saccade_core::judge_provider::batch::inline_request(id.as_str(), request).unwrap()];
    assist::write(&plan, &frozen).unwrap();
    let (exit, _) = cli(
        &["review", "assist", "batch", "submit", "--experimental"],
        &[("--plan", &plan), ("--job", &outputs.join("forged.json"))],
    );
    assert_ne!(exit, 0);
    frozen.tasks[0].transitive.clear();
    assist::write(&plan, &frozen).unwrap();
    let (exit, _) = cli(
        &["review", "assist", "batch", "submit", "--experimental"],
        &[("--plan", &plan), ("--job", &outputs.join("missing.json"))],
    );
    assert_ne!(exit, 0);
}

#[test]
#[ignore = "heavy: wave4-batch-routing"]
fn wave4_report_batch_plan_binds_the_original_pair_and_report() {
    let (_temp, inputs, outputs, before) = fixture();
    let after = inputs.join("after.png");
    image::RgbImage::from_pixel(20, 20, image::Rgb([20, 40, 60]))
        .save(&after)
        .unwrap();
    let report_dir = inputs.join("report");
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .arg("compare")
        .arg(&before)
        .arg(&after)
        .arg("--out")
        .arg(&report_dir)
        .output()
        .unwrap();
    assert!(matches!(result.status.code(), Some(0 | 1)));
    let prepared = outputs.join("pair");
    let report = report_dir.join("saccade-report.v1.json");
    let (exit, _) = cli(
        &[
            "review",
            "explain",
            "--experimental",
            "--offline",
            "--bypass-cache",
            "--gemini-revision",
            "r1",
        ],
        &[("--report", &report), ("--out", &prepared)],
    );
    assert_eq!(exit, 4);
    let plan = prepared.join("batch-plan.json");
    let frozen: batch::FrozenPlan = assist::decode(&std::fs::read(&plan).unwrap()).unwrap();
    assert_eq!(frozen.plan.requests.len(), 2);
    assert_eq!(frozen.tasks[0].transitive.len(), 3);
    let (exit, _) = cli(
        &["review", "assist", "batch", "submit", "--experimental"],
        &[("--plan", &plan), ("--job", &outputs.join("pair.json"))],
    );
    assert_eq!(exit, 0);
    let mut incomplete = frozen.clone();
    incomplete.tasks[0]
        .transitive
        .retain(|r| r.path != saccade_core::paths::portable(&after));
    assist::write(&plan, &incomplete).unwrap();
    let (exit, _) = cli(
        &["review", "assist", "batch", "submit", "--experimental"],
        &[
            ("--plan", &plan),
            ("--job", &outputs.join("incomplete.json")),
        ],
    );
    assert_ne!(exit, 0);
}
#[test]
#[ignore = "heavy: wave4-batch-routing"]
fn wave4_optional_routing_replay_abstains_and_mirrors_mcp_without_visual_calls() {
    let (_temp, inputs, outputs, image) = fixture();
    let prepared = outputs.join("prepared");
    prepare(&image, &prepared);
    let envelope: Envelope =
        assist::decode(&std::fs::read(prepared.join("saccade-assist.v1.json")).unwrap()).unwrap();
    let requests: Value =
        assist::decode(&std::fs::read(prepared.join("requests.json")).unwrap()).unwrap();
    let visual: CacheKey =
        serde_json::from_value(requests["requests"][0]["cache_key"].clone()).unwrap();
    let condition = assist::catalog::Condition::LabelVisible {
        label: "Continuer".into(),
    };
    let (key, _payload) = assist::routing::prepare(
        &envelope.scope,
        &envelope.identity,
        Some(&condition),
        JEV,
        visual.api_config_hash,
    )
    .unwrap();
    let response =
        serde_json::to_vec(&json!({"model":JEV,"answers":{"q":{"choice":"insufficient"}}}))
            .unwrap();
    let record = Cached {
        provenance: Provenance {
            provider: "jev".into(),
            requested_model: JEV.into(),
            returned_model: JEV.into(),
            returned_revision: JEV.into(),
            prompt_hash: key.prompt_hash.clone(),
            encoder_version: key.encoder_version.clone(),
            sampling_settings: key.settings.clone(),
            request_hash: key.payload_hash.clone(),
            response_hash: saccade_core::evidence::canonical::Digest::of_bytes(&response),
            order: key.order.clone(),
            usage: Usage::default(),
            cost_usd: None,
            cost_basis: "fixture only".into(),
            cache_status: "miss".into(),
            started_ms: 1,
            finished_ms: 2,
            elapsed_ms: 1,
        },
        key,
        response,
    };
    let replay = inputs.join("routing.json");
    assist::write(
        &replay,
        &json!({"schema":OBSERVATIONS_SCHEMA,"identity":envelope.identity,"records":[record]}),
    )
    .unwrap();
    let out = outputs.join("routed");
    let (exit, result) = cli(
        &[
            "review",
            "check-ui",
            "Continuer",
            "--box",
            "0,0,20,20",
            "--experimental",
            "--offline",
            "--jev-routing",
            "--gemini-revision",
            "r1",
        ],
        &[("--image", &image), ("--out", &out), ("--replay", &replay)],
    );
    assert_eq!(exit, 0);
    assert_eq!(result["data"]["outcome"], "unverifiable");
    assert_eq!(result["counts"]["provider_stages"], 1);
    let mirrored = mcp(
        &inputs,
        &outputs,
        json!({"operation":"check-ui","artifact":image,"out":outputs.join("routed-mcp"),"experimental":true,"offline":true,"jev_routing":true,"gemini_revision":"r1","replay":replay,"condition":"Continuer","box":[0,0,20,20]}),
    );
    assert_eq!(
        mirrored["result"]["structuredContent"]["data"],
        result["data"]
    );
    let requests: Value =
        assist::decode(&std::fs::read(out.join("requests.json")).unwrap()).unwrap();
    assert_eq!(requests["requests"].as_array().unwrap().len(), 1);
    assert_eq!(requests["requests"][0]["cache_key"]["order"], "route");
}
