//! Generated, offline regressions for security review findings 4, 5, 7 and 8.
#![cfg(all(feature = "ai", feature = "mcp"))]
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
struct Fixture {
    temp: tempfile::TempDir,
    input: PathBuf,
    out: PathBuf,
    report: PathBuf,
    config: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        let out = temp.path().join("out");
        for (dir, color) in [(input.join("baseline"), 20), (input.join("candidate"), 220)] {
            std::fs::create_dir_all(&dir).unwrap();
            image::RgbImage::from_pixel(8, 8, image::Rgb([color; 3]))
                .save(dir.join("sample.png"))
                .unwrap();
        }
        std::fs::create_dir(&out).unwrap();
        let reports = input.join("reports");
        let result = Command::new(BIN)
            .args(["compare"])
            .arg(input.join("baseline"))
            .arg(input.join("candidate"))
            .arg("--out")
            .arg(&reports)
            .arg("--json")
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report = reports.join("saccade-report.v1.json");
        let config = temp.path().join("user.toml");
        std::fs::write(
            &config,
            format!(
                "out_root = {:?}\n[[roots]]\nid = 'input'\npath = {:?}\negress = 'deny'\n",
                out.to_str().unwrap(),
                input.to_str().unwrap()
            ),
        )
        .unwrap();
        Self {
            temp,
            input,
            out,
            report,
            config,
        }
    }
    fn cli(&self, report: &Path, out: &Path, extra: &[&str]) -> (i32, Value) {
        let r = self
            .command()
            .arg("review")
            .arg(report)
            .arg("--out")
            .arg(out)
            .arg("--user-config")
            .arg(&self.config)
            .args(extra)
            .arg("--json")
            .output()
            .unwrap();
        (
            r.status.code().unwrap(),
            serde_json::from_slice(&r.stdout).unwrap(),
        )
    }
    fn command(&self) -> Command {
        let mut c = Command::new(BIN);
        c.current_dir(self.temp.path())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", self.temp.path())
            .env("RAYON_NUM_THREADS", "2");
        c
    }
    fn mcp(&self, args: Value) -> Value {
        let mut child = self
            .command()
            .args(["mcp", "--root"])
            .arg(&self.input)
            .arg("--out-root")
            .arg(&self.out)
            .arg("--user-config")
            .arg(&self.config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for m in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"secfix","version":"1"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"saccade_review","arguments":args}}),
        ] {
            writeln!(stdin, "{m}").unwrap();
        }
        drop(stdin);
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .find(|v| v["id"] == 2)
            .unwrap()["result"]
            .clone()
    }
}
#[test]
fn rooted_relative_report_intent_and_output_use_authorized_paths() {
    let f = Fixture::new();
    std::fs::create_dir(f.temp.path().join("reports")).unwrap();
    std::fs::write(
        f.temp.path().join("reports/saccade-report.v1.json"),
        b"poison report",
    )
    .unwrap();
    let intent = json!({"id":"intent","objective":"keep detail","assurance":"structured","expected_changes":[],"invariants":[],"criteria":[],"source":null,"mask_sources":[],"provenance":{"timestamp_unix_ms":null,"paths":[],"source_roots":[],"source":null}});
    std::fs::write(
        f.input.join("intent.json"),
        serde_json::to_vec(&intent).unwrap(),
    )
    .unwrap();
    std::fs::write(f.temp.path().join("intent.json"), b"poison intent").unwrap();
    let (exit, value) = f.cli(
        Path::new("reports/saccade-report.v1.json"),
        Path::new("preview"),
        &["--intent-file", "intent.json"],
    );
    assert_eq!(exit, 0, "{value}");
    assert!(f.out.join("preview/requests.json").is_file());
    assert!(f.out.join("preview/preview.json").is_file());
    assert!(!f.temp.path().join("preview").exists());
    // Execution must also parse the allowed intent rather than the cwd poison.
    let (exit, value) = f.cli(
        &f.report,
        Path::new("run.json"),
        &[
            "--run",
            "--budget-calls",
            "1",
            "--intent-file",
            "intent.json",
        ],
    );
    assert_eq!(exit, 2);
    assert_eq!(value["errors"][0]["code"], "egress_denied", "{value}");
}
#[cfg(unix)]
#[test]
fn mcp_preview_companion_marker_symlink_is_refused_without_truncation() {
    let f = Fixture::new();
    let sentinel = f.temp.path().join("sentinel");
    std::fs::write(&sentinel, b"preserve marker target").unwrap();
    std::os::unix::fs::symlink(&sentinel, f.out.join(".saccade-run")).unwrap();
    let result = f.mcp(json!({"operation":"preview","artifact":f.report,"out":"case.json"}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["errors"][0]["code"], "config",
        "{result}"
    );
    assert_eq!(std::fs::read(sentinel).unwrap(), b"preserve marker target");
    assert!(!f.out.join("case.json").exists());
}
#[cfg(unix)]
#[test]
fn mcp_expected_case_id_guards_companion_before_decode() {
    let f = Fixture::new();
    let foreign = f.temp.path().join("foreign.json");
    std::fs::write(&foreign, b"outside bytes must not reach JSON decoder").unwrap();
    let evidence = f.report.with_file_name("evidence.json");
    std::fs::remove_file(&evidence).unwrap();
    std::os::unix::fs::symlink(&foreign, evidence).unwrap();
    let result = f.mcp(json!({"operation":"preview","artifact":f.report,"out":"case.json","expected_case_id":format!("sha256:{}", "0".repeat(64))}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(
        result["structuredContent"]["errors"][0]["code"], "unsafe_path",
        "{result}"
    );
    assert!(!f.out.join("case.json").exists());
}
#[test]
fn cli_preview_hardlinked_output_preserves_input_bytes() {
    let f = Fixture::new();
    let baseline = f.input.join("baseline/sample.png");
    let before = std::fs::read(&baseline).unwrap();
    let preview = f.out.join("preview");
    std::fs::create_dir(&preview).unwrap();
    std::fs::hard_link(&baseline, preview.join("requests.json")).unwrap();
    let (exit, value) = f.cli(&f.report, &preview, &[]);
    assert_eq!(exit, 0, "{value}");
    assert!(
        std::fs::read(baseline).unwrap() == before,
        "input bytes changed"
    );
    assert!(
        serde_json::from_slice::<Value>(&std::fs::read(preview.join("requests.json")).unwrap())
            .is_ok()
    );
}
