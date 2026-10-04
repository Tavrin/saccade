//! R5 local command, bounded output and transport permission contracts.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]
use image::{Rgb, RgbImage};
use saccade_core::evidence::action::ResultEnvelope;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
fn image(dir: &Path, name: &str, shade: u8) {
    std::fs::create_dir_all(dir).unwrap();
    RgbImage::from_pixel(16, 16, Rgb([shade; 3]))
        .save(dir.join(name))
        .unwrap();
}
fn json_output(output: Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
fn call(id: u32, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}
fn mcp(roots: &[&Path], out: Option<&Path>, extra: &[&str], messages: &[Value]) -> Vec<Value> {
    let mut command = Command::new(BIN);
    command.arg("mcp");
    for root in roots {
        command.arg("--root").arg(root);
    }
    if let Some(out) = out {
        command.arg("--out-root").arg(out);
    }
    command.args(extra);
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for message in messages {
        writeln!(stdin, "{message}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn report(root: &Path, n: usize) -> PathBuf {
    for i in 0..n {
        let name = format!("entry-{i:03}.png");
        image(&root.join("base"), &name, 40);
        image(&root.join("capture"), &name, 200);
    }
    let output = Command::new(BIN)
        .arg("compare")
        .args([root.join("base"), root.join("capture")])
        .arg("--out")
        .arg(root.join("report"))
        .arg("--json")
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let result = json_output(output);
    let typed: ResultEnvelope = serde_json::from_value(result.clone()).unwrap();
    typed.validate().unwrap();
    assert!(result["entries"].as_array().unwrap().len() <= 5);
    assert!(serde_json::to_vec(&result).unwrap().len() <= 4096);
    root.join("report/saccade-report.v1.json")
}
#[test]
fn help_lists_active_commands_and_watch_alias_stays_hidden() {
    let value = json_output(
        Command::new(BIN)
            .args(["inspect", "capabilities", "--json"])
            .output()
            .unwrap(),
    );
    let operations = value["data"]["operations"].as_array().unwrap();
    let top = operations
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.contains(' '))
        .collect::<Vec<_>>();
    assert_eq!(top.len(), 13, "{top:?}");
    for name in [
        "init",
        "demo",
        "compare",
        "identity",
        "noise",
        "inspect",
        "view",
        "review",
        "approve",
        "experiment",
        "serve",
        "mcp",
        "doctor",
    ] {
        assert!(top.contains(&name));
    }
    for name in [
        "inspect evidence",
        "inspect export",
        "inspect config",
        "inspect capabilities",
        "review request",
        "review propose",
        "review ask",
        "review eval",
    ] {
        assert!(operations.iter().any(|s| s == name));
    }
    let output = Command::new(BIN)
        .args(["watch", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = json_output(output);
    assert_eq!(value["errors"][0]["code"], "usage");
    for flag in ["--json=full", "--json=decision", "--compat"] {
        let output = Command::new(BIN).args(["identity", flag]).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
#[test]
fn pagination_and_actions_are_bounded_and_bound_to_content_and_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let file = report(tmp.path(), 25);
    let read = |cursor: Option<&str>, limit: &str| {
        let mut command = Command::new(BIN);
        command
            .arg("inspect")
            .arg(&file)
            .args(["--json", "--limit", limit]);
        if let Some(cursor) = cursor {
            command.args(["--cursor", cursor]);
        }
        command.output().unwrap()
    };
    let page = json_output(read(None, "10"));
    assert_eq!(page["entries"].as_array().unwrap().len(), 10);
    assert_eq!(page["page"]["omitted"], 15);
    assert!(serde_json::to_vec(&page).unwrap().len() <= 8192);
    let cursor = page["page"]["next_cursor"].as_str().unwrap();
    let second = json_output(read(Some(cursor), "10"));
    assert_eq!(second["entries"][0]["entry_id"], "entry-010.png");
    assert_eq!(second["page"]["omitted"], 5);
    assert_eq!(
        json_output(read(Some(cursor), "5"))["errors"][0]["code"],
        "stale_cursor"
    );
    let output = Command::new(BIN)
        .arg("identity")
        .args([tmp.path().join("base"), tmp.path().join("capture")])
        .args(["--out", "proof", "--json"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let result = json_output(output);
    let continuation = result["page"]["next_cursor"].as_str().unwrap();
    let continued = json_output(
        Command::new(BIN)
            .arg("inspect")
            .arg(tmp.path().join("proof/saccade-report.v1.json"))
            .args(["--json", "--cursor", continuation])
            .output()
            .unwrap(),
    );
    assert_eq!(continued["entries"][0]["entry_id"], "entry-005.png");
    assert_eq!(continued["page"]["omitted"], 10);
    let actions = result["next_actions"].as_array().unwrap();
    assert!(actions.len() <= 3);
    let action = actions[0].clone();
    assert!(action["cli_argv"].is_array());
    assert_eq!(action["tool"], "saccade_inspect");
    let artifact = tmp.path().join("proof/saccade-report.v1.json");
    let replies = mcp(
        &[tmp.path()],
        None,
        &[],
        &[
            call(1, "saccade_inspect", {
                let mut arguments = action["arguments"].clone();
                arguments["expected_case_id"] = action["expected_case_id"].clone();
                // The CLI reference is relative to its invocation directory.
                arguments["artifact"]["path"] = json!(artifact);
                arguments
            }),
            call(
                2,
                "saccade_inspect",
                json!({"operation":"summary","artifact":artifact,"expected_case_id":"0000000000000000000000000000000000000000000000000000000000000000"}),
            ),
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    assert_eq!(
        replies[1]["result"]["structuredContent"]["errors"][0]["code"],
        "stale_action"
    );
    image(&tmp.path().join("capture"), "entry-000.png", 50);
    let replies = mcp(
        &[tmp.path()],
        None,
        &[],
        &[call(
            1,
            "saccade_inspect",
            json!({"operation":"summary","artifact":artifact,"expected_case_id":action["expected_case_id"]}),
        )],
    );
    assert_eq!(
        replies[0]["result"]["structuredContent"]["errors"][0]["code"],
        "stale_evidence"
    );
}

#[test]
fn entry_and_validity_inspection_are_actionable() {
    let tmp = tempfile::tempdir().unwrap();
    let file = report(tmp.path(), 2);
    let compare: Value = json_output(
        Command::new(BIN)
            .current_dir(tmp.path())
            .args(["compare", "base", "capture", "--out", "report", "--json"])
            .output()
            .unwrap(),
    );
    let worst = &compare["worst"];
    assert_eq!(worst["entry"], "entry-000.png");
    assert!(worst["value"].as_f64().unwrap() > worst["threshold"].as_f64().unwrap());
    let entry = json_output(
        Command::new(BIN)
            .current_dir(tmp.path())
            .arg("inspect")
            .arg(&file)
            .args(["--entry", "entry-000.png", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(entry["measurement"], compare["measurement"]);
    assert!(
        (entry["entries"][0]["value"].as_f64().unwrap() - worst["value"].as_f64().unwrap()).abs()
            < 0.001
    );
    assert!(entry["entries"][0]["explanation"].is_string());
    let action = &entry["next_actions"][0];
    assert_eq!(action["cwd"], tmp.path().to_str().unwrap());
    let argv = action["cli_argv"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        Command::new(BIN)
            .current_dir(tmp.path())
            .args(argv)
            .status()
            .unwrap()
            .success()
    );
    let reasons = json_output(
        Command::new(BIN)
            .arg("inspect")
            .arg(&file)
            .args(["--validity-reasons", "--limit", "2", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(reasons["entries"].as_array().unwrap().len(), 2);
    assert!(reasons["page"]["next_cursor"].is_string());
}

#[test]
fn report_pixel_payload_reuses_copied_image_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let file = report(tmp.path(), 1);
    let dir = file.parent().unwrap();
    let pixels = std::fs::read_to_string(dir.join("report-pixels.js")).unwrap();
    assert!(!pixels.contains("data:image/"));
    let report: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    for key in ["baseline", "capture", "heatmap"] {
        if let Some(path) = report["entries"][0]["paths"][key].as_str() {
            assert!(dir.join(path).is_file(), "missing {path}");
            assert!(pixels.contains(path));
        }
    }
}
#[test]
fn roots_are_read_only_and_output_permissions_never_grant_capture_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let other = tmp.path().join("other");
    let out = tmp.path().join("output");
    image(&root.join("base"), "a.png", 40);
    image(&other, "a.png", 40);
    let measure = |out: Value| {
        call(
            1,
            "saccade_measure",
            json!({"operation":"identity","parent_dir":root.join("base"),"candidate_dir":other,"out":out}),
        )
    };
    let replies = mcp(
        &[&root, &other],
        Some(&out),
        &[],
        &[
            measure(json!("report")),
            measure(json!(root.join("write"))),
            measure(json!("../escape")),
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    for reply in &replies[1..] {
        assert_eq!(
            reply["result"]["structuredContent"]["errors"][0]["code"],
            "unsafe_path"
        );
    }
    assert!(!root.join("write").exists());
    assert!(out.join("report/.saccade-run").is_file());
    let replies = mcp(&[&root, &other], None, &[], &[measure(json!("report"))]);
    assert_eq!(replies[0]["result"]["isError"], true);
    let before = std::fs::read(root.join("base/a.png")).unwrap();
    let replies = mcp(
        &[&root],
        Some(&out),
        &[],
        &[
            call(1, "saccade_approve", json!({})),
            call(
                2,
                "saccade_measure",
                json!({"operation":"approve","out":"plan"}),
            ),
        ],
    );
    assert!(replies[0]["error"].is_object());
    assert_eq!(replies[1]["result"]["isError"], true);
    assert_eq!(before, std::fs::read(root.join("base/a.png")).unwrap());
}
#[cfg(unix)]
#[test]
fn aliases_authorize_registered_storage_but_not_direct_roots_or_escape_outputs() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let storage = tmp.path().join("storage");
    let out = tmp.path().join("out");
    std::fs::create_dir_all(&root).unwrap();
    image(&storage, "a.png", 40);
    symlink(&storage, root.join("alias")).unwrap();
    let message = call(
        1,
        "saccade_measure",
        json!({"operation":"identity","parent_dir":root.join("alias"),"candidate_dir":root.join("alias"),"out":"report"}),
    );
    let reply = mcp(&[&root], Some(&out), &[], std::slice::from_ref(&message));
    assert_eq!(reply[0]["result"]["isError"], true);
    let extra = ["--symlink-target", storage.to_str().unwrap()];
    let reply = mcp(
        &[&root],
        Some(&out),
        &extra,
        &[
            message,
            call(
                2,
                "saccade_measure",
                json!({"operation":"identity","parent_dir":storage,"candidate_dir":storage,"out":"direct"}),
            ),
        ],
    );
    assert_eq!(reply[0]["result"]["isError"], false, "{}", reply[0]);
    assert_eq!(reply[1]["result"]["isError"], true);
    std::fs::create_dir_all(&out).unwrap();
    symlink(&root, out.join("escape")).unwrap();
    let reply = mcp(
        &[&root],
        Some(&out),
        &extra,
        &[call(
            1,
            "saccade_evidence",
            json!({"operation":"context","artifact":out.join("report/saccade-report.v1.json"),"out":"escape/new"}),
        )],
    );
    assert_eq!(reply[0]["result"]["isError"], true);
    assert!(!root.join("new").exists());
    symlink(tmp.path().join("secret.json"), root.join("base-card.json")).unwrap();
    std::fs::write(tmp.path().join("secret.json"), b"{}").unwrap();
    std::fs::write(
        root.join("saccade.toml"),
        "[[mask]]\nimage='base-card.json'\n",
    )
    .unwrap();
    let reply = mcp(
        &[&root],
        Some(&out),
        &extra,
        &[call(
            1,
            "saccade_measure",
            json!({"operation":"compare","baseline_dir":root.join("alias"),"capture_dir":root.join("alias"),"out":"masked"}),
        )],
    );
    assert_eq!(
        reply[0]["result"]["structuredContent"]["errors"][0]["code"],
        "unsafe_path"
    );
}
#[test]
fn local_tools_and_preview_never_authorize_network_and_images_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let out = tmp.path().join("out");
    image(&root.join("base"), "a.png", 40);
    image(&root.join("capture"), "a.png", 200);
    let replies = mcp(
        &[&root],
        Some(&out),
        &[],
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
            call(
                2,
                "saccade_measure",
                json!({"operation":"compare","baseline_dir":"base","capture_dir":"capture","out":"report"}),
            ),
            call(
                3,
                "saccade_measure",
                json!({"operation":"compare","baseline_dir":"base","capture_dir":"capture","out":"images","include_images":true}),
            ),
            call(
                4,
                "saccade_review",
                json!({"operation":"run","artifact":"missing.json","out":"review.json"}),
            ),
            call(
                5,
                "saccade_measure",
                json!({"operation":"compare","baseline_dir":"base","capture_dir":"capture","out":"forged","provider_url":"http://127.0.0.1:1"}),
            ),
        ],
    );
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), if cfg!(feature = "ai") { 6 } else { 5 });
    for tool in tools {
        assert!(tool["inputSchema"]["oneOf"].is_array());
        assert!(!matches!(tool["name"].as_str(), Some("saccade_approve")));
    }
    assert_eq!(replies[1]["result"]["isError"], false);
    assert_eq!(
        replies[1]["result"]["structuredContent"]["measurement"],
        "regression"
    );
    assert_eq!(replies[1]["result"]["content"].as_array().unwrap().len(), 1);
    assert!(
        replies[2]["result"]["content"].as_array().unwrap().len() > 1,
        "{}",
        replies[2]
    );
    if cfg!(feature = "ai") {
        assert_eq!(
            replies[3]["result"]["structuredContent"]["errors"][0]["code"],
            "network_authorization_required"
        );
    } else {
        assert!(replies[3]["error"].is_object());
    }
    assert_eq!(replies[4]["result"]["isError"], true);
    let report = out.join("report/saccade-report.v1.json");
    let preview = json_output(
        Command::new(BIN)
            .arg("review")
            .arg(&report)
            .arg("--json")
            .env("JEV_API_KEY", "synthetic-never-send")
            .env("GEMINI_API_KEY", "synthetic-never-send")
            .output()
            .unwrap(),
    );
    assert_eq!(preview["counts"]["dispatched_calls"], 0);
    let denied = json_output(
        Command::new(BIN)
            .arg("review")
            .arg(&report)
            .args(["--run", "--budget-calls", "1", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        denied["errors"][0]["code"],
        if cfg!(feature = "ai") {
            "egress_denied"
        } else {
            "feature_unavailable"
        }
    );
}
