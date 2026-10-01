//! Request routing and the security gate of `saccade serve`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::browse::{
    PathError, SearchQuery, THUMB_EDGE, hash128, images, is_image_name, list, rel_in, resolve_in,
    resolve_under, search, thumbnail,
};
use super::overview::{self, pct_encode};
use super::session::{
    self, Spec, decisions_path, ensure, is_built, is_session_id, recent_decisions, session_dir,
    stage_images, stage_named,
};
use super::{State, ct_eq};
use crate::view::{DECISIONS_SCHEMA, Decisions, MAX_DIRS, MIN_DIRS, is_safe_name};

const PAGE: &str = include_str!("../../assets/serve.html");
const CSS: &str = include_str!("../../assets/serve.css");
const JS: &str = include_str!("../../assets/serve.js");
#[path = "inbox_api.rs"]
mod inbox_api;
#[path = "vote_api.rs"]
mod vote_api;
/// Runs in a served session page: the link back to the run overview, and
/// opening the viewer at the set and pair a deep link names.
const SESSION_OPEN_JS: &str = include_str!("../../assets/session-open.js");
/// The script tag of the viewer page that loads the decisions sidecar.
const DECISIONS_TWIN_TAG: &str = "<script src=\"saccade-decisions.v1.js\"></script>";

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
        let body = crate::render::shared::complete_html(body);
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Resp {
    /// A styled error page for a navigation (a deep link that cannot be
    /// served), with a link back to the browser.
    fn error_page(status: u16, title: &str, msg: &str) -> Self {
        let body = format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light dark\"><title>saccade serve: {t}</title><style>{css}</style></head>\
<body><header class=\"top\"><div class=\"brand\"><span class=\"logo\" aria-hidden=\"true\"></span><h1>saccade</h1><span class=\"tag\">serve</span></div></header>\
<main class=\"big\"><h2>{t}</h2><p class=\"err\" role=\"alert\">{m}</p>\
<p><a class=\"btn primary\" href=\"/\">&larr; Back to the archive</a></p></main></body></html>",
            t = html_escape(title),
            m = html_escape(msg),
            css = crate::render::shared::page_css(&[CSS]),
        );
        Self {
            status,
            html: true,
            ..Self::bytes(status, "text/html; charset=utf-8", body.into_bytes())
        }
    }

    /// Restyles a JSON `{"error": ...}` reply as an error page.
    fn into_error_page(self) -> Self {
        let msg = match &self.body {
            Body::Bytes(b) => serde_json::from_slice::<serde_json::Value>(b)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str().map(str::to_owned))),
            Body::File(..) => None,
        };
        match msg {
            Some(m) => Self::error_page(self.status, "This link cannot be opened", &m),
            None => self,
        }
    }
}

/// `%`-encodes a `/`-separated path segment by segment for a URL fragment.
fn encode_path(p: &str) -> String {
    let mut out = String::new();
    for (i, seg) in p.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for b in seg.bytes() {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                out.push(char::from(b));
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// The archive directory the browser should return to for a session: the
/// deepest directory that contains the parents of all its runs.
fn browse_target(state: &State, id: &str) -> String {
    let info: Option<session::SessionInfo> =
        std::fs::read_to_string(session_dir(state, id).join("session.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
    let mut common: Option<Vec<String>> = None;
    for run in info.iter().flat_map(|i| i.runs.iter()) {
        let mut parts: Vec<String> = run.split('/').map(str::to_owned).collect();
        parts.pop();
        common = Some(match common {
            None => parts,
            Some(c) => c
                .into_iter()
                .zip(parts)
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect(),
        });
    }
    let dir = common.unwrap_or_default().join("/");
    if dir.is_empty() {
        "/".to_owned()
    } else {
        format!("/#/{}", encode_path(&dir))
    }
}

/// The query string of the run overview a session belongs to, when it
/// compares whole runs.
fn session_overview(state: &State, id: &str) -> Option<String> {
    let info: session::SessionInfo =
        std::fs::read_to_string(session_dir(state, id).join("session.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())?;
    info.overview.map(|query| parse_query(&query).canonical())
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

pub(crate) struct Query {
    one: HashMap<String, String>,
    repeated: HashMap<String, Vec<String>>,
}
impl std::ops::Deref for Query {
    type Target = HashMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.one
    }
}
impl Query {
    pub fn runs(&self) -> Vec<String> {
        self.repeated
            .get("run")
            .cloned()
            .unwrap_or_else(|| split_list(self.get("runs").map_or("", String::as_str)))
    }
    pub fn values(&self, key: &str) -> Vec<String> {
        self.repeated.get(key).cloned().unwrap_or_default()
    }
    pub fn canonical(&self) -> String {
        let mut keys: Vec<&String> = self
            .keys()
            .filter(|k| matches!(k.as_str(), "ref" | "labels" | "blind") || k.starts_with("pair"))
            .collect();
        keys.sort();
        let mut parts: Vec<String> = keys
            .iter()
            .map(|k| format!("{}={}", pct_encode(k), pct_encode(&self[*k])))
            .collect();
        parts.extend(self.runs().iter().map(|v| format!("run={}", pct_encode(v))));
        parts.join("&")
    }
}

fn parse_query(q: &str) -> Query {
    let pairs: Vec<(String, String)> = q
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (pct_decode(k, true), pct_decode(v, true))
        })
        .collect();
    let mut repeated: HashMap<String, Vec<String>> = HashMap::new();
    for (k, v) in &pairs {
        repeated.entry(k.clone()).or_default().push(v.clone());
    }
    Query {
        one: pairs.into_iter().collect(),
        repeated,
    }
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
        let token_ok = hdr(req, "X-Saccade-Token").is_some_and(|t| ct_eq(&t, &state.token));
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
        if matches!(
            path.as_str(),
            "/" | "/index.html" | "/api/roots" | "/favicon.ico"
        ) || path.starts_with("/progress/")
        {
            route_get(state, &path, &query)
        } else {
            let relative = storage_label(state, &query);
            let worker = state.clone();
            state
                .storage
                .run(move || {
                    let mut response = route_get(&worker, &path, &query);
                    // Read in the probe too: streaming a remote File from `send`
                    // would put an unbounded mount read back on the HTTP worker.
                    if let Body::File(ref mut file, _) = response.body {
                        let mut bytes = Vec::new();
                        if file.read_to_end(&mut bytes).is_err() {
                            return Resp::error_page(
                                503,
                                "Storage not reachable",
                                &format!("storage not reachable: {relative}"),
                            );
                        }
                        response.body = Body::Bytes(bytes);
                    }
                    response
                })
                .unwrap_or_else(|_| {
                    Resp::error_page(
                        503,
                        "Storage not reachable",
                        &format!(
                            "storage not reachable: {}",
                            storage_label(state, &parse_query(raw_query))
                        ),
                    )
                })
        }
    };
    send(req, resp);
}

fn page(state: &State, mode: &str, session: Option<&str>) -> Resp {
    let data = serde_json::json!({ "token": state.token, "mode": mode, "session": session });
    let html = PAGE
        .replace(
            "/*__SERVE_CSS__*/",
            &crate::render::shared::page_css(&[CSS]),
        )
        .replace("/*__SERVE_JS__*/", &crate::render::shared::page_js(&[JS]))
        .replace("__SERVE_PAGE__", &data.to_string());
    Resp::html(html)
}

fn route_get(state: &Arc<State>, path: &str, q: &Query) -> Resp {
    if let Some(response) = inbox_api::get(state, path) {
        return response;
    }
    if let Some(response) = vote_api::get(state, path, q) {
        return response;
    }
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
        "/api/images" => match images(state, get("path")) {
            Ok(l) => Resp::json(&l),
            Err(e) => path_error(&e),
        },
        "/api/runs" => match overview::model_json(state, q) {
            Ok(m) => Resp::json(&m),
            Err((status, msg)) => Resp::error(status, &msg),
        },
        "/runs" => runs_page(state, q),
        "/run" => run_page(state, get("path")),
        "/image" => single_image(state, get("path")),
        "/open" => open_absolute(state, q),
        "/api/roots" => Resp::json(
            &state
                .roots
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "name": if state.multi() { &r.name } else { "" }, "path": r.path,
                    })
                })
                .collect::<Vec<_>>(),
        ),
        "/runs/heat" => heat_file(state, get("key")),
        "/img" => image_file(state, get("path"), None),
        "/thumb" => {
            let edge = get("w")
                .parse::<u32>()
                .ok()
                .filter(|w| *w == crate::runs::PREVIEW_EDGE)
                .unwrap_or(THUMB_EDGE);
            image_file(state, get("path"), Some(edge))
        }
        "/compare" => compare(state, q),
        "/pair" => pair(state, get("a"), get("b"), q),
        p => {
            if let Some(rest) = p.strip_prefix("/progress/") {
                return if is_session_id(rest) {
                    page(state, "progress", Some(rest))
                } else {
                    Resp::error_page(
                        404,
                        "Unknown comparison",
                        "This comparison link is not valid.",
                    )
                };
            }
            if let Some(id) = p
                .strip_prefix("/api/session/")
                .and_then(|r| r.strip_suffix("/decisions"))
                .filter(|id| is_session_id(id))
            {
                // The saved decisions, so the page shows what `saccade decide` recorded.
                return match crate::view::read_decisions(&decisions_path(state, id)) {
                    Ok(mut d) => {
                        crate::paths::rebase_decisions(
                            &mut d,
                            &decisions_path(state, id),
                            &session_dir(state, id).join("index.html"),
                            state.view.record_absolute_paths,
                        );
                        Resp::json(&d)
                    }
                    Err(_) => Resp::error(404, "no decisions saved for this session"),
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
            if p.starts_with("/api/") || p.starts_with("/img") || p.starts_with("/thumb") {
                return Resp::text(404, "not found");
            }
            Resp::error_page(404, "Page not found", "There is nothing at this address.")
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
    let abs = resolve_under(state, rel).map_err(|e| path_error(&e))?;
    let canon = crate::paths::canonicalize(&abs).map_err(|_| path_error(&PathError::Missing))?;
    let root = state
        .root_of(&abs)
        .ok_or_else(|| path_error(&PathError::Escapes))?;
    if !state.allows(&root.path, &canon) {
        return Err(path_error(&PathError::Escapes));
    }
    let name = canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !canon.is_file() || !is_image_name(&name) || !is_image_name(rel) {
        return Err(Resp::error(403, "only image files are served"));
    }
    Ok(canon)
}

/// An image of the archive: the file itself, or its thumbnail when `thumb`
/// carries the longest edge.
fn image_file(state: &State, rel: &str, thumb: Option<u32>) -> Resp {
    let abs = match resolve_image(state, rel) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Some(edge) = thumb {
        match thumbnail(state, &abs, edge) {
            Ok(p) => Resp::file(&p, "image/png", "private, max-age=3600"),
            Err(e) => Resp::error(422, &e),
        }
    } else {
        Resp::file(&abs, image_ctype(&abs), "no-cache")
    }
}

/// The overview page of a run comparison.
fn runs_page(state: &Arc<State>, q: &Query) -> Resp {
    if let Err((status, msg)) = overview::parse_request(state, q) {
        return Resp::error_page(status, "This overview cannot be opened", &msg);
    }
    Resp::html(overview::page_html(state, q, &state.token))
}

/// A cached mini heatmap of the overview.
fn heat_file(state: &State, key: &str) -> Resp {
    if key.len() != 32 || !key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Resp::text(404, "not found");
    }
    Resp::file(
        &state.cache.join("runs").join(format!("{key}.heat.png")),
        "image/png",
        "private, max-age=3600",
    )
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Redirects to the progress page, carrying `set=`, `a=` and `b=` (the set
/// and the two images the viewer opens on) through to the finished session.
fn to_progress(id: &str, q: &HashMap<String, String>) -> Resp {
    let open: Vec<String> = ["set", "a", "b"]
        .iter()
        .filter_map(|k| {
            q.get(*k)
                .filter(|v| !v.is_empty() && v.len() <= MAX_NAME_LEN)
                .map(|v| format!("{k}={}", pct_encode(v)))
        })
        .collect();
    if open.is_empty() {
        Resp::redirect(&format!("/progress/{id}"))
    } else {
        Resp::redirect(&format!("/progress/{id}?{}", open.join("&")))
    }
}

/// Labels for single images: `parent/name`, made unique with `-2`, `-3`.
fn image_labels(rels: &[String]) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for rel in rels {
        let parts: Vec<&str> = rel.split('/').collect();
        let base = parts[parts.len().saturating_sub(2)..].join("/");
        let mut label = base.clone();
        let mut n = 2;
        while labels.contains(&label) {
            label = format!("{base}-{n}");
            n += 1;
        }
        labels.push(label);
    }
    labels
}

/// Stages 2 to 6 single images under the pairs cache (reused while the files
/// are unchanged) and returns the staged directories.
fn stage_single_images(state: &State, files: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let meta = |p: &PathBuf| {
        std::fs::metadata(p)
            .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
            .unwrap_or_default()
    };
    let parts: Vec<String> = files
        .iter()
        .flat_map(|p| [p.to_string_lossy().into_owned(), meta(p)])
        .collect();
    let refs: Vec<&[u8]> = parts.iter().map(|s| s.as_bytes()).collect();
    let key = hash128(&refs);
    let dest = state.cache.join("pairs").join(format!("n-{key}"));
    let names: Vec<String> = (0..files.len()).map(|i| i.to_string()).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    if !dest.join(&names[files.len() - 1]).is_dir() {
        let tmp = state
            .cache
            .join("pairs")
            .join(format!(".tmp-{key}-{}", session::unique()));
        let paths: Vec<&std::path::Path> = files.iter().map(PathBuf::as_path).collect();
        if let Err(e) = stage_images(&paths, &tmp, &name_refs) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
        let _ = std::fs::remove_dir_all(&dest);
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(format!("staging the images: {e}"));
        }
    }
    Ok(names.iter().map(|n| dest.join(n)).collect())
}

/// Stages a run paired by position or by hand under the pairs cache.
fn stage_paired_run(
    state: &State,
    reference: &crate::runs::RunInput,
    run: &crate::runs::RunInput,
) -> Result<PathBuf, String> {
    let opts = crate::runs::RunsOptions {
        pixels_per_degree: state.view.pixels_per_degree,
        hdr: state.view.hdr,
        meta: state.view.meta.clone(),
        entries: state.view.entries.clone(),
    };
    let plan = crate::runs::plan(reference, std::slice::from_ref(run), &opts)
        .map_err(|e| e.to_string())?;
    let dest = state
        .cache
        .join("pairs")
        .join(format!("r-{}", plan.fingerprint()));
    if !dest.is_dir() {
        let tmp = state
            .cache
            .join("pairs")
            .join(format!(".tmp-r-{}", session::unique()));
        if let Err(e) = stage_named(&plan.paired_files(0), &tmp) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(format!("staging the run: {e}"));
        }
    }
    Ok(dest)
}

fn compare(state: &Arc<State>, q: &Query) -> Resp {
    let runs = q.runs();
    if runs.len() == 1 {
        let dest = if is_image_name(&runs[0]) {
            "/image"
        } else {
            "/run"
        };
        return Resp::redirect(&format!("{dest}?path={}", pct_encode(&runs[0])));
    }
    if !(MIN_DIRS..=MAX_DIRS).contains(&runs.len()) {
        return Resp::error_page(
            400,
            "This comparison link is incomplete",
            &format!(
                "run= takes {MIN_DIRS} to {MAX_DIRS} repeated run paths (legacy runs= is also accepted)."
            ),
        );
    }
    let labels = q
        .get("labels")
        .filter(|l| !l.is_empty())
        .map(|l| split_list(l));
    if labels.as_ref().is_some_and(|l| l.len() != runs.len()) {
        return Resp::error_page(
            400,
            "This comparison link is invalid",
            "labels= must have one label per run.",
        );
    }
    let mut dirs = Vec::new();
    for r in &runs {
        match resolve_under(state, r) {
            Ok(d) => dirs.push(d),
            Err(PathError::Invalid) => {
                return Resp::error_page(400, "Invalid path", &format!("{r}: invalid path."));
            }
            Err(PathError::Escapes) => {
                return Resp::error_page(
                    403,
                    "Outside the archive",
                    &format!("{r}: escapes the archive root."),
                );
            }
            Err(PathError::Missing) => {
                return Resp::error_page(
                    404,
                    "Run not found",
                    &format!(
                        "{r}: no such run in this archive. It may have been moved or deleted."
                    ),
                );
            }
        }
    }
    let blind = q.get("blind").is_some_and(|b| b == "1" || b == "true");
    let rels = runs.clone();
    let (all_dirs, all_files) = (
        dirs.iter().all(|d| d.is_dir()),
        dirs.iter().all(|d| d.is_file()),
    );
    if !all_dirs && !all_files {
        return Resp::error_page(
            400,
            "Mixed selection",
            "Compare either runs (directories) or single images, not both.",
        );
    }
    if all_files {
        for (dir, rel) in dirs.iter_mut().zip(&rels) {
            *dir = match resolve_image(state, rel) {
                Ok(d) => d,
                Err(r) => return r.into_error_page(),
            };
        }
        // Single images: staged so a view pairs them whatever their names.
        if let Some(bad) = dirs
            .iter()
            .zip(&rels)
            .find(|(d, r)| !is_image_name(&d.to_string_lossy()) || !is_image_name(r))
        {
            return Resp::error_page(
                403,
                "Only images",
                &format!("{}: only image files can be compared.", bad.1),
            );
        }
        let staged = match stage_single_images(state, &dirs) {
            Ok(s) => s,
            Err(e) => return Resp::error_page(422, "These images cannot be compared", &e),
        };
        let labels = labels.or_else(|| Some(image_labels(&rels)));
        let id = ensure(
            state,
            Spec {
                dirs: staged,
                labels,
                runs: rels,
                blind,
                overview: None,
            },
        );
        return to_progress(&id, q);
    }
    // Runs: unlike names are paired by position or by hand (`pair<i>=`, the
    // reference being 0) and staged under the reference's names.
    let labels = labels.unwrap_or_else(|| crate::runs::unique_labels(&dirs));
    for (dir, rel) in dirs.iter_mut().zip(&rels) {
        *dir = match super::storage::snapshot(state, rel) {
            Ok(d) => d,
            Err(e) => return path_error(&e).into_error_page(),
        };
    }
    let mut staged = dirs.clone();
    for (i, slot) in staged.iter_mut().enumerate().skip(1) {
        let pairing = match crate::runs::Pairing::parse(
            q.get(&format!("pair{i}")).map_or("", String::as_str),
        ) {
            Ok(p) => p,
            Err(e) => return Resp::error_page(400, "This comparison link is invalid", &e),
        };
        if pairing == crate::runs::Pairing::Name {
            continue;
        }
        let input = |k: usize, p: crate::runs::Pairing| crate::runs::RunInput {
            dir: dirs[k].clone(),
            label: labels[k].clone(),
            display: rels[k].clone(),
            pairing: p,
        };
        match stage_paired_run(
            state,
            &input(0, crate::runs::Pairing::Name),
            &input(i, pairing),
        ) {
            Ok(d) => *slot = d,
            Err(e) => return Resp::error_page(422, "These runs cannot be paired", &e),
        }
    }
    let pairs: String = (1..rels.len())
        .filter_map(|i| {
            let v = q.get(&format!("pair{i}"))?;
            (!v.is_empty() && v != "name").then(|| format!("&pair{i}={}", pct_encode(v)))
        })
        .collect();
    let extras = ["labels", "blind"]
        .iter()
        .filter_map(|k| q.get(*k).map(|v| format!("&{k}={}", pct_encode(v))))
        .collect::<String>();
    let overview = Some(format!(
        "ref={}&{}{pairs}{extras}",
        pct_encode(&rels[0]),
        rels[1..]
            .iter()
            .map(|r| format!("run={}", pct_encode(r)))
            .collect::<Vec<_>>()
            .join("&")
    ));
    let id = ensure(
        state,
        Spec {
            dirs: staged,
            labels: Some(labels),
            runs: rels,
            blind,
            overview,
        },
    );
    to_progress(&id, q)
}

fn pair(state: &Arc<State>, a: &str, b: &str, q: &Query) -> Resp {
    for rel in [a, b] {
        if let Err(r) = resolve_image(state, rel) {
            return r.into_error_page();
        }
    }
    let mut query = format!("run={}&run={}", pct_encode(a), pct_encode(b));
    for k in ["labels", "blind", "set"] {
        if let Some(v) = q.get(k) {
            query.push_str(&format!("&{k}={}", pct_encode(v)));
        }
    }
    compare(state, &parse_query(&query))
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
        return Resp::error_page(
            404,
            "Unknown comparison",
            "This comparison link is not valid.",
        );
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
            return Resp::error_page(
                404,
                "Comparison not found",
                "This comparison is no longer available.",
            );
        };
        // The decisions twin only `file://` pages need: a served session reads
        // and writes decisions through the API, so the request would 404.
        let html = html.replacen(DECISIONS_TWIN_TAG, "", 1);
        let overview = session_overview(state, id).map(|q| format!("/runs?{q}"));
        let data = serde_json::json!({ "token": state.token, "session": id, "browse": browse_target(state, id), "overview": overview })
            .to_string();
        let inject = format!(
            "<script>window.SACCADE_SERVE={data};</script><script>{SESSION_OPEN_JS}</script>"
        );
        let html = html.replacen("</head>", &format!("{inject}</head>"), 1);
        return Resp::html(html);
    }
    if !allowed_session_file(file) {
        return Resp::text(404, "not found");
    }
    let Ok(canon_dir) = crate::paths::canonicalize(&dir) else {
        return Resp::text(404, "not found");
    };
    match resolve_in(&canon_dir, file) {
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
    if let Some(len) = req.body_length()
        && len as u64 > cap
    {
        if (len as u64) <= cap.saturating_mul(4).max(1 << 20) {
            let _ = std::io::copy(&mut req.as_reader().take(len as u64), &mut std::io::sink());
        }
        return Err(Resp::error(413, "body exceeds the size cap"));
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
    if path == "/api/inbox" || path.starts_with("/api/inbox/") {
        return inbox_api::post(state, req, path);
    }
    if path.starts_with("/api/vote/") {
        return vote_api::post(state, req, path);
    }
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
        .map(|e| rel_in(dir, e.path()))
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
        match session::stage_pair(&da.join(&fa[0]), &db.join(&fb[0]), &staged) {
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
            overview: None,
        },
    );
    Resp::json(&serde_json::json!({ "session": sid, "url": format!("/progress/{sid}") }))
}

/// Only root-relative paths appear in a storage timeout, never NAS targets.
fn storage_label(state: &State, q: &Query) -> String {
    for key in ["path", "ref", "a"] {
        if let Some(p) = q.get(key).filter(|p| super::browse::is_plain_rel(p)) {
            return p.clone();
        }
    }
    if let Some(p) = q.runs().first().filter(|p| super::browse::is_plain_rel(p)) {
        return p.clone();
    }
    for abs in q.values("abs") {
        if let Some(root) = state.root_of(std::path::Path::new(&abs)) {
            let sub = rel_in(&root.path, std::path::Path::new(&abs));
            return if state.multi() {
                format!("{}/{sub}", root.name)
            } else {
                sub
            };
        }
    }
    "archive".into()
}

fn open_absolute(state: &State, q: &Query) -> Resp {
    let paths = q.values("abs");
    if paths.is_empty() || paths.len() > MAX_DIRS {
        return Resp::error_page(400, "Incomplete link", "abs= takes 1 to 6 absolute paths.");
    }
    let mut rels = Vec::new();
    for abs in paths.iter().chain(q.get("ref")) {
        match super::storage::absolute_rel(state, abs) {
            Ok(p) => rels.push(p),
            Err(_) => {
                return Resp::error_page(
                    404,
                    "Path not found",
                    &format!("{abs}: not found or not under a served root"),
                );
            }
        }
    }
    let dirs: Vec<bool> = rels
        .iter()
        .map(|r| resolve_under(state, r).is_ok_and(|p| p.is_dir()))
        .collect();
    let files: Vec<bool> = rels
        .iter()
        .map(|r| resolve_under(state, r).is_ok_and(|p| p.is_file()) && is_image_name(r))
        .collect();
    let extras = ["labels", "blind"]
        .iter()
        .filter_map(|k| q.get(*k).map(|v| format!("&{k}={}", pct_encode(v))))
        .collect::<String>();
    let destination = if q.contains_key("ref") && dirs.iter().all(|d| *d) {
        let Some(reference) = rels.pop() else {
            return Resp::text(400, "missing reference");
        };
        format!(
            "/runs?ref={}&{}",
            pct_encode(&reference),
            rels.iter()
                .map(|r| format!("run={}", pct_encode(r)))
                .collect::<Vec<_>>()
                .join("&")
        )
    } else if q.contains_key("ref") {
        return Resp::error_page(400, "Invalid selection", "ref= requires run directories.");
    } else if dirs.iter().all(|d| *d) || files.iter().all(|d| *d) {
        if rels.len() == 1 {
            format!(
                "{}?path={}",
                if dirs[0] { "/run" } else { "/image" },
                pct_encode(&rels[0])
            )
        } else if files.iter().all(|d| *d) && rels.len() == 2 {
            format!(
                "/pair?a={}&b={}",
                pct_encode(&rels[0]),
                pct_encode(&rels[1])
            )
        } else {
            format!(
                "/compare?{}",
                rels.iter()
                    .map(|r| format!("run={}", pct_encode(r)))
                    .collect::<Vec<_>>()
                    .join("&")
            )
        }
    } else {
        return Resp::error_page(
            404,
            "Path not found",
            &format!("{}: not found or not under a served root", paths[0]),
        );
    };
    Resp::redirect(&format!("{destination}{extras}"))
}

fn run_page(state: &State, rel: &str) -> Resp {
    let details = match super::browse::run_details(state, rel) {
        Ok(d) => d,
        Err(e) => return path_error(&e).into_error_page(),
    };
    let data = details.to_string().replace('<', "\\u003c");
    let html = include_str!("../../assets/run.html")
        .replace(
            "/*__RUN_CSS__*/",
            &crate::render::shared::page_css(&[CSS, include_str!("../../assets/run.css")]),
        )
        .replace(
            "/*__RUN_JS__*/",
            &crate::render::shared::page_js(&[include_str!("../../assets/run.js")]),
        )
        .replace("__RUN_DATA__", &data);
    Resp::html(html)
}

fn single_image(state: &Arc<State>, rel: &str) -> Resp {
    let image = match resolve_image(state, rel) {
        Ok(p) => p,
        Err(r) => return r.into_error_page(),
    };
    let dirs = match stage_single_images(state, &[image]) {
        Ok(d) => d,
        Err(_) => {
            return Resp::error_page(
                422,
                "Image cannot be opened",
                "This image cannot be decoded.",
            );
        }
    };
    let id = ensure(
        state,
        Spec {
            dirs,
            labels: Some(vec![rel.to_owned()]),
            runs: vec![rel.to_owned()],
            blind: false,
            overview: None,
        },
    );
    to_progress(&id, &HashMap::new())
}
