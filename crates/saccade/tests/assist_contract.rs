//! Coordinator-only offline CLI/MCP end-to-end gates; no provider calls.
#![cfg(all(feature = "assist", feature = "mcp"))]
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_saccade")
}
#[test]
#[ignore = "heavy: wave4-cli"]
fn wave4_cli_mcp_offline_scope_and_authority_agree() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = temp.path().join("inputs");
    let outputs = temp.path().join("outputs");
    std::fs::create_dir_all(&inputs).unwrap();
    std::fs::create_dir_all(&outputs).unwrap();
    let image = inputs.join("capture.png");
    image::RgbImage::from_pixel(20, 20, image::Rgb([128, 128, 128]))
        .save(&image)
        .unwrap();
    let cli_out = outputs.join("cli");
    let output = Command::new(binary())
        .args(["review", "check-ui", "Continuer", "--image"])
        .arg(&image)
        .args([
            "--box",
            "0,0,20,20",
            "--experimental",
            "--offline",
            "--route",
            "rules",
            "--out",
        ])
        .arg(&cli_out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(cli["data"]["outcome"], "unverifiable");
    let mut child = Command::new(binary())
        .args(["mcp", "--root"])
        .arg(&inputs)
        .arg("--out-root")
        .arg(&outputs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let call = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_review","arguments":{"operation":"check-ui","artifact":image,"out":outputs.join("mcp"),"experimental":true,"offline":true,"route":"rules","condition":"Continuer","box":[0,0,20,20]}}});
    writeln!(child.stdin.as_mut().unwrap(), "{call}").unwrap();
    drop(child.stdin.take());
    let result = child.wait_with_output().unwrap();
    let reply: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(reply["result"]["structuredContent"]["data"], cli["data"]);
    let sidecar: Value =
        serde_json::from_slice(&std::fs::read(cli_out.join("saccade-assist.v1.json")).unwrap())
            .unwrap();
    assert_eq!(
        sidecar["schema"],
        saccade_core::report_links::linked_schema("saccade-assist.v1")
    );
    assert!(sidecar.get("approve").is_none());
    let escaped = std::fs::read_to_string(cli_out.join("index.html")).unwrap();
    assert!(escaped.contains("Experimental AI advice"));
}
#[test]
#[ignore = "heavy: wave4-cli"]
fn wave4_experimental_flag_and_invalid_inputs_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let image = temp.path().join("capture.png");
    image::RgbImage::from_pixel(20, 20, image::Rgb([0, 0, 0]))
        .save(&image)
        .unwrap();
    for flags in [vec![], vec!["--experimental", "--box", "19,0,2,10"]] {
        let mut cmd = Command::new(binary());
        cmd.args(["review", "check-ui", "Saved", "--image"])
            .arg(&image)
            .args(["--box", "0,0,20,20", "--offline", "--out"])
            .arg(temp.path().join("advice"))
            .arg("--json");
        if !flags.is_empty() {
            cmd = Command::new(binary());
            cmd.args(["review", "check-ui", "Saved", "--image"])
                .arg(&image)
                .args(["--offline", "--out"])
                .arg(temp.path().join("advice"))
                .arg("--json");
        }
        let output = cmd.args(flags).output().unwrap();
        assert!(!output.status.success());
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["execution"], "error");
    }
}
#[test]
#[ignore = "heavy: wave4-cli"]
fn wave4_report_workflows_preserve_verdict_and_mirror_mcp() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = temp.path().join("inputs");
    let outputs = temp.path().join("outputs");
    std::fs::create_dir_all(&inputs).unwrap();
    std::fs::create_dir_all(&outputs).unwrap();
    let before = inputs.join("before.png");
    let after = inputs.join("after.png");
    for (path, red) in [(&before, 30), (&after, 170)] {
        image::RgbImage::from_pixel(20, 20, image::Rgb([red, 60, 90]))
            .save(path)
            .unwrap();
    }
    let report_dir = inputs.join("report");
    let compared = Command::new(binary())
        .args(["compare"])
        .arg(&before)
        .arg(&after)
        .arg("--out")
        .arg(&report_dir)
        .output()
        .unwrap();
    assert!(
        matches!(compared.status.code(), Some(0 | 1)),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let report = report_dir.join("saccade-report.v1.json");
    let original = std::fs::read(&report).unwrap();
    let original_report: Value = serde_json::from_slice(&original).unwrap();
    for operation in ["explain", "audit-mask"] {
        let cli_out = outputs.join(format!("cli-{operation}"));
        let output = Command::new(binary())
            .args(["review", operation, "--report"])
            .arg(&report)
            .args([
                "--experimental",
                "--offline",
                "--route",
                "rules",
                "--json",
                "--out",
            ])
            .arg(&cli_out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            cli["data"]["deterministic_verdict"],
            original_report["combined_verdict"]
                .as_str()
                .unwrap_or("regression")
        );
        assert_eq!(std::fs::read(&report).unwrap(), original);
        let mut child = Command::new(binary())
            .args(["mcp", "--root"])
            .arg(&inputs)
            .arg("--out-root")
            .arg(&outputs)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let call = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"saccade_review","arguments":{"operation":operation,"artifact":report,"out":outputs.join(format!("mcp-{operation}")),"experimental":true,"offline":true,"route":"rules"}}});
        writeln!(child.stdin.as_mut().unwrap(), "{call}").unwrap();
        drop(child.stdin.take());
        let reply: Value =
            serde_json::from_slice(&child.wait_with_output().unwrap().stdout).unwrap();
        assert_eq!(reply["result"]["structuredContent"]["data"], cli["data"]);
        for (file, schema) in [
            ("requests.json", "saccade-assist-requests.v1"),
            ("observations.json", "saccade-assist-observations.v1"),
        ] {
            let artifact: Value =
                serde_json::from_slice(&std::fs::read(cli_out.join(file)).unwrap()).unwrap();
            assert_eq!(
                artifact["schema"],
                saccade_core::report_links::linked_schema(schema)
            );
        }
        if operation == "audit-mask" {
            let audit: Value =
                serde_json::from_slice(&std::fs::read(cli_out.join("mask-audit.json")).unwrap())
                    .unwrap();
            assert_eq!(audit["individual"]["availability"], "unavailable");
        }
    }
}
