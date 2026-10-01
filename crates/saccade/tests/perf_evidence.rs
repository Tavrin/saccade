//! Generated evidence through the CLI, overview server and confined MCP.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .current_dir(root)
        .args(args)
        .env("XDG_CACHE_HOME", root.join("cache"))
        .output()
        .unwrap()
}
fn value(o: &Output) -> Value {
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}
fn save(root: &Path, name: &str, pass: f64, grey: u8) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbImage::from_pixel(32, 24, image::Rgb([grey, 90, 110]))
        .save(dir.join("scene.png"))
        .unwrap();
    std::fs::write(dir.join("saccade-perf.json"),json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":pass+2.0,"samples":5,"stat":"p50"},"terms":[{"id":"render","kind":"pass","value":pass},{"id":"gap","kind":"gap","value":2.0},{"id":"detail","kind":"scope","parent":"render","value":pass/2.0}],"counters":{"render":{"work":pass*100.0}}}).to_string()).unwrap();
    std::fs::write(
        dir.join("saccade-meta.json"),
        json!({"quality":if name=="image" {"low"} else {"high"}}).to_string(),
    )
    .unwrap();
    dir
}
fn fixture(root: &Path) {
    for (name, p, g) in [
        ("base", 8.0, 80),
        ("repeat", 8.1, 80),
        ("same", 8.05, 80),
        ("fast", 4.0, 80),
        ("image", 6.0, 150),
    ] {
        save(root, name, p, g);
    }
}
fn schema(name: &str, v: &Value) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(format!("{name}.schema.json"));
    let s: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&s).unwrap();
    let errors: Vec<_> = validator.iter_errors(v).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}
fn model(html: &str) -> Value {
    let data = html
        .split("id=\"saccade-data\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    serde_json::from_str(&data.replace("<\\/", "</").replace("\\u0021", "!")).unwrap()
}

#[test]
fn noise_ablation_compare_identity_markdown_and_explain_share_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    std::fs::write(root.join("perf-options.toml"),
        "perf_resolution_ms = 0.02\nperf_resolution_ticks = 4\nperf_min_delta_ms = 0.4\nperf_min_delta_pct = 1.2\n").unwrap();
    let configured = value(&run(
        root,
        &[
            "noise",
            "base",
            "repeat",
            "--config",
            "perf-options.toml",
            "--perf-resolution",
            "0.03",
            "--out",
            "configured.toml",
            "--json",
        ],
    ));
    schema("saccade-noise.v1", &configured);
    assert_eq!(configured["perf_noise"]["resolution_ms"], 0.03);
    assert_eq!(configured["perf_noise"]["resolution_ticks"], 4);
    assert_eq!(configured["perf_noise"]["min_delta_ms"], 0.4);
    assert_eq!(configured["perf_noise"]["min_delta_pct"], 1.2);
    let recalibrated = value(&run(
        root,
        &[
            "noise",
            "base",
            "repeat",
            "--config",
            "configured.toml",
            "--out",
            "recalibrated.toml",
            "--json",
        ],
    ));
    assert_eq!(recalibrated["perf_noise"], configured["perf_noise"]);
    let custom = value(&run(
        root,
        &[
            "ablate",
            "base",
            "fast",
            "--perf-noise",
            "configured.toml",
            "--perf-min-delta-ms",
            "10",
            "--out",
            "configured-ablation",
            "--json",
        ],
    ));
    assert_eq!(custom["arms"][0]["perf_diff"]["resolution_ms"], 0.03);
    assert_eq!(custom["arms"][0]["perf_diff"]["min_delta_ms"], 10.0);
    assert_eq!(custom["arms"][0]["flag"], "NO-EFFECT");
    let noise = value(&run(
        root,
        &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
    ));
    schema("saccade-noise.v1", &noise);
    assert!((noise["perf_noise"]["frame"].as_f64().unwrap() - 0.1).abs() < 1e-12);
    std::fs::write(root.join("floor.json"), noise.to_string()).unwrap();
    let a = value(&run(
        root,
        &[
            "ablate",
            "base",
            "same",
            "fast",
            "image",
            "--perf-noise",
            "floor.toml",
            "--out",
            "ablation",
            "--json",
        ],
    ));
    schema("saccade-ablate.v1", &a);
    assert_eq!(a["arms"][0]["flag"], "NO-EFFECT");
    assert_eq!(a["arms"][1]["flag"], "PERF-ONLY");
    assert_eq!(a["arms"][2]["flag"], "IMAGE-CHANGE");
    assert_eq!(a["arms"][2]["config_differs"], json!(["quality"]));
    assert!(!a["arms"][1]["top_deltas"].as_array().unwrap().is_empty());
    let html = std::fs::read_to_string(root.join("ablation/index.html")).unwrap();
    assert!(html.contains("saccadePerf.ablation"));
    assert!(html.contains("--surface"));
    for command in ["compare", "identity"] {
        let lean = value(&run(
            root,
            &[
                command,
                "base",
                "fast",
                "--perf-noise",
                "floor.json",
                "--out",
                "pair",
                "--json",
            ],
        ));
        schema("saccade-result.v1", &lean);
        assert!(
            lean["combined_verdict"]
                .as_str()
                .unwrap()
                .contains("beyond noise")
        );
        assert_eq!(lean["perf_diff"]["terms"].as_array().unwrap().len(), 3);
        let full: Value = serde_json::from_slice(
            &std::fs::read(root.join("pair/saccade-report.v1.json")).unwrap(),
        )
        .unwrap();
        schema("saccade-report.v1", &full);
        assert!(full["entries"][0].get("perf_diff").is_none());
        let markdown = run(
            root,
            &[
                "summary",
                "pair/saccade-report.v1.json",
                "--format",
                "markdown",
            ],
        );
        assert!(String::from_utf8_lossy(&markdown.stdout).contains("beyond noise"));
        let explain = run(
            root,
            &["explain", "pair/saccade-report.v1.json", "--out", "explain"],
        );
        assert!(explain.status.success());
        assert!(
            std::fs::read_to_string(root.join("explain/explain.md"))
                .unwrap()
                .contains("beyond noise")
        );
    }
    let unknown = value(&run(
        root,
        &["ablate", "base", "same", "--out", "unknown", "--json"],
    ));
    assert_eq!(unknown["arms"][0]["flag"], "INCONCLUSIVE");
    let original = std::fs::read(root.join("base/saccade-perf.json")).unwrap();
    let unsafe_out = run(root, &["ablate", "base", "fast", "--out", "base", "--json"]);
    assert_eq!(unsafe_out.status.code(), Some(2));
    assert_eq!(
        std::fs::read(root.join("base/saccade-perf.json")).unwrap(),
        original
    );
}

#[test]
fn runs_and_serve_sessions_refresh_performance_and_blind_views_withhold_it() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    value(&run(
        root,
        &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
    ));
    let r = value(&run(
        root,
        &[
            "runs",
            "base",
            "fast",
            "same",
            "--perf-noise",
            "floor.toml",
            "--out",
            "runs",
            "--json",
        ],
    ));
    schema("saccade-runs.v1", &r);
    assert_eq!(r["runs"][0]["flag"], "PERF-ONLY");
    assert_eq!(r["runs"][1]["flag"], "NO-EFFECT");
    assert!(
        std::fs::read_to_string(root.join("runs/index.html"))
            .unwrap()
            .contains("show-ablation")
    );
    assert!(
        run(
            root,
            &[
                "view",
                "base",
                "fast",
                "--perf-noise",
                "floor.toml",
                "--out",
                "view"
            ]
        )
        .status
        .success()
    );
    let v = model(&std::fs::read_to_string(root.join("view/index.html")).unwrap());
    assert_eq!(v["perf_diff"][0]["diff"]["frame"]["delta"], -4.0);
    assert!(
        run(root, &["view", "base", "fast", "--blind", "--out", "blind"])
            .status
            .success()
    );
    assert!(
        model(&std::fs::read_to_string(root.join("blind/index.html")).unwrap())
            .get("perf_diff")
            .is_none()
    );
    // Exercise the actual serve overview and session cache, without a GPU.
    let server_tmp = tempfile::tempdir().unwrap();
    let handle = saccade_core::serve::start({
        let mut o = saccade_core::serve::ServeOptions::new(root.to_path_buf());
        o.port = 0;
        o.cache_dir = server_tmp.path().join("server-cache");
        o.decisions_dir = server_tmp.path().join("decisions");
        o
    })
    .unwrap();
    fn get(port: u16, path: &str) -> String {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        write!(s, "GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
        let mut b = String::new();
        s.read_to_string(&mut b).unwrap();
        b
    }
    let first = get(handle.port(), "/api/runs?ref=base&run=fast");
    assert!(first.contains("saccade-perf-diff.v1"), "{first}");
    let session = get(handle.port(), "/compare?run=base&run=fast");
    let first_location = session
        .lines()
        .find(|l| l.to_lowercase().starts_with("location:"))
        .unwrap()
        .to_string();
    let perf_path = root.join("fast/saccade-perf.json");
    let modified = std::fs::metadata(&perf_path).unwrap().modified().unwrap();
    let mut changed: Value = serde_json::from_slice(&std::fs::read(&perf_path).unwrap()).unwrap();
    changed["frame"]["value"] = json!(7.0);
    changed["terms"][0]["value"] = json!(5.0);
    changed["terms"][2]["value"] = json!(2.5);
    changed["counters"]["render"]["work"] = json!(500.0);
    std::fs::write(&perf_path, changed.to_string()).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&perf_path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let second = get(handle.port(), "/api/runs?ref=base&run=fast");
    assert!(second.contains("\"delta\":-3.0"), "{second}");
    let session = get(handle.port(), "/compare?run=base&run=fast");
    let second_location = session
        .lines()
        .find(|l| l.to_lowercase().starts_with("location:"))
        .unwrap();
    assert_ne!(first_location, second_location);
    drop(handle);
}

#[test]
fn mcp_ablate_and_pair_results_validate_and_confine_noise_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    value(&run(
        root,
        &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
    ));
    let call = |id, name, args| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}});
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        call(
            2,
            "saccade_ablate",
            json!({"base_dir":"base","arm_dirs":["fast","same"],"perf_noise":"floor.toml","out_dir":"ablation",
                "perf_resolution_ms":0.25,"perf_resolution_ticks":3,"perf_min_delta_ms":1.0,"perf_min_delta_pct":1.0}),
        ),
        call(
            3,
            "saccade_compare",
            json!({"baseline_dir":"base","capture_dir":"fast","perf_noise":"floor.toml","out_dir":"pair","include_images":false}),
        ),
        call(
            4,
            "saccade_ablate",
            json!({"base_dir":"base","arm_dirs":["fast"],"perf_noise":"../outside.toml","out_dir":"outside"}),
        ),
    ];
    let mut c = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = c.stdin.take().unwrap();
    for r in requests {
        writeln!(input, "{r}").unwrap();
    }
    drop(input);
    let out = c.wait_with_output().unwrap();
    let replies: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let tool = replies[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "saccade_ablate")
        .unwrap();
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    let a = &replies[1]["result"];
    assert_eq!(a["isError"], false, "{a}");
    schema("saccade-ablate.v1", &a["structuredContent"]);
    assert_eq!(a["structuredContent"]["arms"][0]["flag"], "PERF-ONLY");
    assert_eq!(
        a["structuredContent"]["arms"][0]["perf_diff"]["resolution_ms"],
        0.25
    );
    assert_eq!(
        a["structuredContent"]["arms"][0]["perf_diff"]["resolution_ticks"],
        3
    );
    assert_eq!(
        a["structuredContent"]["arms"][0]["perf_diff"]["min_delta_ms"],
        1.0
    );
    let validator = jsonschema::validator_for(&tool["outputSchema"]).unwrap();
    assert!(validator.is_valid(&a["structuredContent"]));
    schema(
        "saccade-result.v1",
        &replies[2]["result"]["structuredContent"],
    );
    assert!(
        replies[2]["result"]["structuredContent"]["combined_verdict"]
            .as_str()
            .unwrap()
            .contains("beyond noise")
    );
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(
        replies[3]["result"]["structuredContent"]["code"],
        "unsafe_path"
    );
}
