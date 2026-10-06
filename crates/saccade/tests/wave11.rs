//! Generated CLI and cross-link proofs, wired into the coordinator gate.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn invoke(root: &Path, args: &[String]) -> (std::process::ExitStatus, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        !out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status, serde_json::from_slice(&out.stdout).unwrap())
}
fn session() -> Value {
    let mut pairs = Vec::new();
    for aa in [false, true] {
        for i in 0..12 {
            pairs.push(json!({"id":format!("{aa}-{i}"),"block":format!("{aa}-{i}"),"order":if i%2==0{"ab"}else{"ba"},"a":[10.],"b":[if aa{10.}else{11.}],"aa":aa}));
        }
    }
    json!({"schema":"saccade-timing-session.v1","session":"lab-run","band_pct":2,"max_pairs":12,"confidence":0.95,"seed":12,"resamples":1024,"pairs":pairs})
}
#[test]
#[ignore = "heavy: generated CLI contracts and index workflow"]
fn timing_imports_all_adapters_and_links_to_a_portable_external_hub() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("hyperfine.json"),
        serde_json::to_vec(&json!({"results":[{"times":vec![0.01;12]}]})).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("metric.json"),
        br#"{"readings":{"elapsed_us":11000}}"#,
    )
    .unwrap();
    std::fs::write(root.join("saccade-perf.json"),serde_json::to_vec(&json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":11,"samples":2,"stat":"mean"},"terms":[],"counters":{}})).unwrap()).unwrap();
    let mut s = session();
    for (i, p) in s["pairs"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .take(12)
        .enumerate()
    {
        p["a"] = json!({"file":"hyperfine.json","format":"hyperfine","index":i});
        p["b"] = if i % 2 == 0 {
            json!({"file":"metric.json","format":"json","path":"readings.elapsed_us","unit":"us"})
        } else {
            json!({"file":"saccade-perf.json","format":"perf"})
        };
    }
    std::fs::write(root.join("session.json"), serde_json::to_vec(&s).unwrap()).unwrap();
    let (status, r) = invoke(
        root,
        &[
            "timing".into(),
            "ab".into(),
            "session.json".into(),
            "--out".into(),
            "timing".into(),
            "--source-ref".into(),
            "urn:specimen:7".into(),
            "--source-ref".into(),
            "urn:catalog:case7".into(),
            "--json".into(),
        ],
    );
    assert_eq!(status.code(), Some(1));
    assert_eq!(r["verdict"], "slower");
    assert_eq!(r["source_refs"].as_array().unwrap().len(), 2);
    assert_eq!(r["sources"].as_object().unwrap().len(), 4);
    assert_eq!(r["diagnostic_only"], true);
    let persisted: Value = serde_json::from_slice(
        &std::fs::read(root.join("timing/saccade-timing-ab.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(persisted["report_id"], r["report_id"]);
    let (status, rows) = invoke(
        root,
        &[
            "index".into(),
            "export".into(),
            "--format".into(),
            "json".into(),
        ],
    );
    assert!(status.success());
    assert_eq!(rows[0]["report_id"], r["report_id"]);
    assert_eq!(rows[0]["source_refs"][0], "urn:specimen:7");
    let mut csv = String::from("id,block,order,a_ms,b_ms,aa\n");
    for p in session()["pairs"].as_array().unwrap() {
        csv.push_str(&format!(
            "{},{},{},10,{},{}\n",
            p["id"].as_str().unwrap(),
            p["block"].as_str().unwrap(),
            p["order"].as_str().unwrap(),
            p["b"][0],
            p["aa"]
        ));
    }
    std::fs::write(root.join("pairs.csv"), csv).unwrap();
    let (status, from_csv) = invoke(
        root,
        &[
            "timing".into(),
            "ab".into(),
            "pairs.csv".into(),
            "--format".into(),
            "csv".into(),
            "--out".into(),
            "csv".into(),
            "--json".into(),
        ],
    );
    assert_eq!(status.code(), Some(1));
    assert_eq!(from_csv["verdict"], "slower");
}
#[test]
#[ignore = "heavy: generated CLI temporal trajectory"]
fn event_marker_produces_known_settling_and_a_standalone_strip_chart() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let frames = root.join("frames");
    std::fs::create_dir(&frames).unwrap();
    for (i, v) in [255, 255, 255, 180, 80, 0, 0, 0].into_iter().enumerate() {
        image::RgbaImage::from_pixel(8, 8, image::Rgba([v, v, v, 255]))
            .save(frames.join(format!("sample-{i}.png")))
            .unwrap();
    }
    std::fs::write(root.join("event.json"), br#"{"change_frame":2}"#).unwrap();
    let (status, r) = invoke(
        root,
        &[
            "experiment".into(),
            "settle".into(),
            "frames".into(),
            "--event".into(),
            "event.json".into(),
            "--consecutive".into(),
            "2".into(),
            "--final-frames".into(),
            "2".into(),
            "--tile-size".into(),
            "4".into(),
            "--out".into(),
            "trajectory".into(),
            "--json".into(),
        ],
    );
    assert!(status.success());
    assert_eq!(r["settle_frame"], 5);
    assert_eq!(r["lag_frames"], 1);
    assert_eq!(r["sources"].as_object().unwrap().len(), 9);
    assert_eq!(r["tiles"].as_array().unwrap().len(), 4);
    assert!(root.join("trajectory/index.html").is_file());
}

#[test]
fn fingerprint_record_size_cli_overrides_map_and_echoes_effective_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("specimen.json");
    let record = json!({"schema":"saccade-arm-fingerprint.v1","producer":{"binary":"pin","build":{"profile":"release"}},"inputs":{"identity":"sample"},"run":{"mode":"lab","session":"session","env":{},"readiness":[{"criterion":{"name":"prepared","parameters":{}},"reached":true,"observed":1}]},"telemetry":"x".repeat(2*1024*1024)});
    std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_saccade"))
            .current_dir(tmp.path())
            .args(["arms", "check", "specimen.json", "specimen.json", "--json"])
            .args(extra)
            .output()
            .unwrap()
    };
    let output = run(&[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["max_record_bytes"], 16 * 1024 * 1024);
    std::fs::write(
        tmp.path().join("map.json"),
        br#"{"max_record_bytes":1048576}"#,
    )
    .unwrap();
    let output = run(&["--fingerprint-map", "map.json"]);
    assert_eq!(output.status.code(), Some(2));
    let error = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for expected in [
        "specimen.json".to_string(),
        std::fs::metadata(&path).unwrap().len().to_string(),
        "1048576".into(),
        "--max-record-bytes".into(),
        "max_record_bytes".into(),
    ] {
        assert!(error.contains(&expected), "{error}");
    }
    let output = run(&[
        "--fingerprint-map",
        "map.json",
        "--max-record-bytes",
        "16777216",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["max_record_bytes"],
        16777216
    );
    let output = run(&["--max-record-bytes", "67108865"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .contains("hard ceiling")
    );
}
