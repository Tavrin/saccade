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
        .arg("--record-absolute-paths")
        .env("XDG_CACHE_HOME", root.join("cache"))
        .output()
        .unwrap()
}
fn value(root: &Path, o: &Output) -> Value {
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    let result: Value = serde_json::from_slice(&o.stdout).unwrap();
    if saccade_core::report_links::original_schema(result["schema"].as_str().unwrap_or_default())
        == "saccade-result.v2"
    {
        schema("saccade-result.v2", &result);
    }
    if saccade_core::report_links::original_schema(result["schema"].as_str().unwrap_or_default())
        == "saccade-result.v2"
        && result["artifact"].is_object()
        && result["operation"] != "compare"
        && result["operation"] != "identity"
    {
        return serde_json::from_slice(
            &std::fs::read(root.join(result["artifact"]["path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
    }
    result
}
fn save(root: &Path, name: &str, pass: f64, grey: u8) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbImage::from_pixel(32, 24, image::Rgb([grey, 90, 110]))
        .save(dir.join("scene.png"))
        .unwrap();
    let mut context: Value = serde_json::from_str(include_str!(
        "../../saccade-core/tests/fixtures/perf/context.json"
    ))
    .unwrap();
    context["capture_hash"] = json!(saccade_core::evidence::canonical::Digest::of_bytes(
        name.as_bytes()
    ));
    context["sample_window"]["hash"] = context["capture_hash"].clone();
    std::fs::write(dir.join("saccade-perf.json"),json!({"schema":"saccade-perf.v2","kind":"measurement","context":context,"unit":"ms","frame":{"value":pass+2.0,"samples":5,"stat":"p50"},"terms":[{"id":"render","kind":"pass","value":pass},{"id":"gap","kind":"gap","value":2.0},{"id":"detail","kind":"scope","parent":"render","value":pass/2.0}],"counters":{"render":{"work":pass*100.0}}}).to_string()).unwrap();
    std::fs::write(dir.join("gpu_clock.json"), json!({"schema":"saccade-gpu-clock.v1","device_id":"fixture-gpu","power_state":"ac-performance","windows":[{"name":"frame","core_mhz":{"min":1800.0,"median":1800.0,"max":1800.0},"memory_mhz":null,"sample_count":32,"expected_frames":5,"observed_frames":5,"query_failures":0,"throttle_reasons":[],"stabilized":true}]}).to_string()).unwrap();
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
    let successor = saccade_core::report_links::linked_schema(name);
    let name = if v["schema"] == successor {
        successor
    } else {
        name
    };
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/schemas")
        .join(format!("{name}.schema.json"));
    let s: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&s).unwrap();
    let errors: Vec<_> = validator.iter_errors(v).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}

#[test]
fn unstable_arm_cannot_keep_perf_only_or_exit_success() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    save(root, "base", 8.0, 80);
    save(root, "repeat", 8.1, 80);
    save(root, "arm1", 4.0, 80);
    save(root, "arm2", 4.0, 150);
    let output = run(
        root,
        &[
            "experiment",
            "ablate",
            "--base",
            "base",
            "--base",
            "repeat",
            "--arm",
            "unstable=arm1",
            "--arm",
            "unstable=arm2",
            "--out",
            "unstable",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let model: Value = serde_json::from_slice(
        &std::fs::read(root.join("unstable/saccade-ablate.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(model["arms"][0]["flag"], "INCONCLUSIVE");
    assert_eq!(model["arms"][0]["perf_only"], false);
    assert!(
        model["arms"][0]["combined_verdict"]
            .as_str()
            .unwrap()
            .contains("repeat instability")
    );
    assert_eq!(
        model["arms"][0]["validity_findings"][0],
        "arm output is not deterministic across repeats"
    );
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
    let configured = value(
        root,
        &run(
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
        ),
    );
    schema("saccade-noise.v1", &configured);
    assert_eq!(configured["perf_noise"]["resolution_ms"], 0.03);
    assert_eq!(configured["perf_noise"]["resolution_ticks"], 4);
    assert_eq!(configured["perf_noise"]["min_delta_ms"], 0.4);
    assert_eq!(configured["perf_noise"]["min_delta_pct"], 1.2);
    let recalibrated = value(
        root,
        &run(
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
        ),
    );
    assert_eq!(recalibrated["perf_noise"], configured["perf_noise"]);
    let custom = value(
        root,
        &run(
            root,
            &[
                "experiment",
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
        ),
    );
    assert_eq!(custom["arms"][0]["perf_diff"]["resolution_ms"], 0.03);
    assert_eq!(custom["arms"][0]["perf_diff"]["min_delta_ms"], 10.0);
    assert_eq!(custom["arms"][0]["flag"], "NO-EFFECT");
    let noise = value(
        root,
        &run(
            root,
            &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
        ),
    );
    schema("saccade-noise.v1", &noise);
    assert!((noise["perf_noise"]["frame"].as_f64().unwrap() - 0.1).abs() < 1e-12);
    let floor = value(
        root,
        &run(
            root,
            &[
                "noise",
                "base",
                "repeat",
                "--kind",
                "performance",
                "--out",
                "floor.json",
                "--json",
            ],
        ),
    );
    schema("saccade-perf.v2", &floor);
    assert_eq!(floor["comparability"], "qualified");
    let a = value(
        root,
        &run(
            root,
            &[
                "experiment",
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
        ),
    );
    schema("saccade-ablate.v1", &a);
    // Wave 11 orders rows by timing rank, rather than positional arguments.
    let arms = a["arms"].as_array().unwrap();
    let by_label = |label| arms.iter().find(|arm| arm["label"] == label).unwrap();
    assert_eq!(by_label("same")["flag"], "NO-EFFECT");
    assert_eq!(by_label("fast")["flag"], "PERF-ONLY");
    assert_eq!(by_label("image")["flag"], "IMAGE-CHANGE");
    assert_eq!(by_label("image")["config_differs"], json!(["quality"]));
    assert!(
        !by_label("fast")["top_deltas"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        arms.iter()
            .map(|arm| arm["label"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["fast", "image", "same"]
    );
    let html = std::fs::read_to_string(root.join("ablation/index.html")).unwrap();
    assert!(html.contains("saccadePerf.ablation"));
    assert!(html.contains("--surface"));
    for command in ["compare", "identity"] {
        let lean = value(
            root,
            &run(
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
            ),
        );
        if command == "compare" {
            schema("saccade-result.v2", &lean);
        } else {
            assert_eq!(
                lean["schema"],
                saccade_core::report_links::linked_schema("saccade-result.v1")
            );
        }
        let full: Value = serde_json::from_slice(
            &std::fs::read(root.join("pair/saccade-report.v1.json")).unwrap(),
        )
        .unwrap();
        assert!(
            full["combined_verdict"]
                .as_str()
                .unwrap()
                .contains("beyond noise")
        );
        assert_eq!(full["perf_diff"]["terms"].as_array().unwrap().len(), 3);
        schema("saccade-report.v1", &full);
        assert!(full["entries"][0].get("perf_diff").is_none());
        let markdown = run(
            root,
            &[
                "inspect",
                "export",
                "pair/saccade-report.v1.json",
                "--format",
                "markdown",
                "--out",
                "summary.md",
            ],
        );
        assert!(markdown.status.success());
        assert!(
            std::fs::read_to_string(root.join("summary.md"))
                .unwrap()
                .contains("beyond noise")
        );
        let explain = run(
            root,
            &[
                "inspect",
                "evidence",
                "pair/saccade-report.v1.json",
                "--out",
                "explain",
            ],
        );
        assert!(explain.status.success());
        assert!(
            std::fs::read_to_string(root.join("explain/explain.md"))
                .unwrap()
                .contains("beyond noise")
        );
    }
    let unknown = value(
        root,
        &run(
            root,
            &[
                "experiment",
                "ablate",
                "base",
                "same",
                "--out",
                "unknown",
                "--json",
            ],
        ),
    );
    assert_eq!(unknown["arms"][0]["flag"], "INCONCLUSIVE");
    let original = std::fs::read(root.join("base/saccade-perf.json")).unwrap();
    let unsafe_out = run(
        root,
        &[
            "experiment",
            "ablate",
            "base",
            "fast",
            "--out",
            "base",
            "--json",
        ],
    );
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
    value(
        root,
        &run(
            root,
            &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
        ),
    );
    let r = value(
        root,
        &run(
            root,
            &[
                "experiment",
                "ablate",
                "base",
                "fast",
                "same",
                "--perf-noise",
                "floor.toml",
                "--out",
                "runs",
                "--json",
            ],
        ),
    );
    schema("saccade-ablate.v1", &r);
    assert_eq!(r["arms"][0]["flag"], "PERF-ONLY");
    assert_eq!(r["arms"][1]["flag"], "NO-EFFECT");
    assert!(
        std::fs::read_to_string(root.join("runs/index.html"))
            .unwrap()
            .contains("PERF-ONLY")
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
        run(
            root,
            &[
                "view",
                "base",
                "fast",
                "--blind",
                "--key-out",
                "blind-key.json",
                "--out",
                "blind"
            ]
        )
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
    let outputs = tempfile::tempdir().unwrap();
    fixture(root);
    value(
        root,
        &run(
            root,
            &["noise", "base", "repeat", "--out", "floor.toml", "--json"],
        ),
    );
    let call = |id, name, mut args: Value| {
        args["operation"] = json!(name);
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"saccade_measure","arguments":args}})
    };
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        call(
            2,
            "ablate",
            json!({"base_dir":"base","arm_dirs":["fast","same"],"perf_noise":"floor.toml","out":"ablation",
                "perf_resolution_ms":0.25,"perf_resolution_ticks":3,"perf_min_delta_ms":1.0,"perf_min_delta_pct":1.0}),
        ),
        call(
            3,
            "compare",
            json!({"baseline_dir":"base","capture_dir":"fast","perf_noise":"floor.toml","out":"pair","include_images":false}),
        ),
        call(
            4,
            "ablate",
            json!({"base_dir":"base","arm_dirs":["fast"],"perf_noise":"../outside.toml","out":"outside"}),
        ),
        call(
            5,
            "noise",
            json!({"dirs":["base","repeat"],"kind":"performance","out":"performance.json"}),
        ),
        call(
            6,
            "noise",
            json!({"dirs":["base","repeat"],"out":"image.toml"}),
        ),
        call(
            7,
            "ablate",
            json!({"base_dir":"base","arm_dirs":["same"],"perf_noise":"floor.json","out":"wrong-kind"}),
        ),
    ];
    let mut c = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(root)
        .arg("--out-root")
        .arg(outputs.path())
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
        .find(|t| t["name"] == "saccade_measure")
        .unwrap();
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    let a = &replies[1]["result"];
    assert_eq!(a["isError"], false, "{a}");
    schema("saccade-result.v2", &a["structuredContent"]);
    let full: Value = serde_json::from_slice(
        &std::fs::read(outputs.path().join("ablation/saccade-ablate.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(full["arms"][0]["flag"], "PERF-ONLY");
    assert_eq!(full["arms"][0]["perf_diff"]["resolution_ms"], 0.25);
    assert_eq!(full["arms"][0]["perf_diff"]["resolution_ticks"], 3);
    assert_eq!(full["arms"][0]["perf_diff"]["min_delta_ms"], 1.0);
    let validator = jsonschema::validator_for(&tool["outputSchema"]).unwrap();
    assert!(validator.is_valid(&a["structuredContent"]));
    schema(
        "saccade-result.v2",
        &replies[2]["result"]["structuredContent"],
    );
    let pair: Value = serde_json::from_slice(
        &std::fs::read(outputs.path().join("pair/saccade-report.v1.json")).unwrap(),
    )
    .unwrap();
    assert!(
        pair["combined_verdict"]
            .as_str()
            .unwrap()
            .contains("beyond noise")
    );
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(
        replies[3]["result"]["structuredContent"]["errors"][0]["code"],
        "unsafe_path"
    );
    for (index, file, family, kind, unit) in [
        (
            4,
            "performance.json",
            "saccade-perf.v2",
            "performance_noise",
            "ms",
        ),
        (5, "image.json", "saccade-noise.v1", "image_noise", "FLIP"),
    ] {
        let reply = &replies[index]["result"];
        assert_eq!(reply["isError"], false, "{reply}");
        let envelope = &reply["structuredContent"];
        schema("saccade-result.v2", envelope);
        assert_eq!(envelope["data"]["kind"], kind);
        assert_eq!(envelope["data"]["unit"], unit);
        let record: Value =
            serde_json::from_slice(&std::fs::read(outputs.path().join(file)).unwrap()).unwrap();
        schema(family, &record);
        assert_eq!(record["kind"], kind);
        assert_eq!(record["unit"], unit);
        assert!(record.get("units").is_none());
        if kind == "performance_noise" {
            assert_eq!(record["comparability"], "qualified");
            assert_eq!(envelope["data"]["comparability"], "qualified");
            assert_eq!(record["sources"].as_array().unwrap().len(), 2);
        }
    }
    assert_eq!(replies[6]["result"]["isError"], true);
    assert_eq!(
        replies[6]["result"]["structuredContent"]["errors"][0]["code"],
        "wrong_noise_kind"
    );
}

#[test]
fn typed_performance_noise_and_cross_kind_inputs_have_explicit_units_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    let noise = value(
        root,
        &run(
            root,
            &[
                "noise",
                "base",
                "repeat",
                "--kind",
                "performance",
                "--out",
                "performance.json",
                "--json",
            ],
        ),
    );
    schema("saccade-perf.v2", &noise);
    assert_eq!(noise["kind"], "performance_noise");
    assert_eq!(noise["unit"], "ms");
    assert_eq!(noise["comparability"], "qualified");
    let a = value(
        root,
        &run(
            root,
            &[
                "experiment",
                "ablate",
                "base",
                "same",
                "--perf-noise",
                "performance.json",
                "--out",
                "typed-ablation",
                "--json",
            ],
        ),
    );
    assert_eq!(a["arms"][0]["flag"], "NO-EFFECT");
    let wrong = run(
        root,
        &[
            "noise",
            "base",
            "repeat",
            "--kind",
            "image",
            "--config",
            "performance.json",
            "--out",
            "image.toml",
            "--json",
        ],
    );
    assert_eq!(wrong.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&wrong.stdout).unwrap();
    assert_eq!(error["errors"][0]["code"], "wrong_noise_kind");
    assert!(
        error["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("--kind image")
    );
    std::fs::write(
        root.join("legacy-performance.toml"),
        "[perf_noise]\nframe = 0.0\n",
    )
    .unwrap();
    let wrong = run(
        root,
        &[
            "noise",
            "base",
            "repeat",
            "--kind",
            "image",
            "--config",
            "legacy-performance.toml",
            "--out",
            "legacy-image.toml",
            "--json",
        ],
    );
    let error: Value = serde_json::from_slice(&wrong.stdout).unwrap();
    assert_eq!(wrong.status.code(), Some(2));
    assert_eq!(error["errors"][0]["code"], "wrong_noise_kind");
    std::fs::write(
        root.join("mixed-config.toml"),
        "threshold = 0.01\n[perf_noise]\nframe = 0.0\n",
    )
    .unwrap();
    let image = value(
        root,
        &run(
            root,
            &[
                "noise",
                "base",
                "repeat",
                "--kind",
                "image",
                "--config",
                "mixed-config.toml",
                "--out",
                "mixed-image.toml",
                "--json",
            ],
        ),
    );
    assert_eq!(image["kind"], "image_noise");
    assert_eq!(image["unit"], "FLIP");
    std::fs::write(
        root.join("image.json"),
        json!({"schema":"saccade-noise.v1","kind":"image_noise","unit":"FLIP"}).to_string(),
    )
    .unwrap();
    let wrong = run(
        root,
        &[
            "experiment",
            "ablate",
            "base",
            "same",
            "--perf-noise",
            "image.json",
            "--out",
            "wrong",
            "--json",
        ],
    );
    assert_eq!(wrong.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&wrong.stdout).unwrap();
    assert_eq!(error["errors"][0]["code"], "wrong_noise_kind");
    assert!(
        error["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("--kind performance")
    );
}

#[test]
fn gpu_maps_flow_through_cli_config_noise_and_confined_mcp() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    let map = json!({"fields":{"device_id":{"path":"job.device"},"power_state":{"path":"job.power"}},"windows":{"path":"job.windows","fields":{
        "name":{"path":"name"},"core_mhz":{"path":"core_mhz"},"sample_count":{"path":"sample_count"},"expected_frames":{"path":"expected_frames"},"observed_frames":{"path":"observed_frames"},"query_failures":{"path":"query_failures"},"throttle_reasons":{"path":"throttle_reasons"},"stabilized":{"path":"stabilized"}
    }}});
    std::fs::create_dir(root.join("config")).unwrap();
    std::fs::write(root.join("config/map.json"), map.to_string()).unwrap();
    std::fs::write(
        root.join("config/saccade.toml"),
        "gpu_clock_map = \"map.json\"\n",
    )
    .unwrap();
    for dir in ["base", "repeat", "same"] {
        let path = root.join(dir).join("gpu_clock.json");
        let clock: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        std::fs::write(path, json!({"schema":"pipeline-telemetry.v1","job":{"device":clock["device_id"],"power":clock["power_state"],"windows":clock["windows"]}}).to_string()).unwrap();
    }
    let direct = value(
        root,
        &run(
            root,
            &[
                "compare",
                "base",
                "same",
                "--gpu-clock-map",
                "config/map.json",
                "--out",
                "mapped-cli",
                "--json",
            ],
        ),
    );
    let configured = value(
        root,
        &run(
            root,
            &[
                "compare",
                "base",
                "same",
                "--config",
                "config/saccade.toml",
                "--out",
                "mapped-config",
                "--json",
            ],
        ),
    );
    assert_eq!(direct["performance"]["comparability"], "qualified");
    assert_eq!(direct["performance"], configured["performance"]);
    let output = run(
        root,
        &[
            "noise",
            "base",
            "repeat",
            "--kind",
            "performance",
            "--gpu-clock-map",
            "config/map.json",
            "--out",
            "mapped-noise.json",
            "--json",
        ],
    );
    let noise = value(root, &output);
    assert_eq!(noise["comparability"], "qualified", "{noise}");
    let outputs = tempfile::tempdir().unwrap();
    let mut child = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(root)
        .arg("--out-root")
        .arg(outputs.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for (id, args) in [
        (
            1,
            json!({"operation":"compare","baseline_dir":"base","capture_dir":"same","out":"mapped","gpu_clock_map":"config/map.json"}),
        ),
        (
            2,
            json!({"operation":"compare","baseline_dir":"base","capture_dir":"same","out":"unsafe","gpu_clock_map":"../map.json"}),
        ),
        (
            3,
            json!({"operation":"compare","baseline_dir":"base","capture_dir":"same","out":"configured","config":"config/saccade.toml"}),
        ),
    ] {
        writeln!(input,"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"saccade_measure","arguments":args}})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    for index in [0, 2] {
        assert_eq!(
            replies[index]["result"]["isError"], false,
            "{}",
            replies[index]
        );
        assert_eq!(
            replies[index]["result"]["structuredContent"]["performance"]["comparability"],
            "qualified"
        );
    }
    assert_eq!(replies[1]["result"]["isError"], true);
    assert_eq!(
        replies[1]["result"]["structuredContent"]["errors"][0]["code"],
        "unsafe_path"
    );
}
