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
    let mut body = raw[split + 4..].to_vec();
    if head.to_lowercase().contains("transfer-encoding: chunked") {
        body = dechunk(&body);
    }
    Reply { status, head, body }
}

/// Decodes an HTTP/1.1 chunked body.
fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(eol) = rest.windows(2).position(|w| w == b"\r\n") {
        let size =
            usize::from_str_radix(String::from_utf8_lossy(&rest[..eol]).trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        out.extend_from_slice(&rest[eol + 2..eol + 2 + size]);
        rest = &rest[eol + 2 + size + 2..];
    }
    out
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
        .split('?')
        .next()
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
    assert!(page.text().contains(r#""browse":"/#/g""#));
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
    assert!(get(&f, &format!("/session/{id}/images/x.png.d/pane0.png")).status == 200);
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

#[test]
fn bad_deep_link_gets_a_styled_page_with_a_way_back() {
    let f = fixture(1024);
    for path in [
        "/compare?runs=g/a,nope/missing",
        "/session/not-a-session/",
        "/no-such-page",
    ] {
        let r = get(&f, path);
        assert!(r.status == 404, "{path}: {}", r.status);
        assert!(r.head.to_lowercase().contains("text/html"), "{path}");
        let t = r.text();
        assert!(
            t.contains("href=\"/\"") && t.contains("Back to the archive"),
            "{path}"
        );
    }
}

/// A server over `roots`, owning the temporary directory they live in.
fn serve_roots(tmp: tempfile::TempDir, roots: &[PathBuf], follow: bool) -> Fixture {
    let (cache, decisions) = (tmp.path().join("cache"), tmp.path().join("decisions"));
    let mut opts = ServeOptions::new(roots[0].clone());
    opts.extra_roots = roots[1..].to_vec();
    opts.follow_symlinks_within_roots = follow;
    opts.cache_dir = cache.clone();
    opts.decisions_dir = decisions.clone();
    let handle = start(opts).unwrap();
    Fixture {
        root: roots[0].canonicalize().unwrap(),
        cache: cache.canonicalize().unwrap(),
        decisions: decisions.canonicalize().unwrap(),
        handle,
        _tmp: tmp,
    }
}

fn json(r: &Reply) -> serde_json::Value {
    serde_json::from_str(&r.text()).unwrap()
}

#[test]
fn single_images_compare_from_anywhere() {
    let f = fixture(1024);
    let listing = json(&get(&f, "/api/images?path=g/a"));
    assert_eq!(listing["images"][0]["name"], "x.png");
    // Images of three different runs, staged under one name.
    let r = get(
        &f,
        "/compare?runs=g/a/x.png,g/b/x.png,g/c/x.png&set=image.png",
    );
    assert_eq!(r.status, 302);
    assert!(r.head.contains("set=image.png"), "{}", r.head);
    let id = session_of(&r);
    wait_ready(&f, &id);
    let base = format!("/session/{id}/images/image.png.d");
    assert_eq!(get(&f, &format!("{base}/pane2.png")).status, 200);
    assert_eq!(get(&f, &format!("{base}/heatmap1.png")).status, 200);
    // Runs and images do not mix; a non-image file is refused.
    assert_eq!(get(&f, "/compare?runs=g/a,g/b/x.png").status, 400);
    assert_eq!(
        get(&f, "/compare?runs=g/a/x.png,g/a/flipdiff-meta.json").status,
        403
    );
}

#[test]
fn several_roots_are_top_level_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("one/data"), tmp.path().join("two/data"));
    write_png(&a.join("r1/x.png"), [1, 1, 1]);
    write_png(&b.join("r2/x.png"), [9, 9, 9]);
    let f = serve_roots(tmp, &[a, b], false);
    let top = json(&get(&f, "/api/ls?path="));
    let names: Vec<&str> = top["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["data", "data-2"], "equal names are told apart");
    assert_eq!(top["multi"], true);
    assert_eq!(top["dirs"][0]["has_images"], false);
    assert_eq!(json(&get(&f, "/api/ls?path=data-2/r2"))["run"]["images"], 1);
    assert_eq!(get(&f, "/img?path=data-2/r2/x.png").status, 200);
    assert_eq!(get(&f, "/api/ls?path=nope").status, 404);
    assert_eq!(get(&f, "/api/ls?path=data/..").status, 400);
    // A run from each root compares.
    assert_eq!(get(&f, "/compare?runs=data/r1,data-2/r2").status, 302);
    let found = json(&get(&f, "/api/search?path="));
    assert_eq!(found["runs"].as_array().unwrap().len(), 2);
}

#[cfg(unix)]
#[test]
fn symlinks_resolving_inside_any_root_are_followed_and_others_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b, other) = (
        tmp.path().join("a"),
        tmp.path().join("b"),
        tmp.path().join("other"),
    );
    write_png(&a.join("real/x.png"), [5, 5, 5]);
    write_png(&other.join("y.png"), [6, 6, 6]);
    std::fs::create_dir_all(&b).unwrap();
    std::os::unix::fs::symlink(a.join("real"), b.join("link")).unwrap();
    std::os::unix::fs::symlink(&other, b.join("outside")).unwrap();
    let roots = [a.clone(), b.clone()];

    let with = serve_roots(tempfile::tempdir().unwrap(), &roots, true);
    let ls = json(&get(&with, "/api/ls?path=b"));
    let listed: Vec<&str> = ls["dirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        listed,
        ["link"],
        "the link into root a is listed, the outside one is not"
    );
    assert_eq!(ls["outside_links"], 1);
    assert_eq!(get(&with, "/api/ls?path=b/link").status, 200);
    assert_eq!(get(&with, "/img?path=b/link/x.png").status, 200);
    assert_eq!(get(&with, "/api/ls?path=b/outside").status, 403);
    assert_eq!(get(&with, "/img?path=b/outside/y.png").status, 403);
    assert_eq!(get(&with, "/compare?runs=a/real,b/outside").status, 403);

    // Without the flag, a link into another root is refused as well.
    let without = serve_roots(tempfile::tempdir().unwrap(), &roots, false);
    assert_eq!(get(&without, "/api/ls?path=b/link").status, 403);
    assert_eq!(get(&without, "/img?path=b/link/x.png").status, 403);
}

#[test]
fn api_runs_returns_the_overview_model() {
    let f = fixture(1024);
    let q = "ref=g/a&runs=g/b,g/c";
    let mut model = json(&get(&f, &format!("/api/runs?{q}")));
    for _ in 0..200 {
        if model["progress"]["complete"] == true {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        model = json(&get(&f, &format!("/api/runs?{q}")));
    }
    assert_eq!(model["schema"], "flipdiff-runs.v1");
    assert_eq!(model["progress"]["total"], 2);
    assert_eq!(model["progress"]["complete"], true);
    assert_eq!(model["ref"]["label"], "a");
    assert_eq!(model["runs"][0]["label"], "b");
    assert_eq!(model["runs"][0]["changed"], 1);
    assert_eq!(model["runs"][0]["no_visible_effect"], false);
    let cell = &model["images"][0]["cells"][1];
    assert_eq!(cell["status"], "changed");
    assert!(cell["metrics"]["mean"].as_f64().unwrap() > 0.0);
    assert!(
        cell["thumb"]
            .as_str()
            .unwrap()
            .starts_with("/thumb?path=g%2Fc%2Fx.png")
    );
    let heat = cell["heat"].as_str().unwrap();
    assert_eq!(get(&f, heat).status, 200, "{heat}");
    // The page, and refusals that follow the path rules.
    let page = get(&f, &format!("/runs?{q}"));
    assert_eq!(page.status, 200);
    assert!(page.text().contains("FLIPDIFF_RUNS"));
    #[cfg(unix)]
    assert_eq!(get(&f, "/api/runs?ref=g/a&runs=escape").status, 403);
    assert_eq!(get(&f, "/api/runs?ref=g/a&runs=g/nope").status, 404);
    assert_eq!(get(&f, "/api/runs?ref=g/a").status, 400);
}
