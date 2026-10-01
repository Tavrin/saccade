//! Focused S6 production-path contracts (seven tests).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use flipdiff_core::bisect::{BisectOptions, runs};
use flipdiff_core::serve::{ServeHandle, ServeOptions};
use image::{Rgb, RgbImage};
use serde_json::{Value, json};
const BIN: &str = env!("CARGO_BIN_EXE_flipdiff");

fn image(dir: &Path, shade: u8) {
    std::fs::create_dir_all(dir).unwrap();
    RgbImage::from_pixel(16, 16, Rgb([shade; 3]))
        .save(dir.join("scene.png"))
        .unwrap();
}
fn validate(name: &str, value: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../schemas/flipdiff-{name}.v1.schema.json"));
    let schema: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let check = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = check.iter_errors(value).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}: {value}");
}
fn serve(root: &Path) -> (ServeHandle, PathBuf, PathBuf) {
    let archive = root.join("archive");
    std::fs::create_dir_all(&archive).unwrap();
    let cache = root.join("cache");
    let decisions = root.join("decisions");
    let mut opts = ServeOptions::new(archive);
    opts.cache_dir = cache.clone();
    opts.decisions_dir = decisions.clone();
    (flipdiff_core::serve::start(opts).unwrap(), cache, decisions)
}
#[allow(clippy::too_many_arguments)]
fn http(
    server: &ServeHandle,
    method: &str,
    path: &str,
    body: Value,
    token: Option<&str>,
    origin: Option<&str>,
    host: Option<&str>,
) -> (u16, Value) {
    let mut stream = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let body = if method == "POST" {
        body.to_string()
    } else {
        String::new()
    };
    let host = host
        .map(str::to_owned)
        .unwrap_or_else(|| format!("127.0.0.1:{}", server.port()));
    let auth = format!(
        "{}{}",
        token.map_or(String::new(), |t| format!("X-Flipdiff-Token: {t}\r\n")),
        origin.map_or(String::new(), |o| format!("Origin: {o}\r\n"))
    );
    write!(stream,"{method} {path} HTTP/1.1\r\nHost: {host}\r\n{auth}Connection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let i = bytes.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    let status = String::from_utf8_lossy(&bytes[..i])
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let body = serde_json::from_slice(&bytes[i + 4..])
        .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes[i + 4..])));
    (status, body)
}
fn post(server: &ServeHandle, path: &str, body: Value) -> (u16, Value) {
    http(
        server,
        "POST",
        path,
        body,
        Some(server.token()),
        Some(&format!("http://127.0.0.1:{}", server.port())),
        None,
    )
}

#[test]
fn bisect_a_identity_threshold_selection_skips_and_observed_non_monotonicity() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("reference");
    image(&base, 64);
    let dirs: Vec<_> = (0..7)
        .map(|i| tmp.path().join(format!("run-{i}")))
        .collect();
    for (i, d) in dirs.iter().enumerate() {
        image(d, if i < 3 { 64 } else { 140 });
    }
    let result = runs(
        &dirs,
        None,
        &tmp.path().join("normal"),
        &BisectOptions::default(),
    )
    .unwrap();
    assert_eq!(result.first_bad.as_deref(), dirs[3].to_str());
    assert_eq!(result.last_good.as_deref(), dirs[2].to_str());
    assert!(result.total_probes <= 5);
    validate("bisect", &serde_json::to_value(&result).unwrap());
    let cli = Command::new(BIN)
        .current_dir(tmp.path())
        .args([
            "bisect",
            "--runs",
            "run-0",
            "run-1",
            "run-4",
            "--out",
            "relative-out",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        cli.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli["first_bad"], dirs[4].display().to_string());
    validate("bisect", &cli);
    let relaxed = runs(
        &dirs,
        None,
        &tmp.path().join("relaxed"),
        &BisectOptions {
            threshold: Some(1.0),
            entries: Some("scene.png".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(relaxed.status, "pass");
    std::fs::write(dirs[3].join("scene.png"), b"broken png").unwrap();
    let skipped = runs(
        &dirs,
        None,
        &tmp.path().join("skip"),
        &BisectOptions::default(),
    )
    .unwrap();
    assert_eq!(skipped.status, "inconclusive");
    assert!(skipped.first_bad.is_none());
    assert_eq!(
        skipped.candidates,
        vec![dirs[3].display().to_string(), dirs[4].display().to_string()]
    );
    assert!(
        skipped
            .probes
            .iter()
            .any(|p| p.verdict == "skip" && p.report_dir.is_some())
    );
    let result = runs(
        &[dirs[4].clone(), dirs[0].clone()],
        Some(&base),
        &tmp.path().join("nonmono"),
        &BisectOptions::default(),
    )
    .unwrap();
    assert_eq!(result.status, "non_monotonic");
    assert_eq!(result.non_monotonic.len(), 1);
    validate("bisect", &serde_json::to_value(result).unwrap());
    let unsafe_out = runs(&dirs, None, &dirs[0].join("out"), &BisectOptions::default());
    assert!(unsafe_out.is_err());
    // The low bit of a 16-bit PNG changes native samples but disappears on
    // the FLIP pipeline's 8-bit conversion.
    let native: Vec<_> = ["native-a", "native-b"]
        .iter()
        .map(|name| tmp.path().join(name))
        .collect();
    for (dir, value) in native.iter().zip([25_700u16, 25_701]) {
        std::fs::create_dir(dir).unwrap();
        image::ImageBuffer::<Rgb<u16>, Vec<u16>>::from_pixel(16, 16, Rgb([value; 3]))
            .save(dir.join("scene.png"))
            .unwrap();
    }
    let strict = runs(
        &native,
        None,
        &tmp.path().join("native-strict"),
        &BisectOptions::default(),
    )
    .unwrap();
    assert_eq!(strict.first_bad.as_deref(), native[1].to_str());
    let relaxed = runs(
        &native,
        None,
        &tmp.path().join("native-flip"),
        &BisectOptions {
            threshold: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(relaxed.status, "pass");
    let empty = tmp.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let missing = runs(
        &[base.clone(), empty],
        None,
        &tmp.path().join("missing"),
        &BisectOptions::default(),
    )
    .unwrap();
    assert_eq!(missing.status, "found");
    #[cfg(unix)]
    {
        let out = tmp.path().join("linked-output");
        let external = tempfile::tempdir().unwrap();
        std::fs::create_dir(&out).unwrap();
        std::fs::write(out.join("flipdiff-bisect.v1.json"), "{}").unwrap();
        std::os::unix::fs::symlink(external.path(), out.join("probe-0")).unwrap();
        assert!(runs(&native, None, &out, &BisectOptions::default()).is_err());
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
    }
}

fn git(root: &Path, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
#[test]
fn bisect_b_fake_capture_in_temporary_git_preserves_git_state_and_skips_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q"], "");
    let tree = git(root, &["mktree"], "");
    let good = git(root, &["commit-tree", &tree], "good\n");
    let mut revs = vec![good];
    for _ in 0..3 {
        revs.push(git(
            root,
            &["commit-tree", &tree, "-p", revs.last().unwrap()],
            "next\n",
        ));
    }
    let same = root.join("same");
    let bad = root.join("bad");
    image(&same, 64);
    image(&bad, 140);
    let before = std::fs::read(root.join(".git/HEAD")).unwrap();
    let run = |skip: bool| {
        let command = format!(
            "{}if [ {{rev}} = {} ]; then cp {}/scene.png {{out}}/scene.png; else cp {}/scene.png {{out}}/scene.png; fi",
            if skip {
                format!("if [ {{rev}} = {} ]; then exit 9; fi; ", revs[2])
            } else {
                String::new()
            },
            revs[1],
            same.display(),
            bad.display()
        );
        Command::new(BIN)
            .current_dir(root)
            .args([
                "bisect",
                "--git",
                &format!("{}..{}", revs[0], revs[3]),
                "--capture-cmd",
                &command,
                "--reference",
            ])
            .arg(&same)
            .arg("--out")
            .arg(root.join(if skip { "skip" } else { "reports" }))
            .arg("--json")
            .output()
            .unwrap()
    };
    let out = run(false);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["first_bad"], revs[2]);
    validate("bisect", &v);
    let out = run(true);
    assert_eq!(out.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "inconclusive");
    assert!(
        v["probes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["verdict"] == "skip")
    );
    assert_eq!(before, std::fs::read(root.join(".git/HEAD")).unwrap());
    assert!(!root.join(".git/index").exists());
}

#[test]
fn watch_once_and_debounced_change_produce_schema_valid_results() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base");
    let cap = tmp.path().join("cap");
    image(&base, 64);
    image(&cap, 64);
    let out = Command::new(BIN)
        .args(["watch"])
        .arg(&base)
        .arg(&cap)
        .arg("--out")
        .arg(tmp.path().join("once"))
        .args(["--once", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    validate("result", &v);
    let opts = flipdiff_core::watch::WatchOptions {
        baseline: base.clone(),
        capture: cap.clone(),
        out: tmp.path().join("watch"),
        config: Default::default(),
        debounce: Duration::from_millis(100),
    };
    let (tx, rx) = mpsc::channel();
    let watcher = flipdiff_core::watch::start(opts, move |r| {
        tx.send(r.map(|r| r.is_regression()).map_err(|e| e.to_string()))
            .unwrap();
    });
    assert!(!rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap());
    image(&cap, 140);
    assert!(rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap());
    drop(watcher);
    #[cfg(unix)]
    {
        image(&cap, 64);
        let mut child = Command::new(BIN)
            .arg("watch")
            .arg(&base)
            .arg(&cap)
            .arg("--out")
            .arg(tmp.path().join("cli-watch"))
            .args(["--debounce-ms", "100", "--json"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx
                    .send(serde_json::from_str::<Value>(&line.unwrap()).unwrap())
                    .is_err()
                {
                    break;
                }
            }
        });
        let initial = rx.recv_timeout(Duration::from_secs(10));
        if initial.is_err() {
            child.kill().unwrap();
        }
        let initial = initial.unwrap();
        assert_eq!(initial["verdict"], "pass");
        validate("result", &initial);
        image(&cap, 140);
        let changed = rx.recv_timeout(Duration::from_secs(10));
        if changed.is_err() {
            child.kill().unwrap();
        }
        let changed = changed.unwrap();
        assert_eq!(changed["verdict"], "regression");
        validate("result", &changed);
        assert!(
            Command::new("kill")
                .args(["-INT", &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(1));
                break;
            }
            if started.elapsed() > Duration::from_secs(10) {
                child.kill().unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        reader.join().unwrap();
    }
}

#[test]
fn inbox_security_persistence_invalid_answers_and_discovery_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let (server, cache, decisions) = serve(tmp.path());
    let q = json!({"question":"Is this intended?","allowed_answers":["accept","reject"],"context":"Light changed","link":"/compare?runs=a,b#entry=scene.png"});
    let origin = format!("http://127.0.0.1:{}", server.port());
    for (token, origin, host) in [
        (None, Some(origin.as_str()), None),
        (Some("wrong"), Some(origin.as_str()), None),
        (Some(server.token()), Some("http://evil.invalid"), None),
        (
            Some(server.token()),
            Some(origin.as_str()),
            Some("evil.invalid"),
        ),
    ] {
        assert_eq!(
            http(
                &server,
                "POST",
                "/api/inbox",
                q.clone(),
                token,
                origin,
                host
            )
            .0,
            403
        );
    }
    assert_eq!(
        post(
            &server,
            "/api/inbox",
            json!({"question":"bad link","allowed_answers":["yes"],"link":"javascript:alert(1)"})
        )
        .0,
        400
    );
    let (status, v) = post(&server, "/api/inbox", q);
    assert_eq!(status, 200);
    let id = v["id"].as_str().unwrap();
    assert!(decisions.join("inbox").join(format!("{id}.json")).is_file());
    assert_eq!(
        post(
            &server,
            &format!("/api/inbox/{id}/answer"),
            json!({"answer":"invalid"})
        )
        .0,
        400
    );
    let (status, item) = post(
        &server,
        &format!("/api/inbox/{id}/answer"),
        json!({"answer":"reject","note":"Unexpected light"}),
    );
    assert_eq!(status, 200);
    assert_eq!(item["answer"], "reject");
    validate("inbox-item", &item);
    let (_, second) = post(
        &server,
        "/api/inbox",
        json!({"question":"Second","allowed_answers":["yes"]}),
    );
    let (_, list) = http(&server, "GET", "/api/inbox", Value::Null, None, None, None);
    assert_eq!(list[0]["id"], second["id"]);
    assert_eq!(list[1]["id"], id);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(cache.join("serve.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let port = server.port();
    drop(server);
    let mut opts = ServeOptions::new(tmp.path().join("archive"));
    opts.cache_dir = cache;
    opts.decisions_dir = decisions;
    opts.port = port;
    let server = flipdiff_core::serve::start(opts).unwrap();
    assert_eq!(
        http(
            &server,
            "GET",
            &format!("/api/inbox/{id}"),
            Value::Null,
            None,
            None,
            None
        )
        .1["answer"],
        "reject"
    );
}

#[test]
fn ask_wait_round_trip_timeout_and_non_loopback_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    let (server, cache, _) = serve(tmp.path());
    let child = Command::new(BIN)
        .args([
            "ask",
            "--serve",
            &format!("http://127.0.0.1:{}", server.port()),
            "--question",
            "Continue?",
            "--answers",
            "yes,no",
            "--cache-dir",
        ])
        .arg(&cache)
        .args(["--wait", "--timeout", "10", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let id = loop {
        let (_, v) = http(&server, "GET", "/api/inbox", Value::Null, None, None, None);
        if let Some(id) = v[0]["id"].as_str() {
            break id.to_owned();
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        post(
            &server,
            &format!("/api/inbox/{id}/answer"),
            json!({"answer":"yes","note":"Reviewed"})
        )
        .0,
        200
    );
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["answer"], "yes");
    validate("ask-result", &v);
    let out = Command::new(BIN)
        .args([
            "ask",
            "--serve",
            &format!("http://127.0.0.1:{}", server.port()),
            "--question",
            "Timeout",
            "--answers",
            "yes",
            "--cache-dir",
        ])
        .arg(&cache)
        .args(["--wait", "--timeout", "0", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["timed_out"], true);
    validate("ask-result", &v);
    for host in ["localhost", "example.invalid", "127.0.0.1.evil.invalid"] {
        let out = Command::new(BIN)
            .args([
                "ask",
                "--serve",
                &format!("http://{host}:{}", server.port()),
                "--question",
                "q",
                "--answers",
                "yes",
                "--json",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap()["code"],
            "usage"
        );
    }
}

fn mcp_send(child: &mut std::process::Child, id: u64, name: &str, args: Value) {
    writeln!(child.stdin.as_mut().unwrap(),"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
}
#[test]
fn mcp_ask_inbox_bisect_are_schema_valid_and_capture_commands_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (server, cache, _) = serve(tmp.path());
    let base = tmp.path().join("base");
    let bad = tmp.path().join("bad");
    image(&base, 64);
    image(&bad, 140);
    let mut child = Command::new(BIN)
        .arg("mcp")
        .arg("--root")
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let url = format!("http://127.0.0.1:{}", server.port());
    mcp_send(
        &mut child,
        1,
        "flipdiff_ask_human",
        json!({"serve":url,"cache_dir":cache,"question":"Proceed?","allowed_answers":["yes","no"]}),
    );
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["isError"], false);
    let result = &v["result"]["structuredContent"];
    validate("ask-result", result);
    let id = result["id"].as_str().unwrap();
    post(
        &server,
        &format!("/api/inbox/{id}/answer"),
        json!({"answer":"no"}),
    );
    mcp_send(
        &mut child,
        2,
        "flipdiff_inbox_get",
        json!({"serve":url,"cache_dir":cache,"id":id}),
    );
    line.clear();
    output.read_line(&mut line).unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["structuredContent"]["answer"], "no");
    validate("inbox-item", &v["result"]["structuredContent"]);
    mcp_send(
        &mut child,
        3,
        "flipdiff_bisect",
        json!({"runs":[base,bad],"out_dir":"bisect"}),
    );
    line.clear();
    output.read_line(&mut line).unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    validate("bisect", &v["result"]["structuredContent"]);
    assert_eq!(
        v["result"]["structuredContent"]["first_bad"],
        bad.display().to_string()
    );
    for (id, args) in [
        (4, json!({"runs":[base,bad],"out_dir":"../escape"})),
        (
            5,
            json!({"runs":[base,bad],"out_dir":"bisect","capture_cmd":"touch sentinel"}),
        ),
    ] {
        mcp_send(&mut child, id, "flipdiff_bisect", args);
        line.clear();
        output.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["result"]["isError"], true);
    }
    child.stdin.take();
    assert!(child.wait().unwrap().success());
    assert!(!tmp.path().join("sentinel").exists());
}

#[test]
fn mcp_watch_status_and_notifications_share_valid_jsonl_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base");
    let cap = tmp.path().join("cap");
    image(&base, 64);
    image(&cap, 64);
    let mut child = Command::new(BIN)
        .args(["mcp", "--root"])
        .arg(tmp.path())
        .arg("--watch")
        .arg("base:cap")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            tx.send(serde_json::from_str::<Value>(&line.unwrap()).unwrap())
                .unwrap();
        }
    });
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let start = Instant::now();
    loop {
        mcp_send(
            &mut child,
            1,
            "flipdiff_watch_status",
            json!({"capture_dir":"cap"}),
        );
        let v = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        if v["result"]["structuredContent"]["verdict"] == "pass"
            || v["method"] == "notifications/message"
        {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    image(&cap, 140);
    let start = Instant::now();
    loop {
        let v = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        if v["method"] == "notifications/message"
            && v["params"]["data"]["result"]["verdict"] == "regression"
        {
            validate("result", &v["params"]["data"]["result"]);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    child.stdin.take();
    assert!(child.wait().unwrap().success());
    reader.join().unwrap();
}
