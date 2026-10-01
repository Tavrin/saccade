//! HTTP-level tests of `saccade serve` against an ephemeral port.

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
    std::fs::write(root.join("g/a/saccade-meta.json"), r#"{"mode":"Probe"}"#).unwrap();
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
        &[("Origin", &origin), ("X-Saccade-Token", f.handle.token())],
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
    assert_eq!(get(&f, "/img?path=g/a/saccade-meta.json").status, 403);
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
                ("X-Saccade-Token", tok)
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
            &[("Origin", "http://evil.example"), ("X-Saccade-Token", tok)],
            b"x"
        )
        .status,
        403
    );
    assert_eq!(
        http(&f, "POST", up, &[("X-Saccade-Token", tok)], b"x").status,
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
            &[("Origin", &good_origin), ("X-Saccade-Token", "nope")],
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
    assert!(page.text().contains("window.SACCADE_SERVE="));
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
        "schema": "saccade-decisions.v1", "seed": 1, "labels": ["a", "b"], "blind": false,
        "sets": [{"name": "x.png", "decision": "accept", "chosen_label": null, "no_difference": false, "note": "ok", "roi": null, "timestamp_ms": 5}]
    })
    .to_string();
    let r = post(&f, &format!("/api/session/{id}/decisions"), body.as_bytes());
    assert_eq!(r.status, 200, "{}", r.text());
    let file = f.decisions.join(format!("{id}.saccade-decisions.v1.json"));
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
        get(&f, "/compare?runs=g/a/x.png,g/a/saccade-meta.json").status,
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
    assert_eq!(model["schema"], "saccade-runs.v1");
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
    assert!(page.text().contains("SACCADE_RUNS"));
    #[cfg(unix)]
    assert_eq!(get(&f, "/api/runs?ref=g/a&runs=escape").status, 403);
    assert_eq!(get(&f, "/api/runs?ref=g/a&runs=g/nope").status, 404);
    assert_eq!(get(&f, "/api/runs?ref=g/a").status, 400);
}

#[test]
fn deeplink_single_run_and_single_pane_viewer() {
    let f = fixture(1024);
    let r = get(&f, "/compare?run=g%2Fa");
    assert_eq!(r.status, 302);
    assert!(r.head.contains("/run?path=g%2Fa"));
    assert!(
        get(&f, "/compare?runs=g/a")
            .head
            .contains("/run?path=g%2Fa")
    );
    let page = get(&f, "/run?path=g/a");
    assert_eq!(page.status, 200);
    assert!(page.text().contains("Probe") && page.text().contains("x.png"));
    assert!(page.text().contains("/thumb?path=") && page.text().contains("Compare with"));
    let id = session_of(&get(&f, "/image?path=g/a/x.png"));
    wait_ready(&f, &id);
    let page = get(&f, &format!("/session/{id}/"));
    let text = page.text();
    let start = text.find("id=\"saccade-data\"").unwrap();
    let json_start = start + text[start..].find('>').unwrap() + 1;
    let json_end = json_start + text[json_start..].find("</script>").unwrap();
    let model: serde_json::Value = serde_json::from_str(&text[json_start..json_end]).unwrap();
    assert_eq!(model["labels"].as_array().unwrap().len(), 1);
    assert_eq!(model["sets"][0]["panes"].as_array().unwrap().len(), 1);
}

#[test]
fn deeplink_repeated_comma_paths_and_legacy_forms() {
    let f = fixture(1024);
    write_png(&f.root.join("g/a,b/x.png"), [7, 8, 9]);
    let id = session_of(&get(
        &f,
        "/compare?run=g%2Fa%2Cb&run=g%2Fb&runs=nope,missing",
    ));
    wait_ready(&f, &id);
    let info: session::SessionInfo = serde_json::from_str(
        &std::fs::read_to_string(f.cache.join("sessions").join(id).join("session.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(info.runs, ["g/a,b", "g/b"]);
    assert!(info.overview.unwrap().contains("run=g%2Fb"));
    assert_eq!(get(&f, "/compare?runs=g/a,g/b").status, 302);
    for q in [
        "ref=g/a&run=g%2Fa%2Cb&run=g/b&runs=missing",
        "ref=g/a&runs=g/b,g/c",
    ] {
        let page = get(&f, &format!("/runs?{q}"));
        assert_eq!(page.status, 200);
        assert!(page.text().contains("run=g%2F") && !page.text().contains("runs="));
        assert_eq!(get(&f, &format!("/api/runs?{q}")).status, 200);
    }
}

#[test]
fn deeplink_open_two_roots_reference_images_and_outside() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    for dir in [a.join("r,1"), a.join("ref"), b.join("r2")] {
        write_png(&dir.join("x.png"), [3, 4, 5]);
    }
    let outside = tmp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let f = serve_roots(tmp, &[a.clone(), b.clone()], false);
    let enc = overview::pct_encode;
    let run = enc(&a.join("r,1").to_string_lossy());
    let other = enc(&b.join("r2").to_string_lossy());
    let r = get(
        &f,
        &format!(
            "/open?abs={run}&abs={other}&ref={}&blind=1&labels=ref,one,two",
            enc(&a.join("ref").to_string_lossy())
        ),
    );
    assert_eq!(r.status, 302);
    assert!(
        r.head
            .contains("/runs?ref=a%2Fref&run=a%2Fr%2C1&run=b%2Fr2&labels=ref%2Cone%2Ctwo&blind=1"),
        "{}",
        r.head
    );
    let r = get(&f, &format!("/open?abs={run}&abs={other}"));
    assert!(r.head.contains("/compare?run=a%2Fr%2C1&run=b%2Fr2"));
    assert!(
        get(&f, &format!("/open?abs={run}"))
            .head
            .contains("/run?path=a%2Fr%2C1")
    );
    let image_a = enc(&a.join("r,1/x.png").to_string_lossy());
    let image_b = enc(&b.join("r2/x.png").to_string_lossy());
    assert!(
        get(&f, &format!("/open?abs={image_a}"))
            .head
            .contains("/image?path=a%2Fr%2C1%2Fx.png")
    );
    assert!(
        get(&f, &format!("/open?abs={image_a}&abs={image_b}"))
            .head
            .contains("/pair?a=a%2Fr%2C1%2Fx.png&b=b%2Fr2%2Fx.png")
    );
    for path in [
        outside.clone(),
        outside.join("missing"),
        a.join("../outside"),
    ] {
        let r = get(&f, &format!("/open?abs={}", enc(&path.to_string_lossy())));
        assert_eq!(r.status, 404);
        assert!(
            r.text().contains("not found or not under a served root")
                && r.text().contains("Back to the archive")
        );
    }
}

#[test]
fn deeplink_roots_names_and_config() {
    let f = fixture(1024);
    let roots = json(&get(&f, "/api/roots"));
    assert_eq!(roots[0]["name"], "");
    assert_eq!(roots[0]["path"], f.root.to_string_lossy().as_ref());
    let tmp = tempfile::tempdir().unwrap();
    let roots: Vec<PathBuf> = ["a/data", "b/data", "c/data"]
        .iter()
        .map(|r| tmp.path().join(r))
        .collect();
    for root in &roots {
        std::fs::create_dir_all(root).unwrap();
    }
    let multi = serve_roots(tmp, &roots, false);
    let api = json(&get(&multi, "/api/roots"));
    assert_eq!(api[0]["name"], "data");
    assert_eq!(api[1]["name"], "data-2");
    assert_eq!(api[2]["name"], "data-3");
    let cfg = crate::config::RunConfig::from_toml_str(
        "symlink_targets = ['/nas/captures']\nfs_timeout_ms = 40",
    )
    .unwrap();
    assert_eq!(cfg.symlink_targets, [PathBuf::from("/nas/captures")]);
    assert_eq!(cfg.fs_timeout_ms, 40);
    assert!(crate::config::RunConfig::from_toml_str("fs_timeout_ms = 0").is_err());
}

#[cfg(unix)]
#[test]
fn deeplink_allowed_external_symlinks_uniformly_confined() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let target = tmp.path().join("nas");
    let denied = tmp.path().join("denied");
    write_png(&root.join("local/x.png"), [2, 2, 2]);
    write_png(&target.join("remote/x.png"), [8, 8, 8]);
    write_png(&denied.join("x.png"), [9, 9, 9]);
    std::fs::write(target.join("remote/custom.json"), r#"{"host":"nas"}"#).unwrap();
    std::os::unix::fs::symlink(target.join("remote"), root.join("linked,run")).unwrap();
    std::os::unix::fs::symlink(&denied, root.join("denied")).unwrap();
    let mut opts = ServeOptions::new(root.clone());
    opts.symlink_targets = vec![target.clone()];
    opts.view.meta.name = "custom.json".into();
    opts.cache_dir = tmp.path().join("cache");
    opts.decisions_dir = tmp.path().join("decisions");
    let handle = start(opts).unwrap();
    let f = Fixture {
        root: root.clone(),
        cache: tmp.path().join("cache"),
        decisions: tmp.path().join("decisions"),
        handle,
        _tmp: tmp,
    };
    for path in [
        "/api/ls?path=linked%2Crun",
        "/run?path=linked%2Crun",
        "/api/images?path=linked%2Crun",
        "/img?path=linked%2Crun/x.png",
        "/thumb?path=linked%2Crun/x.png",
    ] {
        assert_eq!(get(&f, path).status, 200, "{path}");
    }
    let listing = json(&get(&f, "/api/ls?path=linked%2Crun"));
    std::os::unix::fs::symlink(target.join("remote/custom.json"), root.join("fake.png")).unwrap();
    for route in [
        "/img?path=fake.png",
        "/thumb?path=fake.png",
        "/image?path=fake.png",
        "/compare?run=local/x.png&run=fake.png",
    ] {
        assert_eq!(get(&f, route).status, 403, "{route}");
    }
    assert_eq!(
        json(&get(&f, "/api/images"))["images"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(listing["path"], "linked,run");
    assert_eq!(listing["run"]["meta"]["host"], "nas");
    assert_eq!(
        json(&get(&f, "/api/search"))["runs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let open = format!(
        "/open?abs={}",
        overview::pct_encode(&root.join("linked,run").to_string_lossy())
    );
    assert!(get(&f, &open).head.contains("/run?path=linked%2Crun"));
    assert_eq!(
        get(
            &f,
            &format!(
                "/open?abs={}",
                overview::pct_encode(&target.join("remote").to_string_lossy())
            )
        )
        .status,
        404,
        "allowlisted storage cannot be addressed directly"
    );
    let id = session_of(&get(&f, "/compare?run=local&run=linked%2Crun"));
    wait_ready(&f, &id);
    assert_eq!(get(&f, "/api/runs?ref=local&run=linked%2Crun").status, 200);
    for path in [
        "/api/ls?path=denied",
        "/run?path=denied",
        "/img?path=denied/x.png",
        "/thumb?path=denied/x.png",
        "/compare?run=local&run=denied",
        "/api/runs?ref=local&run=denied",
    ] {
        assert_eq!(get(&f, path).status, 403, "{path}");
    }
}

#[test]
fn deeplink_storage_timeout_and_probe_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    write_png(&root.join("r/x.png"), [1, 2, 3]);
    let mut opts = ServeOptions::new(root.clone());
    opts.cache_dir = tmp.path().join("cache");
    opts.decisions_dir = tmp.path().join("decisions");
    opts.probe_timeout_ms = Some(20);
    opts.probe_delay_ms = 200;
    let handle = start(opts).unwrap();
    let f = Fixture {
        root,
        cache: tmp.path().join("cache"),
        decisions: tmp.path().join("decisions"),
        handle,
        _tmp: tmp,
    };
    let started = std::time::Instant::now();
    for _ in 0..8 {
        let r = get(&f, "/run?path=r");
        assert_eq!(r.status, 503);
        assert!(r.text().contains("storage not reachable: r"));
    }
    assert!(started.elapsed() < Duration::from_millis(800));
    // Eight still-running probes retain every slot; the ninth never executes.
    let r = get(&f, "/img?path=r/x.png");
    assert_eq!(r.status, 503);
    assert_eq!(get(&f, "/").status, 200);
    assert_eq!(get(&f, "/api/roots").status, 200);
    let storage = storage::Storage::new(Duration::from_millis(5));
    let release = Arc::new(std::sync::Barrier::new(9));
    for _ in 0..8 {
        let release = release.clone();
        assert!(
            storage
                .run(move || {
                    release.wait();
                })
                .is_err()
        );
    }
    assert!(storage.run(|| panic!("ninth probe must not run")).is_err());
    release.wait();
}
