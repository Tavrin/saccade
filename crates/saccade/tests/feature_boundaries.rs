//! Feature capabilities reflect compiled local operations.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::Value;
use std::process::Command;
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
#[test]
fn capabilities_match_compiled_operations() {
    let output = Command::new(BIN)
        .args(["inspect", "capabilities", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let features = value["data"]["features"].as_array().unwrap();
    let operations = value["data"]["operations"].as_array().unwrap();
    for (feature, enabled, operation) in [
        (
            "graphics",
            cfg!(feature = "graphics"),
            "experiment sequence",
        ),
        ("workbench", cfg!(feature = "workbench"), "serve"),
        ("mcp", cfg!(feature = "mcp"), "mcp"),
        (
            "prechecks",
            cfg!(feature = "prechecks"),
            "experiment safety",
        ),
    ] {
        assert_eq!(features.iter().any(|v| v == feature), enabled);
        assert_eq!(operations.iter().any(|v| v == operation), enabled);
    }
    for operation in [
        "compare",
        "identity",
        "noise",
        "view",
        "approve",
        "review",
        "inspect capabilities",
    ] {
        assert!(operations.iter().any(|v| v == operation));
    }
}
#[test]
fn disabled_computation_returns_targeted_feature_error() {
    for (enabled, feature, command) in [
        (
            cfg!(feature = "graphics"),
            "graphics",
            vec!["experiment", "sequence"],
        ),
        (cfg!(feature = "workbench"), "workbench", vec!["serve"]),
        (cfg!(feature = "mcp"), "mcp", vec!["mcp"]),
        (
            cfg!(feature = "prechecks"),
            "prechecks",
            vec!["experiment", "safety"],
        ),
    ] {
        if enabled {
            continue;
        }
        let output = Command::new(BIN)
            .args(command)
            .arg("--json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["errors"][0]["code"], "feature_unavailable");
        assert!(
            error["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains(feature)
        );
    }
}
#[cfg(feature = "mcp")]
#[test]
fn local_tools_list_exactly_the_operations_this_binary_implements() {
    use serde_json::json;
    use std::io::Write;
    use std::process::Stdio;
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})
    )
    .unwrap();
    child.stdin.take();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    let tools = reply["result"]["tools"].as_array().unwrap();
    assert_eq!(
        tools.len(),
        (if cfg!(feature = "ai") { 7 } else { 6 }) + usize::from(cfg!(feature = "products"))
    );
    let general = tools
        .iter()
        .find(|tool| tool["name"] == "saccade_general")
        .unwrap();
    let general_operations = general["inputSchema"]["oneOf"].as_array().unwrap();
    for operation in [
        "registered_compare",
        "hash",
        "dedupe",
        "text",
        "assess",
        "inspect_image",
        "capabilities",
        "compare_question",
    ] {
        assert!(
            general_operations
                .iter()
                .any(|variant| variant["properties"]["operation"]["const"] == operation),
            "missing general operation {operation}"
        );
    }
    assert_eq!(
        tools.iter().any(|tool| tool["name"] == "saccade_products"),
        cfg!(feature = "products")
    );
    let measure = tools
        .iter()
        .find(|t| t["name"] == "saccade_measure")
        .unwrap();
    let operations = measure["inputSchema"]["oneOf"].as_array().unwrap();
    for (operation, enabled) in [
        ("compare", true),
        ("identity", true),
        ("noise", true),
        ("sequence", cfg!(feature = "graphics")),
        ("rank", cfg!(feature = "graphics")),
        ("bisect", cfg!(feature = "graphics")),
        ("safety", cfg!(feature = "prechecks")),
    ] {
        assert_eq!(
            operations
                .iter()
                .any(|v| v["properties"]["operation"]["const"] == operation),
            enabled,
            "{operation}"
        );
    }
}
