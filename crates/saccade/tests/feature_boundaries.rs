//! Compiled capabilities and errors in reduced CLI builds.
#![allow(clippy::unwrap_used, missing_docs)]

use serde_json::Value;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

#[test]
fn capabilities_match_the_compiled_command_surface() {
    let output = Command::new(BIN)
        .args(["inspect", "capabilities", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let features = value["features"].as_array().unwrap();
    let operations = value["operations"].as_array().unwrap();
    for (feature, enabled, command) in [
        ("graphics", cfg!(feature = "graphics"), "sequence"),
        ("ai", cfg!(feature = "ai"), "review"),
        ("workbench", cfg!(feature = "workbench"), "serve"),
        ("mcp", cfg!(feature = "mcp"), "mcp"),
        (
            "evaluation",
            cfg!(feature = "evaluation"),
            "judge calibrate",
        ),
        ("prechecks", cfg!(feature = "prechecks"), "safety"),
    ] {
        assert_eq!(features.iter().any(|v| v == feature), enabled, "{feature}");
        assert_eq!(
            operations.iter().any(|v| v == command),
            enabled,
            "{command}"
        );
        if enabled {
            let help = Command::new(BIN)
                .args(command.split_whitespace())
                .arg("--help")
                .output()
                .unwrap();
            assert!(
                help.status.success(),
                "{command}: {}",
                String::from_utf8_lossy(&help.stderr)
            );
        }
    }
    assert_eq!(
        features.iter().any(|v| v == "parallel"),
        cfg!(feature = "parallel")
    );
    assert_eq!(
        features.iter().any(|v| v == "schema"),
        cfg!(feature = "schema")
    );
    for command in [
        "compare",
        "identity",
        "noise",
        "view",
        "approve",
        "inspect capabilities",
    ] {
        assert!(operations.iter().any(|v| v == command), "{command}");
    }
    assert!(!operations.iter().any(|v| v == "watch"));
}

#[test]
fn disabled_cli_operations_return_feature_unavailable() {
    for (enabled, feature, command) in [
        (cfg!(feature = "graphics"), "graphics", "sequence"),
        (cfg!(feature = "ai"), "ai", "review"),
        (cfg!(feature = "workbench"), "workbench", "serve"),
        (cfg!(feature = "mcp"), "mcp", "mcp"),
        (
            cfg!(feature = "evaluation"),
            "evaluation",
            "judge calibrate",
        ),
        (cfg!(feature = "prechecks"), "prechecks", "safety"),
    ] {
        if enabled || (feature == "evaluation" && !cfg!(feature = "ai")) {
            continue;
        }
        let output = Command::new(BIN)
            .args(command.split_whitespace())
            .arg("--json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{command}");
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], "feature_unavailable", "{command}: {error}");
        assert!(
            error["message"].as_str().unwrap().contains(feature),
            "{error}"
        );
    }
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_advertises_only_compiled_tools_and_rejects_missing_computation() {
    use serde_json::json;
    use std::io::Write;
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})
    )
    .unwrap();
    for (id, tool) in [
        (2, "saccade_sequence"),
        (3, "saccade_review"),
        (4, "saccade_ask_human"),
        (5, "saccade_safety"),
        (6, "saccade_judge_calibrate"),
    ] {
        // Empty arguments make any mistakenly advertised disabled tool fail validation.
        writeln!(input,"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":tool,"arguments":{}}})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    assert!(!tools.iter().any(|t| t["name"] == "saccade_watch_status"));
    for (id, feature, tool, enabled) in [
        (
            2,
            "graphics",
            "saccade_sequence",
            cfg!(feature = "graphics"),
        ),
        (3, "ai", "saccade_review", cfg!(feature = "ai")),
        (
            4,
            "workbench",
            "saccade_ask_human",
            cfg!(feature = "workbench"),
        ),
        (
            5,
            "prechecks",
            "saccade_safety",
            cfg!(feature = "prechecks"),
        ),
        (
            6,
            "evaluation",
            "saccade_judge_calibrate",
            cfg!(feature = "evaluation"),
        ),
    ] {
        assert_eq!(tools.iter().any(|t| t["name"] == tool), enabled, "{tool}");
        if !enabled {
            let reply = replies.iter().find(|v| v["id"] == id).unwrap();
            assert_eq!(
                reply["result"]["structuredContent"]["code"], "feature_unavailable",
                "{reply}"
            );
            assert!(
                reply["result"]["structuredContent"]["message"]
                    .as_str()
                    .unwrap()
                    .contains(feature)
            );
        }
    }
}
