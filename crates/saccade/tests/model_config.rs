//! One model/runtime configuration shared by the CLI and MCP; compatibility readers for old flags.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Isolated environment: no ambient model settings leak in from the machine.
fn command(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_saccade"));
    c.env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_CACHE_HOME", home.join("cache"));
    c
}

fn as_path(v: &Value) -> std::path::PathBuf {
    std::path::PathBuf::from(v.as_str().unwrap())
}

fn json_of(out: &Output) -> Value {
    assert!(out.status.success(), "{out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn config_file_env_and_legacy_env_resolve_in_order() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("config/saccade");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "[models]\ndir = \"from-file\"\n").unwrap();
    let show = |env: &[(&str, &Path)]| {
        let mut c = command(home.path());
        c.args(["models", "config", "--json"]);
        for (k, v) in env {
            c.env(k, v);
        }
        json_of(&c.output().unwrap())
    };
    let v = show(&[]);
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get("saccade-model-config.v1").unwrap())
            .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&v)
        .unwrap();
    assert_eq!(v["schema"], "saccade-model-config.v1");
    assert_eq!(as_path(&v["dir"]["value"]), dir.join("from-file"));
    assert_eq!(v["dir"]["source"]["kind"], "config_file");
    let legacy = home.path().join("legacy");
    let v = show(&[("SACCADE_MODEL_CACHE", &legacy)]);
    assert_eq!(as_path(&v["dir"]["value"]), legacy);
    assert_eq!(v["dir"]["source"]["name"], "SACCADE_MODEL_CACHE");
    let new = home.path().join("new");
    let v = show(&[
        ("SACCADE_MODEL_CACHE", &legacy),
        ("SACCADE_MODELS_DIR", &new),
    ]);
    assert_eq!(as_path(&v["dir"]["value"]), new);
}

#[cfg(feature = "mcp")]
#[test]
fn cli_and_mcp_resolve_the_same_pinned_cache_and_registry() {
    use std::io::Write;
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("operator-cache");
    std::fs::create_dir(&cache).unwrap();
    let cli = json_of(
        &command(home.path())
            .env("SACCADE_MODELS_DIR", &cache)
            .args(["models", "list", "--json"])
            .output()
            .unwrap(),
    );
    let root = home.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let mut child = command(home.path())
        .env("SACCADE_MODELS_DIR", &cache)
        .args(["mcp", "--root"])
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in [json!({"operation":"models_list"})].iter().enumerate() {
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":"saccade_inspect","arguments":args}})).unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let reply: Value =
        serde_json::from_slice(out.stdout.split(|b| *b == b'\n').next().unwrap()).unwrap();
    assert_ne!(reply["result"]["isError"], true, "{reply}");
    assert_eq!(reply["result"]["structuredContent"], cli);
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_requests_cannot_choose_a_location_or_trigger_a_download() {
    use std::io::Write;
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("root");
    let cache = home.path().join("operator-cache");
    let other = root.join("request-cache");
    for d in [&root, &cache, &other] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(root.join("anything.json"), b"{}").unwrap();
    let calls = [
        json!({"operation":"models_list","cache":other}),
        json!({"operation":"models_list","registry":root.join("anything.json")}),
        json!({"operation":"models_pull","id":"yunet-2026may"}),
        json!({"operation":"models_list","allow_download":true}),
    ];
    let mut child = command(home.path())
        .env("SACCADE_MODELS_DIR", &cache)
        .args(["mcp", "--root"])
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in calls.iter().enumerate() {
        let tool = if args["operation"] == "models_pull" {
            "saccade_measure"
        } else {
            "saccade_inspect"
        };
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":tool,"arguments":args}})).unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let replies: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 4);
    assert!(
        replies.iter().all(|r| r["result"]["isError"] == true),
        "{replies:?}"
    );
    assert!(
        replies[0]
            .to_string()
            .contains("model_location_not_request_controlled")
    );
    assert!(
        replies[1]
            .to_string()
            .contains("model_location_not_request_controlled")
    );
    assert!(
        replies[2]
            .to_string()
            .contains("cannot authorize model downloads")
    );
    // Nothing was provisioned by any of the requests.
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
}

#[test]
fn old_flags_keep_working_and_print_a_deprecation_notice() {
    let home = tempfile::tempdir().unwrap();
    let cache = home.path().join("flag-cache");
    std::fs::create_dir(&cache).unwrap();
    let out = command(home.path())
        .args(["models", "list", "--json", "--cache"])
        .arg(&cache)
        .output()
        .unwrap();
    assert_eq!(
        json_of(&out)["schema"],
        saccade_core::report_links::linked_schema("saccade-model-status.v1")
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("deprecated") && stderr.contains("SACCADE_MODELS_DIR"),
        "{stderr}"
    );
    assert!(
        command(home.path())
            .args(["models", "list", "--json"])
            .output()
            .unwrap()
            .stderr
            .is_empty()
    );

    let a = home.path().join("a.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([9; 3]))
        .save(&a)
        .unwrap();
    let out = command(home.path())
        .args(["text"])
        .arg(&a)
        .arg(&a)
        .arg("--download-model")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("deprecated: --download-model") && stderr.contains("saccade models pull"),
        "{stderr}"
    );
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_embedding_model_path_is_refused_in_favour_of_registry_ids() {
    use std::io::Write;
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("root");
    let out = home.path().join("out");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let img = root.join("a.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([9; 3]))
        .save(&img)
        .unwrap();
    std::fs::write(root.join("model.json"), b"{}").unwrap();
    std::fs::create_dir(root.join("imgs")).unwrap();
    let calls = [
        json!({"operation":"compare_question","question":"same-content","reference":img,"capture":img,"model":root.join("model.json"),"out":out.join("q")}),
        json!({"operation":"compare_question","question":"same-content","reference":img,"capture":img,"model":"not-in-registry","out":out.join("r")}),
        json!({"operation":"embedding_export_inputs","dir":root.join("imgs"),"model":root.join("model.json"),"out":out.join("e")}),
        json!({"operation":"index_build","dir":root.join("imgs"),"model":root.join("model.json"),"out":out.join("i")}),
    ];
    let mut child = command(home.path())
        .args(["mcp", "--root"])
        .arg(&root)
        .arg("--out-root")
        .arg(&out)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in calls.iter().enumerate() {
        writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":id+1,"method":"tools/call","params":{"name":"saccade_general","arguments":args}})).unwrap();
    }
    drop(stdin);
    let result = child.wait_with_output().unwrap();
    let replies: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 4);
    for r in &replies {
        assert_eq!(r["result"]["isError"], true, "{r}");
        assert!(
            r.to_string()
                .contains("model_location_not_request_controlled"),
            "{r}"
        );
    }
}
