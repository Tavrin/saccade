//! Request routing and the security gate of `flipdiff serve`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::browse::{
    PathError, SearchQuery, hash128, is_image_name, list, rel_of, resolve_under, search, thumbnail,
};
use super::session::{
    self, Spec, decisions_path, ensure, is_built, is_session_id, recent_decisions, session_dir,
    stage_pair,
};
use super::{State, ct_eq};
use crate::view::{DECISIONS_SCHEMA, Decisions, MAX_DIRS, MIN_DIRS, is_safe_name};

const PAGE: &str = include_str!("../../assets/serve.html");
const CSS: &str = include_str!("../../assets/serve.css");
const JS: &str = include_str!("../../assets/serve.js");

/// Largest decisions body accepted.
const MAX_DECISIONS_BYTES: u64 = 8 * 1024 * 1024;
/// Longest path or name accepted from a client.
const MAX_NAME_LEN: usize = 512;
const CSP: &str = "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'unsafe-inline'; \
img-src 'self' data: blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

enum Body {
    Bytes(Vec<u8>),
    File(std::fs::File, u64),
}

struct Resp {
    status: u16,
    ctype: &'static str,
    cache: &'static str,
    location: Option<String>,
    html: bool,
    body: Body,
}

impl Resp {
    fn bytes(status: u16, ctype: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            ctype,
            cache: "no-store",
            location: None,
            html: false,
            body: Body::Bytes(body),
        }
    }

    fn html(body: String) -> Self {
        Self {
            html: true,
            ..Self::bytes(200, "text/html; charset=utf-8", body.into_bytes())
        }
    }

    fn json<T: Serialize>(value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(v) => Self::bytes(200, "application/json", v),
            Err(_) => Self::text(500, "serialisation failed"),
        }
    }

    fn text(status: u16, msg: &str) -> Self {
        Self::bytes(
            status,
            "text/plain; charset=utf-8",
            format!("{msg}\n").into_bytes(),
        )
    }

    fn error(status: u16, msg: &str) -> Self {
        let body = serde_json::json!({ "error": msg }).to_string().into_bytes();
        Self::bytes(status, "application/json", body)
    }

    fn redirect(to: &str) -> Self {
        Self {
            location: Some(to.to_owned()),
            ..Self::text(302, "redirecting")
        }
    }

    fn file(path: &std::path::Path, ctype: &'static str, cache: &'static str) -> Self {
        match std::fs::File::open(path).and_then(|f| f.metadata().map(|m| (f, m.len()))) {
            Ok((f, len)) => Self {
                status: 200,
                ctype,
                cache,
                location: None,
                html: false,
                body: Body::File(f, len),
            },
            Err(_) => Self::text(404, "not found"),
        }
    }
}

fn path_error(e: &PathError) -> Resp {
    match e {
        PathError::Invalid => Resp::error(400, "invalid path"),
        PathError::Escapes => Resp::error(403, "path escapes the archive root"),
        PathError::Missing => Resp::error(404, "no such path"),
    }
}

fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

fn send(req: Request, r: Resp) {
    let mut headers: Vec<Header> = [
        header("Content-Type", r.ctype),
        header("Cache-Control", r.cache),
        header("X-Content-Type-Options", "nosniff"),
        header("Referrer-Policy", "no-referrer"),
        header("Cross-Origin-Resource-Policy", "same-origin"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if r.html {
        headers.extend(header("Content-Security-Policy", CSP));
        headers.extend(header("X-Frame-Options", "DENY"));
    }
    if let Some(loc) = &r.location {
        headers.extend(header("Location", loc));
    }
    let status = StatusCode(r.status);
    let _ = match r.body {
        Body::Bytes(v) => {
            let len = v.len();
            let reader: Box<dyn Read + Send> = Box::new(std::io::Cursor::new(v));
            req.respond(Response::new(status, headers, reader, Some(len), None))
        }
        Body::File(f, len) => {
            let reader: Box<dyn Read + Send> = Box::new(f);
            req.respond(Response::new(
                status,
                headers,
                reader,
                usize::try_from(len).ok(),
                None,
            ))
        }
    };
}

fn hdr(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_owned())
}

fn pct_decode(s: &str, plus_is_space: bool) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(v) = hex {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' if plus_is_space => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (pct_decode(k, true), pct_decode(v, true))
        })
        .collect()
}

/// Host, Origin and fetch-metadata checks; `Err` is the rejection.
fn gate(state: &State, req: &Request, post: bool) -> Result<(), Resp> {
    let port = state.port;
    let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let host_ok =
        hdr(req, "Host").is_some_and(|h| hosts.iter().any(|a| a.eq_ignore_ascii_case(&h)));
    if !host_ok {
        return Err(Resp::text(403, "forbidden: unexpected Host header"));
    }
    if hdr(req, "Sec-Fetch-Site").as_deref() == Some("cross-site")
        && hdr(req, "Sec-Fetch-Mode").as_deref() != Some("navigate")
    {
        return Err(Resp::text(403, "forbidden: cross-site request"));
    }
    if post {
        let origins = [
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ];
        let origin_ok =
            hdr(req, "Origin").is_some_and(|o| origins.iter().any(|a| a.eq_ignore_ascii_case(&o)));
        if !origin_ok {
            return Err(Resp::text(403, "forbidden: unexpected Origin header"));
        }
        let token_ok = hdr(req, "X-Flipdiff-Token").is_some_and(|t| ct_eq(&t, &state.token));
        if !token_ok {
            return Err(Resp::text(403, "forbidden: missing or wrong token"));
        }
    }
    Ok(())
}

/// Handles one request.
pub(crate) fn handle(state: &Arc<State>, mut req: Request) {
    let post = match req.method() {
        Method::Get => false,
        Method::Post => true,
        _ => {
            send(req, Resp::text(405, "method not allowed"));
            return;
        }
    };
    if let Err(r) = gate(state, &req, post) {
        send(req, r);
        return;
    }
    let url = req.url().to_owned();
    let (raw_path, raw_query) = url.split_once('?').unwrap_or((&url, ""));
    let path = pct_decode(raw_path, false);
    let query = parse_query(raw_query);
    let resp = if post {
        route_post(state, &mut req, &path, &query)
    } else {
        route_get(state, &path, &query)
    };
    send(req, resp);
}

fn page(state: &State, mode: &str, session: Option<&str>) -> Resp {
    let data = serde_json::json!({ "token": state.token, "mode": mode, "session": session });
    let html = PAGE
        .replace("/*__SERVE_CSS__*/", CSS)
        .replace("/*__SERVE_JS__*/", JS)
        .replace("__SERVE_PAGE__", &data.to_string());
    Resp::html(html)
}

fn route_get(state: &Arc<State>, path: &str, q: &HashMap<String, String>) -> Resp {
    let get = |k: &str| q.get(k).map_or("", String::as_str);
    match path {
        "/" | "/index.html" => page(state, "landing", None),
        "/api/ls" => match list(state, get("path")) {
            Ok(l) => Resp::json(&l),
            Err(e) => path_error(&e),
        },
        "/api/search" => api_search(state, q),
        "/favicon.ico" => Resp::bytes(204, "image/x-icon", Vec::new()),
        "/api/decisions" => Resp::json(&recent_decisions(state, 30)),
        "/img" => image_file(state, get("path"), false),
        "/thumb" => image_file(state, get("path"), true),
        "/compare" => compare(state, q),
        "/pair" => pair(state, get("a"), get("b")),
        p => {
            if let Some(rest) = p.strip_prefix("/progress/") {
                return if is_session_id(rest) {
                    page(state, "progress", Some(rest))
                } else {
                    Resp::text(404, "not found")
                };
            }
            if let Some(rest) = p.strip_prefix("/api/session/") {
                return match rest.strip_suffix("/status") {
                    Some(id) if is_session_id(id) => match session::status(state, id) {
                        Some(s) => Resp::json(&s),
                        None => Resp::error(404, "unknown session"),
                    },
                    _ => Resp::text(404, "not found"),
                };
            }
            if let Some(rest) = p.strip_prefix("/session/") {
                return session_file(state, rest);
            }
            Resp::text(404, "not found")
        }
    }
}

fn api_search(state: &State, q: &HashMap<String, String>) -> Resp {
    let get = |k: &str| q.get(k).map_or("", String::as_str);
    let meta = Some(get("meta"))
        .filter(|m| !m.is_empty())
        .map(|m| match m.split_once('=') {
            Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
            None => (m.to_owned(), None),
        });
    let budget = get("budget_ms")
        .parse::<u64>()
        .unwrap_or(3000)
        .clamp(200, 10_000);
    let query = SearchQuery {
        text: get("q").chars().take(MAX_NAME_LEN).collect(),
        meta,
        recent: get("recent") == "1",
        budget: Duration::from_millis(budget),
    };
    match search(state, get("path"), &query) {
        Ok(r) => Resp::json(&r),
        Err(e) => path_error(&e),
    }
}

fn image_ctype(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Resolves a client path to an image file inside the root.
fn resolve_image(state: &State, rel: &str) -> Result<PathBuf, Resp> {
    if rel.len() > MAX_NAME_LEN {
        return Err(Resp::error(400, "path too long"));
    }
    let abs = resolve_under(&state.root, rel).map_err(|e| path_error(&e))?;
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !abs.is_file() || !is_image_name(&name) || !is_image_name(rel) {
        return Err(Resp::error(403, "only image files are served"));
    }
    Ok(abs)
}

fn image_file(state: &State, rel: &str, thumb: bool) -> Resp {
    let abs = match resolve_image(state, rel) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if thumb {
        match thumbnail(state, &abs) {
            Ok(p) => Resp::file(&p, "image/png", "private, max-age=3600"),
            Err(e) => Resp::error(422, &e),
        }
    } else {
        Resp::file(&abs, image_ctype(&abs), "no-cache")
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

fn to_progress(id: &str) -> Resp {
    Resp::redirect(&format!("/progress/{id}"))
}

fn compare(state: &Arc<State>, q: &HashMap<String, String>) -> Resp {
    let runs = split_list(q.get("runs").map_or("", String::as_str));
    if !(MIN_DIRS..=MAX_DIRS).contains(&runs.len()) {
        return Resp::text(
            400,
            &format!("runs= takes {MIN_DIRS} to {MAX_DIRS} comma-separated run paths"),
        );
    }
    let labels = q
        .get("labels")
        .filter(|l| !l.is_empty())
        .map(|l| split_list(l));
    if labels.as_ref().is_some_and(|l| l.len() != runs.len()) {
        return Resp::text(400, "labels= must have one label per run");
    }
    let mut dirs = Vec::new();
    for r in &runs {
        match resolve_under(&state.root, r) {
            Ok(d) if d.is_dir() => dirs.push(d),
            Ok(_) => return Resp::text(400, &format!("{r}: not a directory")),
            Err(PathError::Invalid) => return Resp::text(400, &format!("{r}: invalid path")),
            Err(PathError::Escapes) => {
                return Resp::text(403, &format!("{r}: escapes the archive root"));
            }
            Err(PathError::Missing) => return Resp::text(404, &format!("{r}: no such run")),
        }
    }
    let blind = q.get("blind").is_some_and(|b| b == "1" || b == "true");
    let runs = dirs.iter().map(|d| rel_of(&state.root, d)).collect();
    let id = ensure(
        state,
        Spec {
            dirs,
            labels,
            runs,
            blind,
        },
    );
    to_progress(&id)
}

fn pair(state: &Arc<State>, a: &str, b: &str) -> Resp {
    let (pa, pb) = match (resolve_image(state, a), resolve_image(state, b)) {
        (Ok(pa), Ok(pb)) => (pa, pb),
        (Err(r), _) | (_, Err(r)) => return r,
    };
    let meta = |p: &PathBuf| {
        std::fs::metadata(p)
            .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
            .unwrap_or_default()
    };
    let key = hash128(&[
        pa.to_string_lossy().as_bytes(),
        meta(&pa).as_bytes(),
        pb.to_string_lossy().as_bytes(),
        meta(&pb).as_bytes(),
    ]);
    let dest = state.cache.join("pairs").join(&key);
    if !dest.join("b").is_dir() {
        let tmp = state
            .cache
            .join("pairs")
            .join(format!(".tmp-{key}-{}", session::unique()));
        if let Err(e) = stage_pair(&pa, &pb, &tmp) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Resp::text(422, &e);
        }
        let _ = std::fs::remove_dir_all(&dest);
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Resp::text(500, &format!("staging the pair: {e}"));
        }
    }
    let (ra, rb) = (rel_of(&state.root, &pa), rel_of(&state.root, &pb));
    let labels = if ra == rb {
        vec!["a".to_owned(), "b".to_owned()]
    } else {
        vec![ra.clone(), rb.clone()]
    };
    let spec = Spec {
        dirs: vec![dest.join("a"), dest.join("b")],
        labels: Some(labels),
        runs: vec![ra, rb],
        blind: false,
    };
    to_progress(&ensure(state, spec))
}

fn allowed_session_file(rest: &str) -> bool {
    let Some(file) = rest.strip_prefix("images/") else {
        return false;
    };
    let ext = std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    is_safe_name(file)
        && matches!(
            ext.as_deref(),
            Some("png" | "jpg" | "jpeg" | "exr" | "hdr" | "js")
        )
}

fn session_file(state: &State, rest: &str) -> Resp {
    let (id, file) = rest.split_once('/').unwrap_or((rest, ""));
    if !is_session_id(id) {
        return Resp::text(404, "not found");
    }
    if file.is_empty() && !rest.contains('/') {
        return Resp::redirect(&format!("/session/{id}/"));
    }
    if !is_built(state, id) {
        return Resp::redirect(&format!("/progress/{id}"));
    }
    let dir = session_dir(state, id);
    if file.is_empty() || file == "index.html" {
        let Ok(html) = std::fs::read_to_string(dir.join("index.html")) else {
            return Resp::text(404, "not found");
        };
        let data = serde_json::json!({ "token": state.token, "session": id }).to_string();
        let inject = format!("<script>window.FLIPDIFF_SERVE={data};</script>");
        let html = html.replacen("</head>", &format!("{inject}</head>"), 1);
        return Resp::html(html);
    }
    if !allowed_session_file(file) {
        return Resp::text(404, "not found");
    }
    let Ok(canon_dir) = dir.canonicalize() else {
        return Resp::text(404, "not found");
    };
    match resolve_under(&canon_dir, file) {
        Ok(p) if p.is_file() => {
            let ctype = if p.extension().is_some_and(|e| e == "js") {
                "text/javascript; charset=utf-8"
            } else {
                image_ctype(&p)
            };
            Resp::file(&p, ctype, "private, max-age=3600")
        }
        _ => Resp::text(404, "not found"),
    }
}

/// Reads at most `cap` bytes of the body. `Err` is the rejection; an
/// over-long body is drained (bounded) first so the client sees the response.
fn read_body(req: &mut Request, cap: u64) -> Result<Vec<u8>, Resp> {
    if let Some(len) = req.body_length() {
        if len as u64 > cap {
            if (len as u64) <= cap.saturating_mul(4).max(1 << 20) {
                let _ = std::io::copy(&mut req.as_reader().take(len as u64), &mut std::io::sink());
            }
            return Err(Resp::error(413, "body exceeds the size cap"));
        }
    }
    let mut buf = Vec::new();
    if req.as_reader().take(cap + 1).read_to_end(&mut buf).is_err() {
        return Err(Resp::error(400, "could not read the request body"));
    }
    if buf.len() as u64 > cap {
        return Err(Resp::error(413, "body exceeds the size cap"));
    }
    Ok(buf)
}

fn route_post(
    state: &Arc<State>,
    req: &mut Request,
    path: &str,
    q: &HashMap<String, String>,
) -> Resp {
    if let Some(id) = path
        .strip_prefix("/api/session/")
        .and_then(|r| r.strip_suffix("/decisions"))
    {
        return post_decisions(state, req, id);
    }
    match path {
        "/api/upload" => post_upload(state, req, q),
        "/api/upload/open" => post_upload_open(state, q),
        _ => Resp::text(404, "not found"),
    }
}

fn post_decisions(state: &State, req: &mut Request, id: &str) -> Resp {
    if !is_session_id(id) {
        return Resp::error(404, "unknown session");
    }
    let known = is_built(state, id) || state.sessions.lock().is_ok_and(|m| m.contains_key(id));
    if !known {
        return Resp::error(404, "unknown session");
    }
    let body = match read_body(req, MAX_DECISIONS_BYTES) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let decisions: Decisions = match serde_json::from_slice(&body) {
        Ok(d) => d,
        Err(e) => return Resp::error(400, &format!("not a decisions file: {e}")),
    };
    if decisions.schema != DECISIONS_SCHEMA {
        return Resp::error(400, "wrong decisions schema");
    }
    match session::save_decisions(state, id, &decisions) {
        Ok(()) => Resp::json(&serde_json::json!({
            "ok": true,
            "file": decisions_path(state, id).file_name().map(|n| n.to_string_lossy().into_owned()),
        })),
        Err(e) => Resp::error(500, &format!("writing decisions: {e}")),
    }
}

fn is_upload_id(id: &str) -> bool {
    (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn post_upload(state: &State, req: &mut Request, q: &HashMap<String, String>) -> Resp {
    let get = |k: &str| q.get(k).map_or("", String::as_str);
    let (id, side, name) = (get("upload"), get("side"), get("name"));
    if !is_upload_id(id) || !matches!(side, "a" | "b") {
        return Resp::error(400, "upload= must be a hex id and side= a or b");
    }
    if name.len() > MAX_NAME_LEN || !is_safe_name(name) || !is_image_name(name) {
        return Resp::error(400, "name= must be a plain relative image path");
    }
    let body = match read_body(req, state.max_upload) {
        Ok(b) => b,
        Err(r) => return r,
    };
    let dest = state.cache.join("uploads").join(id).join(side).join(name);
    let write = dest
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::File::create(&dest))
        .and_then(|mut f| f.write_all(&body));
    match write {
        Ok(()) => Resp::json(&serde_json::json!({ "ok": true, "bytes": body.len() })),
        Err(e) => Resp::error(500, &format!("storing the upload: {e}")),
    }
}

/// Image files below `dir`, as `/`-separated relative names.
fn upload_files(dir: &std::path::Path) -> Vec<String> {
    walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| rel_of(dir, e.path()))
        .collect()
}

fn post_upload_open(state: &Arc<State>, q: &HashMap<String, String>) -> Resp {
    let id = q.get("upload").map_or("", String::as_str);
    if !is_upload_id(id) {
        return Resp::error(400, "upload= must be a hex id");
    }
    let root = state.cache.join("uploads").join(id);
    let (da, db) = (root.join("a"), root.join("b"));
    let (fa, fb) = (upload_files(&da), upload_files(&db));
    if fa.is_empty() || fb.is_empty() {
        return Resp::error(400, "both sides need at least one image");
    }
    let labels = q
        .get("labels")
        .filter(|l| !l.is_empty())
        .map(|l| split_list(l));
    if labels.as_ref().is_some_and(|l| l.len() != 2) {
        return Resp::error(400, "labels= must have two labels");
    }
    let (dirs, runs) = if fa.len() == 1 && fb.len() == 1 && fa != fb {
        // Two single images with different names: pair them under one name.
        let staged = root.join("pair");
        let _ = std::fs::remove_dir_all(&staged);
        match stage_pair(&da.join(&fa[0]), &db.join(&fb[0]), &staged) {
            Ok((x, y)) => (vec![x, y], vec![fa[0].clone(), fb[0].clone()]),
            Err(e) => return Resp::error(422, &e),
        }
    } else {
        (
            vec![da, db],
            vec!["upload a".to_owned(), "upload b".to_owned()],
        )
    };
    let blind = q.get("blind").is_some_and(|b| b == "1");
    let sid = ensure(
        state,
        Spec {
            dirs,
            labels,
            runs,
            blind,
        },
    );
    Resp::json(&serde_json::json!({ "session": sid, "url": format!("/progress/{sid}") }))
}
