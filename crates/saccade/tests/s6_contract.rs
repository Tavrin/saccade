//! Focused S6 production-path contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use image::{Rgb, RgbImage};
use saccade_core::bisect::{BisectOptions, runs};
use saccade_core::serve::{ServeHandle, ServeOptions};
use serde_json::{Value, json};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");

fn image(dir: &Path, shade: u8) {
    std::fs::create_dir_all(dir).unwrap();
    RgbImage::from_pixel(16, 16, Rgb([shade; 3]))
        .save(dir.join("scene.png"))
        .unwrap();
}
fn assert_same_path(recorded: &str, expected: &Path) {
    // Recorded paths use the product's canonical spelling, including resolved
    // macOS /var aliases and non-verbatim Windows paths.
    assert!(Path::new(recorded).is_absolute(), "{recorded}");
    assert_eq!(
        Path::new(recorded),
        saccade_core::paths::canonicalize(expected).unwrap()
    );
}
fn validate(name: &str, value: &Value) {
    let expected = format!("saccade-{name}.v1");
    let successor = saccade_core::report_links::linked_schema(&expected);
    let id = if value["schema"] == successor {
        successor
    } else {
        &expected
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/schemas")
        .join(format!("{id}.schema.json"));
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
    (saccade_core::serve::start(opts).unwrap(), cache, decisions)
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
        token.map_or(String::new(), |t| format!("X-Saccade-Token: {t}\r\n")),
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
    assert_same_path(result.first_bad.as_deref().unwrap(), &dirs[3]);
    assert_same_path(result.last_good.as_deref().unwrap(), &dirs[2]);
    assert!(result.total_probes <= 5);
    validate("bisect", &serde_json::to_value(&result).unwrap());
    let cli = Command::new(BIN)
        .current_dir(tmp.path())
        .args([
            "experiment",
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
    assert_eq!(
        cli["schema"],
        saccade_core::report_links::linked_schema("saccade-result.v2")
    );
    let full: Value = serde_json::from_slice(
        &std::fs::read(tmp.path().join("relative-out/saccade-bisect.v1.json")).unwrap(),
    )
    .unwrap();
    assert_same_path(full["first_bad"].as_str().unwrap(), &dirs[4]);
    validate("bisect", &full);
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
    assert_eq!(skipped.candidates.len(), 2);
    for (candidate, expected) in skipped.candidates.iter().zip(&dirs[3..5]) {
        assert_same_path(candidate, expected);
    }
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
    assert_same_path(strict.first_bad.as_deref().unwrap(), &native[1]);
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
        std::fs::write(out.join("saccade-bisect.v1.json"), "{}").unwrap();
        std::os::unix::fs::symlink(external.path(), out.join("probe-0")).unwrap();
        assert!(runs(&native, None, &out, &BisectOptions::default()).is_err());
        assert_eq!(std::fs::read_dir(external.path()).unwrap().count(), 0);
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
    let server = saccade_core::serve::start(opts).unwrap();
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
