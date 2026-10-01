//! The machine-readable contract: blind outputs that leak nothing, JSON errors,
//! the lean result, shipped schemas, and the MCP root policy and image blocks.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use image::{Rgb, RgbImage};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

/// A grey `w` x `h` frame with a bright `patch` (x, y, w, h) when given.
fn save(dir: &Path, name: &str, (w, h): (u32, u32), patch: Option<[u32; 4]>) {
    std::fs::create_dir_all(dir).expect("mkdir");
    let mut img = RgbImage::from_pixel(w, h, Rgb([90, 90, 90]));
    if let Some([px, py, pw, ph]) = patch {
        for y in py..py + ph {
            for x in px..px + pw {
                img.put_pixel(x, y, Rgb([230, 230, 230]));
            }
        }
    }
    img.save(dir.join(name)).expect("save");
}

/// Two directories with unmistakable names: `scene.png` differs, `same.png` does not.
fn dirs(tmp: &Path) -> (PathBuf, PathBuf) {
    let (base, cap) = (tmp.join("zz_alpha_dir"), tmp.join("zz_omega_dir"));
    for (d, patch) in [(&base, None), (&cap, Some([60, 80, 40, 30]))] {
        save(d, "scene.png", (160, 120), patch);
        save(d, "same.png", (160, 120), None);
    }
    (base, cap)
}

fn run(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(BIN)
        .args(args.iter().map(|a| a.as_ref()))
        .output()
        .expect("spawn")
}

fn stdout_json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {}",
            String::from_utf8_lossy(&o.stdout)
        )
    })
}

/// Every file under `dir` that is text (the page, scripts, JSON), as one string.
fn text_of_tree(dir: &Path) -> String {
    let mut all = String::new();
    for e in walk(dir) {
        if image::open(&e).is_err() {
            if let Ok(t) = std::fs::read_to_string(&e) {
                all.push_str(&t);
            }
        }
    }
    all
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).expect("read_dir") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn schema_check(file: &str, instance: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(file);
    let schema: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("schema file")).expect("json");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{e} at {}", e.instance_path))
        .collect();
    assert!(errors.is_empty(), "{file}: {errors:?}\n{instance}");
}

#[test]
fn blind_outputs_name_neither_the_reference_nor_the_sides() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let (view, keys) = (tmp.path().join("view"), tmp.path().join("keys"));
    let o = run(&[
        &"view",
        &base,
        &cap,
        &"--labels",
        &"lbl_one,lbl_two",
        &"--blind",
        &"--seed",
        &"31",
        &"--out",
        &view,
        &"--key-out",
        &keys.join("view-key.json"),
        &"--json",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let secrets = ["zz_alpha", "zz_omega", "lbl_one", "lbl_two"];
    let page = text_of_tree(&view);
    for s in secrets {
        assert!(!page.contains(s), "the view leaks {s:?}");
    }
    let html = std::fs::read_to_string(view.join("index.html")).unwrap();
    let embedded = html
        .split("id=\"saccade-data\">")
        .nth(1)
        .and_then(|t| t.split("</script>").next())
        .expect("embedded model");
    let model: Value = serde_json::from_str(&embedded.replace("<\\/", "</")).expect("model json");
    assert_eq!(model["blind"], true);
    assert_eq!(model["reference"], 2, "no pane is the reference");
    assert_ne!(model["seed"], 31, "the shuffle seed is not embedded");
    for set in model["sets"].as_array().unwrap() {
        assert_eq!(set["order"], json!([0, 1]), "no per-set order");
        for pane in set["panes"].as_array().unwrap() {
            assert!(pane["heatmap"].is_null() && pane["metrics"].is_null());
            assert!(pane["hotspots"].as_array().unwrap().is_empty());
            let path = pane["path"].as_str().unwrap();
            assert!(path.contains("/p_"), "neutral pane file name: {path}");
        }
    }
    let key: Value =
        serde_json::from_str(&std::fs::read_to_string(keys.join("view-key.json")).unwrap())
            .unwrap();
    assert_eq!(key["labels"], json!(["lbl_one", "lbl_two"]));
    assert_eq!(key["shuffle_seed"], 31);

    // The explain pack of a blind run: no report path, no labels, key elsewhere.
    let report = tmp.path().join("report");
    run(&[
        &"compare",
        &base,
        &cap,
        &"--labels",
        &"lbl_one,lbl_two",
        &"--out",
        &report,
    ]);
    let pack = tmp.path().join("pack");
    let o = run(&[
        &"explain",
        &report.join("saccade-report.v1.json"),
        &"--blind",
        &"--out",
        &pack,
        &"--key-out",
        &keys.join("explain-key.json"),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = text_of_tree(&pack);
    for s in secrets.into_iter().chain(["report", "baseline", "capture"]) {
        assert!(!text.contains(s), "the blind pack leaks {s:?}");
    }
    assert!(keys.join("explain-key.json").is_file());
    assert!(!walk(&pack).iter().any(|p| p.ends_with("blind-key.json")));
    let key: Value =
        serde_json::from_str(&std::fs::read_to_string(keys.join("explain-key.json")).unwrap())
            .unwrap();
    assert!(key["items"][0]["a"].as_str().unwrap().starts_with("lbl_"));
}

#[test]
fn every_failure_is_a_json_error_with_the_same_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let missing = tmp.path().join("nope");
    let report = tmp.path().join("report");
    run(&[&"compare", &base, &cap, &"--out", &report]);
    let rj = report.join("saccade-report.v1.json");
    let cases: Vec<(Output, &str)> = vec![
        // runtime I/O failure
        (run(&[&"compare", &missing, &cap, &"--json"]), "io"),
        // clap rejects the command line
        (run(&[&"compare", &"--json"]), "usage"),
        (
            run(&[&"summary", &rj, &"--format", &"json", &"--bogus"]),
            "usage",
        ),
        // a report that is not there, under --format json
        (run(&[&"summary", &missing, &"--format", &"json"]), "io"),
        // a name that escapes the baseline directory
        (
            run(&[&"approve", &cap, &base, &"../x", &"--json"]),
            "unsafe_path",
        ),
        // a blind key inside the pack it must stay out of
        (
            run(&[
                &"explain",
                &rj,
                &"--blind",
                &"--out",
                &tmp.path().join("p"),
                &"--key-out",
                &tmp.path().join("p/k.json"),
                &"--json",
            ]),
            "unsafe_path",
        ),
        (run(&[&"view", &base, &"--json"]), "usage"),
    ];
    for (o, code) in cases {
        assert_eq!(
            o.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let v = stdout_json(&o);
        assert_eq!(v["schema"], "saccade-error.v1");
        assert_eq!(v["code"], code, "{v}");
        assert!(!v["message"].as_str().unwrap().is_empty());
        schema_check("saccade-error.v1.schema.json", &v);
    }
    // Without --json the error stays on stderr.
    let o = run(&[&"compare", &missing, &cap]);
    assert!(o.stdout.is_empty() && !o.stderr.is_empty());
}

#[test]
fn compare_json_is_a_lean_result_and_full_is_the_report() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let out = tmp.path().join("report");
    let o = run(&[&"compare", &base, &cap, &"--out", &out, &"--json"]);
    assert_eq!(o.status.code(), Some(1));
    let v = stdout_json(&o);
    assert_eq!(v["schema"], "saccade-result.v1");
    assert_eq!(v["verdict"], "regression");
    assert_eq!(v["totals"]["fail"], 1);
    let f = &v["failing"][0];
    assert_eq!(f["name"], "scene.png");
    assert!(f["hotspots"].as_array().unwrap().len() <= 3);
    assert!(f["hotspots"][0]["share_of_total_error"].as_f64().unwrap() > 0.5);
    for p in ["report_json", "index_html"] {
        assert!(Path::new(v["paths"][p].as_str().unwrap()).is_absolute());
    }
    assert!(v["next_step"].as_str().unwrap().contains("saccade explain"));
    // Every float has at most 4 significant digits.
    fn floats(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Number(n) if n.is_f64() => out.extend(n.as_f64()),
            Value::Array(a) => a.iter().for_each(|x| floats(x, out)),
            Value::Object(m) => m.values().for_each(|x| floats(x, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    floats(&v, &mut all);
    assert!(!all.is_empty());
    for x in all {
        assert_eq!(
            format!("{x:.3e}").parse::<f64>().unwrap(),
            x,
            "{x} is not rounded"
        );
    }
    let full = stdout_json(&run(&[
        &"compare",
        &base,
        &cap,
        &"--out",
        &out,
        &"--json=full",
    ]));
    assert_eq!(full["schema"], "saccade-report.v1");
    assert!(full["entries"].as_array().unwrap().len() == 2);
    assert!(serde_json::to_string(&v).unwrap().len() < serde_json::to_string(&full).unwrap().len());
}

#[test]
fn shipped_schemas_validate_what_the_tools_print() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = dirs(tmp.path());
    let out = tmp.path().join("report");
    let rj = out.join("saccade-report.v1.json");
    let result = stdout_json(&run(&[&"compare", &base, &cap, &"--out", &out, &"--json"]));
    schema_check("saccade-result.v1.schema.json", &result);
    let ident = stdout_json(&run(&[
        &"identity",
        &base,
        &base,
        &"--out",
        &tmp.path().join("id"),
        &"--json",
    ]));
    schema_check("saccade-result.v1.schema.json", &ident);
    schema_check(
        "saccade-report.v1.schema.json",
        &stdout_json(&run(&[
            &"compare",
            &base,
            &cap,
            &"--out",
            &out,
            &"--json=full",
        ])),
    );
    schema_check(
        "saccade-summary.v1.schema.json",
        &stdout_json(&run(&[&"summary", &rj, &"--format", &"json"])),
    );
    schema_check(
        "saccade-explain.v1.schema.json",
        &stdout_json(&run(&[
            &"explain",
            &rj,
            &"--out",
            &tmp.path().join("pack"),
            &"--json",
        ])),
    );
    let key = tmp.path().join("keys/explain-key.json");
    run(&[
        &"explain",
        &rj,
        &"--blind",
        &"--out",
        &tmp.path().join("bpack"),
        &"--key-out",
        &key,
    ]);
    schema_check(
        "saccade-explain-blind-key.v1.schema.json",
        &serde_json::from_str(&std::fs::read_to_string(&key).unwrap()).unwrap(),
    );
    let vkey = tmp.path().join("keys/view-key.json");
    schema_check(
        "saccade-view-summary.v1.schema.json",
        &stdout_json(&run(&[
            &"view",
            &base,
            &cap,
            &"--blind",
            &"--out",
            &tmp.path().join("v"),
            &"--key-out",
            &vkey,
            &"--json",
        ])),
    );
    schema_check(
        "saccade-blind-key.v1.schema.json",
        &serde_json::from_str(&std::fs::read_to_string(&vkey).unwrap()).unwrap(),
    );
    schema_check(
        "saccade-approve.v1.schema.json",
        &stdout_json(&run(&[
            &"approve",
            &cap,
            &base,
            &"--all-failing",
            &rj,
            &"--json",
        ])),
    );
    // Decisions as the viewer exports them.
    let decisions = json!({"schema": "saccade-decisions.v1", "seed": 1, "labels": ["a", "b"],
        "blind": false, "sets": [{"name": "scene.png", "decision": "accept", "note": "", "timestamp_ms": 0}]});
    schema_check("saccade-decisions.v1.schema.json", &decisions);
    // MCP: the explain result and the summary result.
    let replies = mcp(
        tmp.path(),
        &[
            call(
                1,
                "saccade_explain",
                json!({"report_json": rj, "out_dir": tmp.path().join("mpack")}),
            ),
            call(2, "saccade_summary", json!({"report_json": rj})),
        ],
    );
    schema_check(
        "saccade-explain-result.v1.schema.json",
        &replies[0]["result"]["structuredContent"],
    );
    schema_check(
        "saccade-summary.v1.schema.json",
        &replies[1]["result"]["structuredContent"],
    );
}

/// Sends `requests` (one JSON message per line) to `saccade mcp --root root`.
fn mcp(root: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .arg("--root")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    for r in requests {
        writeln!(stdin, "{r}").expect("write");
    }
    drop(stdin);
    let out = child.wait_with_output().expect("wait");
    String::from_utf8(out.stdout)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("each stdout line is JSON"))
        .collect()
}

fn call(id: u32, tool: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": tool, "arguments": args}})
}

#[test]
fn mcp_refuses_every_path_outside_the_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    let (base, cap) = dirs(&root);
    let (obase, _) = dirs(&outside);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&obase, root.join("sneaky")).unwrap();
    let ok = json!({"baseline_dir": "zz_alpha_dir", "capture_dir": cap, "out_dir": "out"});
    let with = |k: &str, v: Value| {
        let mut a = ok.clone();
        a[k] = v;
        a
    };
    let mut requests = vec![
        call(1, "saccade_compare", ok.clone()),
        call(2, "saccade_compare", with("baseline_dir", json!(obase))),
        call(
            3,
            "saccade_compare",
            with("baseline_dir", json!("../outside/zz_alpha_dir")),
        ),
        call(
            4,
            "saccade_compare",
            with("out_dir", json!(outside.join("o"))),
        ),
        call(
            5,
            "saccade_compare",
            with("config", json!(outside.join("c.toml"))),
        ),
        call(
            6,
            "saccade_summary",
            json!({"report_json": "../outside/x.json"}),
        ),
        call(
            7,
            "saccade_explain",
            json!({"report_json": "out/saccade-report.v1.json", "out_dir": "p", "blind": true, "key_out": outside.join("k.json")}),
        ),
    ];
    if cfg!(unix) {
        requests.push(call(
            8,
            "saccade_compare",
            with("baseline_dir", json!("sneaky")),
        ));
    }
    let replies = mcp(&root, &requests);
    let _ = base;
    let first = &replies[0]["result"];
    assert_eq!(first["isError"], false, "{first}");
    assert!(root.join("out/saccade-report.v1.json").is_file());
    for (i, r) in replies.iter().enumerate().skip(1) {
        let r = &r["result"];
        assert_eq!(r["isError"], true, "request {}: {r}", i + 1);
        assert_eq!(
            r["structuredContent"]["code"],
            "unsafe_path",
            "request {}: {r}",
            i + 1
        );
    }
}

/// Decodes standard base64.
fn unbase64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= u32::from(val(c)) << (18 - 6 * i);
        }
        out.extend(&n.to_be_bytes()[1..chunk.len()]);
    }
    out
}

#[test]
fn mcp_results_carry_downscaled_strip_images_unless_asked_not_to() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = (tmp.path().join("base"), tmp.path().join("cap"));
    save(&base, "big.png", (1000, 700), None);
    save(&cap, "big.png", (1000, 700), Some([50, 40, 850, 600]));
    let replies = mcp(
        tmp.path(),
        &[
            call(
                1,
                "saccade_compare",
                json!({"baseline_dir": base, "capture_dir": cap, "out_dir": "a"}),
            ),
            call(
                2,
                "saccade_explain",
                json!({"report_json": "a/saccade-report.v1.json", "out_dir": "n", "include_images": false}),
            ),
            call(
                3,
                "saccade_explain",
                json!({"report_json": "a/saccade-report.v1.json", "out_dir": "e", "top": 3}),
            ),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
        ],
    );
    let blocks = |r: &Value| -> Vec<Value> {
        r["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["type"] == "image")
            .cloned()
            .collect()
    };
    for idx in [0, 2] {
        let images = blocks(&replies[idx]);
        assert!(
            (1..=3).contains(&images.len()),
            "{} image blocks",
            images.len()
        );
        for b in &images {
            assert_eq!(b["mimeType"], "image/png");
            let img = image::load_from_memory(&unbase64(b["data"].as_str().unwrap())).unwrap();
            assert_eq!(img.width(), 1024, "a 1536 px strip is shown at 1024 px");
        }
    }
    assert!(blocks(&replies[1]).is_empty(), "include_images=false");
    // The strips on disk never exceed 1536 px.
    let strip = image::open(tmp.path().join("a/explain/hotspots/big.png.d/h1.png")).unwrap();
    assert!(strip.width() <= 1536);
    // Every tool declares an output schema and annotations.
    for t in replies[3]["result"]["tools"].as_array().unwrap() {
        assert!(t["outputSchema"].is_object() && t["annotations"]["destructiveHint"] == false);
    }
}

#[test]
fn mcp_snapshot_returns_an_image_block_and_a_decision_request_is_schema_valid() {
    let tmp = tempfile::tempdir().unwrap();
    let (base, cap) = (tmp.path().join("base"), tmp.path().join("cap"));
    save(&base, "big.png", (400, 300), None);
    save(&cap, "big.png", (400, 300), Some([50, 40, 200, 150]));
    let replies = mcp(
        tmp.path(),
        &[
            call(
                1,
                "saccade_compare",
                json!({"baseline_dir": base, "capture_dir": cap, "out_dir": "a", "include_images": false}),
            ),
            call(
                2,
                "saccade_snapshot",
                json!({"report_json": "a/saccade-report.v1.json", "entry": "big.png", "state": "layout=swipe&split=0.25&zoom=2&at=200,150", "width": 800}),
            ),
            call(
                3,
                "saccade_decision_request",
                json!({"report_json": "a/saccade-report.v1.json", "all_failing": true, "intent": "brighten the patch"}),
            ),
        ],
    );
    let content = replies[1]["result"]["content"].as_array().unwrap();
    let image = content
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image block");
    assert_eq!(image["mimeType"], "image/png");
    let png = image::load_from_memory(&unbase64(image["data"].as_str().unwrap())).unwrap();
    assert_eq!(png.width(), 800);
    let path = replies[1]["result"]["structuredContent"]["paths"][0]
        .as_str()
        .unwrap();
    assert!(Path::new(path).is_file(), "{path}");
    schema_check(
        "saccade-decision-request.v1.schema.json",
        &replies[2]["result"]["structuredContent"],
    );
}
