//! HTTP-level tests of `flipdiff serve` against an ephemeral port.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

use super::*;

struct Fixture {
    _tmp: tempfile::TempDir,
    handle: ServeHandle,
    root: PathBuf,
    cache: PathBuf,
    decisions: PathBuf,
}

fn write_png(path: &Path, rgb: [u8; 3]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(16, 16, image::Rgb(rgb))
        .save(path)
        .unwrap();
}

fn fixture(max_upload: u64) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    for (run, rgb) in [
        ("g/a", [10, 10, 10]),
        ("g/b", [200, 10, 10]),
        ("g/c", [10, 200, 10]),
    ] {
        write_png(&root.join(run).join("x.png"), rgb);
    }
    std::fs::write(root.join("g/a/flipdiff-meta.json"), r#"{"mode":"Probe"}"#).unwrap();
    std::fs::write(tmp.path().join("secret.png"), b"not an image").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(tmp.path().join("secret.png"), root.join("g/link.png")).unwrap();
        std::os::unix::fs::symlink(tmp.path(), root.join("escape")).unwrap();
    }
    let (cache, decisions) = (tmp.path().join("cache"), tmp.path().join("decisions"));
    let mut opts = ServeOptions::new(root.clone());
    opts.cache_dir = cache.clone();
    opts.decisions_dir = decisions.clone();
    opts.max_upload_bytes = max_upload;
    let handle = start(opts).unwrap();
    Fixture {
        root: root.canonicalize().unwrap(),
        cache: cache.canonicalize().unwrap(),
        decisions: decisions.canonicalize().unwrap(),
        handle,
        _tmp: tmp,
    }
}

struct Reply {
    status: u16,
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn http(f: &Fixture, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Reply {
    let port = f.handle.port();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        req.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    s.write_all(req.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    let mut raw = Vec::new();
    let _ = s.read_to_end(&mut raw);
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    Reply {
        status,
        head,
        body: raw[split + 4..].to_vec(),
    }
}

fn get(f: &Fixture, path: &str) -> Reply {
    http(f, "GET", path, &[], b"")
}

fn post(f: &Fixture, path: &str, body: &[u8]) -> Reply {
    let port = f.handle.port().to_string();
    let origin = format!("http://127.0.0.1:{port}");
    http(
        f,
        "POST",
        path,
        &[("Origin", &origin), ("X-Flipdiff-Token", f.handle.token())],
        body,
    )
}

fn wait_ready(f: &Fixture, id: &str) {
    for _ in 0..200 {
        let r = get(f, &format!("/api/session/{id}/status"));
        let text = r.text();
        if text.contains("\"ready\"") {
            return;
        }
        assert!(!text.contains("failed"), "{text}");
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("session never became ready");
}

fn session_of(reply: &Reply) -> String {
    reply
        .head
        .lines()
        .find_map(|l| {
            l.strip_prefix("location: ")
                .or_else(|| l.strip_prefix("Location: "))
        })
        .and_then(|l| l.strip_prefix("/progress/"))
        .unwrap()
        .to_owned()
}

#[test]
fn rejects_traversal_and_symlink_escape() {
    let f = fixture(1024);
    for p in [
        "/api/ls?path=..",
        "/api/ls?path=g/../..",
        "/api/ls?path=%2Fetc",
        "/api/ls?path=g%2F..%2F..",
        "/img?path=../secret.png",
        "/thumb?path=%2Fetc%2Fpasswd",
    ] {
        let r = get(&f, p);
        assert!(
            matches!(r.status, 400 | 403),
            "{p} -> {} {}",
            r.status,
            r.text()
        );
    }
    #[cfg(unix)]
    {
        assert_eq!(get(&f, "/api/ls?path=escape").status, 403);
        assert_eq!(get(&f, "/img?path=escape/secret.png").status, 403);
        assert_eq!(get(&f, "/img?path=g/link.png").status, 403);
        assert_eq!(get(&f, "/compare?runs=g/a,escape").status, 403);
    }
    assert_eq!(get(&f, "/img?path=g/a/flipdiff-meta.json").status, 403);
    assert_eq!(get(&f, "/img?path=g/a/x.png").status, 200);
    assert_eq!(
        get(&f, &format!("/session/{}/index.html", "0".repeat(32))).status,
        302
    );
    assert_eq!(
        get(&f, &format!("/session/{}/blind-key.json", "0".repeat(32))).status,
        302
    );
}

#[test]
fn rejects_wrong_host_origin_and_missing_token() {
    let f = fixture(1024);
    let port = f.handle.port().to_string();
    let good_origin = format!("http://127.0.0.1:{port}");
    let up = "/api/upload?upload=00112233445566778899aabbccddeeff&side=a&name=x.png";
    assert_eq!(
        http(&f, "GET", "/api/ls", &[("Host", "evil.example")], b"").status,
        403
    );
    assert_eq!(
        http(&f, "GET", "/api/ls", &[("Host", "127.0.0.1:1")], b"").status,
        403
    );
    let tok = f.handle.token();
    let host_good = |h: &str| http(&f, "GET", "/api/ls", &[("Host", h)], b"").status;
    assert_eq!(host_good(&format!("localhost:{port}")), 200);
    // POST: wrong host, wrong or missing Origin, wrong or missing token.
    assert_eq!(
        http(
            &f,
            "POST",
            up,
            &[
                ("Host", "evil.example"),
                ("Origin", &good_origin),
                ("X-Flipdiff-Token", tok)
            ],
            b"x"
        )
        .status,
        403
    );
    assert_eq!(
        http(
            &f,
            "POST",
            up,
            &[("Origin", "http://evil.example"), ("X-Flipdiff-Token", tok)],
            b"x"
        )
        .status,
        403
    );
    assert_eq!(
        http(&f, "POST", up, &[("X-Flipdiff-Token", tok)], b"x").status,
        403
    );
    assert_eq!(
        http(&f, "POST", up, &[("Origin", &good_origin)], b"x").status,
        403
    );
    assert_eq!(
        http(
            &f,
            "POST",
            up,
            &[("Origin", &good_origin), ("X-Flipdiff-Token", "nope")],
            b"x"
        )
        .status,
        403
    );
    assert_eq!(post(&f, up, b"x").status, 200);
}

#[test]
fn ls_lists_one_level_lazily() {
    let f = fixture(1024);
    let root: serde_json::Value = serde_json::from_str(&get(&f, "/api/ls?path=").text()).unwrap();
    let names: Vec<&str> = root["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"g"));
    assert!(root["run"].is_null());
    let g: serde_json::Value = serde_json::from_str(&get(&f, "/api/ls?path=g").text()).unwrap();
    let gdirs = g["dirs"].as_array().unwrap();
    assert_eq!(gdirs.len(), 3);
    assert!(gdirs.iter().all(|d| d["has_images"] == true));
    let a: serde_json::Value = serde_json::from_str(&get(&f, "/api/ls?path=g/a").text()).unwrap();
    assert_eq!(a["run"]["images"], 1);
    assert_eq!(a["run"]["meta"]["mode"], "Probe");
    assert!(a["dirs"].as_array().unwrap().is_empty());
    let found: serde_json::Value =
        serde_json::from_str(&get(&f, "/api/search?path=&meta=mode%3DProbe").text()).unwrap();
    assert_eq!(found["runs"].as_array().unwrap().len(), 1);
}

#[test]
fn compare_session_builds_and_is_reused() {
    let f = fixture(1024);
    let r1 = get(&f, "/compare?runs=g/a,g/b,g/c");
    assert_eq!(r1.status, 302);
    let id = session_of(&r1);
    wait_ready(&f, &id);
    let page = get(&f, &format!("/session/{id}/"));
    assert_eq!(page.status, 200);
    assert!(page.text().contains("window.FLIPDIFF_SERVE="));
    assert!(page.text().contains(f.handle.token()));
    let built = std::fs::metadata(f.cache.join("sessions").join(&id).join("index.html"))
        .unwrap()
        .modified()
        .unwrap();
    let id2 = session_of(&get(&f, "/compare?runs=g/a,g/b,g/c"));
    assert_eq!(id, id2);
    wait_ready(&f, &id2);
    let again = std::fs::metadata(f.cache.join("sessions").join(&id).join("index.html"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(built, again, "unchanged inputs must not rebuild");
    // A changed input yields a new session.
    write_png(&f.root.join("g/b/x.png"), [1, 2, 3]);
    assert_ne!(id, session_of(&get(&f, "/compare?runs=g/a,g/b,g/c")));
    assert!(get(&f, &format!("/session/{id}/images/x.png/pane0.png")).status == 200);
    assert_eq!(get(&f, &format!("/session/{id}/session.json")).status, 404);
}

#[test]
fn decisions_post_writes_outside_root() {
    let f = fixture(1024);
    let id = session_of(&get(&f, "/compare?runs=g/a,g/b"));
    wait_ready(&f, &id);
    let before = walkdir::WalkDir::new(&f.root).into_iter().flatten().count();
    let body = serde_json::json!({
        "schema": "flipdiff-decisions.v1", "seed": 1, "labels": ["a", "b"], "blind": false,
        "sets": [{"name": "x.png", "decision": "accept", "chosen_label": null, "no_difference": false, "note": "ok", "roi": null, "timestamp_ms": 5}]
    })
    .to_string();
    let r = post(&f, &format!("/api/session/{id}/decisions"), body.as_bytes());
    assert_eq!(r.status, 200, "{}", r.text());
    let file = f.decisions.join(format!("{id}.flipdiff-decisions.v1.json"));
    let saved = crate::view::read_decisions(&file).unwrap();
    assert_eq!(saved.sets[0].name, "x.png");
    assert_eq!(
        walkdir::WalkDir::new(&f.root).into_iter().flatten().count(),
        before
    );
    assert!(get(&f, "/api/decisions").text().contains(&id));
    assert_eq!(
        post(&f, &format!("/api/session/{id}/decisions"), b"{}").status,
        400
    );
    assert_eq!(
        post(&f, "/api/session/nothex/decisions", body.as_bytes()).status,
        404
    );
}

#[test]
fn upload_over_the_cap_is_rejected() {
    let f = fixture(1000);
    let up = "/api/upload?upload=00112233445566778899aabbccddeeff&side=a&name=big.png";
    assert_eq!(post(&f, up, &vec![0u8; 1001]).status, 413);
    assert_eq!(post(&f, up, &vec![0u8; 1000]).status, 200);
    assert!(
        post(
            &f,
            "/api/upload?upload=00112233445566778899aabbccddeeff&side=a&name=../x.png",
            b"x"
        )
        .status
            == 400
    );
    assert!(
        !f.cache
            .join("uploads/00112233445566778899aabbccddeeff/a/big.png")
            .to_string_lossy()
            .is_empty()
    );
    assert!(!f.root.join("big.png").exists());
}
