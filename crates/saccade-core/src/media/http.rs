//! Loopback HTTP API sharing the analyzer and exact index implementation.
use super::{Analyzer, MediaError, Options, Result, search};
use crate::{general::input, root_policy::RootPolicy};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    time::{Duration, Instant},
};
/// Versioned health output.
pub const HEALTH_SCHEMA: &str = "saccade-api-health.v1";
/// Generic API error output (stable code shared with CLI/Python).
pub const ERROR_SCHEMA: &str = "saccade-media-error.v1";
/// Local API with startup-owned roots, model configuration, size limit and optional token.
/// Credentials deliberately have no Debug/Serialize implementation.
pub struct Api {
    analyzer: Analyzer,
    policy: RootPolicy,
    max_bytes: usize,
    token: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Source {
    Path(PathSource),
    Bytes(BytesSource),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathSource {
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BytesSource {
    bytes_base64: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Analyze {
    source: Source,
    #[serde(default)]
    options: Options,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Compare {
    a: Source,
    b: Source,
    #[serde(default = "default_ppd")]
    ppd: f32,
}
fn default_ppd() -> f32 {
    67.
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    index: PathBuf,
    image: Option<Source>,
    text: Option<String>,
    #[serde(default = "default_top")]
    top: usize,
}
fn default_top() -> usize {
    10
}
impl Api {
    /// Create from startup-owned authority. Request bodies cannot change roots or pull models.
    pub fn new(
        analyzer: Analyzer,
        policy: RootPolicy,
        max_bytes: usize,
        token: Option<String>,
    ) -> Result<Self> {
        if max_bytes == 0
            || max_bytes > input::MAX_BYTES as usize
            || token.as_ref().is_some_and(|s| {
                s.is_empty() || s.len() > 4096 || s.chars().any(char::is_whitespace)
            })
        {
            return Err(MediaError::new(
                "invalid_media_options",
                "API token/size limit invalid",
            ));
        }
        Ok(Self {
            analyzer,
            policy,
            max_bytes,
            token,
        })
    }
    fn source(&self, s: Source) -> Result<Vec<u8>> {
        match s {
            Source::Path(p) => {
                let path = self
                    .policy
                    .read(&p.path)
                    .map_err(|e| MediaError::new("unsafe_path", e.to_string()))?;
                Ok(input::bytes(&path, input::MAX_BYTES)?)
            }
            Source::Bytes(b) => {
                if b.bytes_base64.len() > input::MAX_BYTES as usize * 4 / 3 + 4 {
                    return Err(MediaError::new(
                        "request_too_large",
                        "encoded image exceeds limit",
                    ));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b.bytes_base64)
                    .map_err(|_| MediaError::new("invalid_media_input", "invalid base64 image"))?;
                if bytes.len() as u64 > input::MAX_BYTES {
                    return Err(MediaError::new(
                        "request_too_large",
                        "decoded base64 exceeds input limit",
                    ));
                }
                Ok(bytes)
            }
        }
    }
    /// Route one bounded request; returns HTTP status and versioned JSON without logging inputs.
    /// Startup bearer authentication covers health and computation alike.
    pub fn handle(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
    ) -> (u16, Value) {
        let result = (|| -> Result<Value> {
            if let Some(token) = &self.token {
                let expected = format!("Bearer {token}");
                let supplied = authorization.unwrap_or("");
                let different = expected.len() != supplied.len()
                    || expected
                        .bytes()
                        .zip(supplied.bytes())
                        .fold(0u8, |a, (b, c)| a | (b ^ c))
                        != 0;
                if different {
                    return Err(MediaError::new("unauthorized", "bearer token required"));
                }
            }
            if body.len() > self.max_bytes {
                return Err(MediaError::new(
                    "request_too_large",
                    "request body exceeds startup limit",
                ));
            }
            if method == "GET" && path == "/v1/health" {
                return Ok(
                    json!({"schema":HEALTH_SCHEMA,"status":"ok","media_schema":super::SCHEMA,"version":env!("CARGO_PKG_VERSION")}),
                );
            }
            if method != "POST" {
                return Err(MediaError::new(
                    "method_not_allowed",
                    "use POST for computation or GET /v1/health",
                ));
            }
            match path {
                "/v1/analyze-media" => {
                    let r: Analyze = serde_json::from_slice(body)?;
                    let record = match r.source {
                        Source::Path(p) => {
                            let path = self
                                .policy
                                .read(&p.path)
                                .map_err(|e| MediaError::new("unsafe_path", e.to_string()))?;
                            let source = path.to_str().ok_or_else(|| {
                                MediaError::new("invalid_media_input", "non-UTF8 API path")
                            })?;
                            self.analyzer.analyze_media(source, &r.options)?
                        }
                        source => self
                            .analyzer
                            .analyze_bytes(&self.source(source)?, &r.options)?,
                    };
                    Ok(serde_json::to_value(record)?)
                }
                "/v1/compare" => {
                    let r: Compare = serde_json::from_slice(body)?;
                    self.analyzer
                        .compare(&self.source(r.a)?, &self.source(r.b)?, r.ppd)
                }
                "/v1/search" => {
                    let r: Search = serde_json::from_slice(body)?;
                    let path = self
                        .policy
                        .read(&r.index)
                        .map_err(|e| MediaError::new("unsafe_path", e.to_string()))?;
                    let metadata = self
                        .policy
                        .read(&path.join(format!("{}.json", search::SCHEMA)))
                        .map_err(|e| MediaError::new("unsafe_path", e.to_string()))?;
                    let _vectors = self
                        .policy
                        .read(&path.join("vectors.bin"))
                        .map_err(|e| MediaError::new("unsafe_path", e.to_string()))?;
                    let index =
                        search::Index::load(metadata.parent().ok_or_else(|| {
                            MediaError::new("unsafe_path", "index parent missing")
                        })?)?;
                    match (r.image, r.text) {
                        (Some(image), None) => {
                            index.query_image(&self.analyzer, &self.source(image)?, r.top)
                        }
                        (None, Some(text)) => index.query_text(&self.analyzer, &text, r.top),
                        _ => Err(MediaError::new(
                            "invalid_media_options",
                            "search requires exactly one image or text",
                        )),
                    }
                }
                _ => Err(MediaError::new(
                    "not_found",
                    "unknown versioned API endpoint",
                )),
            }
        })();
        match result {
            Ok(v) => (200, v),
            Err(e) => failure(e),
        }
    }
    /// Serve one connection (bounded headers/body and terminating socket deadlines).
    /// This small HTTP/1.1 subset requires Content-Length and closes each connection.
    pub fn connection(&self, mut stream: TcpStream) -> Result<()> {
        let request = read_request(&mut stream, self.max_bytes);
        let (status, value) = match request {
            Ok(r) => self.handle(&r.method, &r.path, r.authorization.as_deref(), &r.body),
            Err(e) => failure(e),
        };
        let body = serde_json::to_vec(&value)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| MediaError::new("io_error", e.to_string()))?;
        let message = match status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            415 => "Unsupported Media Type",
            _ => "Error",
        };
        write!(stream,"HTTP/1.1 {status} {message}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).and_then(|_|stream.write_all(&body)).map_err(|e|MediaError::new("io_error",e.to_string()))?;
        Ok(())
    }
    /// Bind to 127.0.0.1 and serve sequential requests using one reusable analyzer.
    pub fn serve(&self, port: u16) -> Result<()> {
        self.serve_on(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)
    }
    /// Bind to a caller-declared address; loopback remains the default startup policy.
    pub fn serve_on(&self, bind: std::net::IpAddr, port: u16) -> Result<()> {
        let listener = TcpListener::bind((bind, port))
            .map_err(|e| MediaError::new("io_error", e.to_string()))?;
        eprintln!(
            "Saccade media API listening on {}",
            listener
                .local_addr()
                .map_err(|e| MediaError::new("io_error", e.to_string()))?
        );
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let _ = self.connection(stream);
                }
                Err(e) => return Err(MediaError::new("io_error", e.to_string())),
            }
        }
        Ok(())
    }
}
fn failure(e: MediaError) -> (u16, Value) {
    let status = match e.code.as_str() {
        "unauthorized" => 401,
        "unsafe_path" => 403,
        "not_found" => 404,
        "method_not_allowed" => 405,
        "request_too_large" => 413,
        "unsupported_media_type" => 415,
        _ => 400,
    };
    (
        status,
        json!({"schema":ERROR_SCHEMA,"code":e.code,"message":e.message}),
    )
}
struct Request {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Vec<u8>,
}
fn deadline(stream: &TcpStream, start: Instant) -> Result<()> {
    let left = Duration::from_secs(10)
        .checked_sub(start.elapsed())
        .ok_or_else(|| MediaError::new("invalid_http_request", "request deadline"))?;
    stream
        .set_read_timeout(Some(left.max(Duration::from_millis(1))))
        .map_err(|e| MediaError::new("io_error", e.to_string()))?;
    Ok(())
}
fn read_request(stream: &mut TcpStream, max: usize) -> Result<Request> {
    let start = Instant::now();
    let mut header = Vec::new();
    let mut byte = [0u8];
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= 32768 {
            return Err(MediaError::new(
                "request_too_large",
                "HTTP headers exceed limit",
            ));
        }
        deadline(stream, start)?;
        stream.read_exact(&mut byte).map_err(|_| {
            MediaError::new(
                "invalid_http_request",
                "incomplete HTTP headers or deadline",
            )
        })?;
        header.push(byte[0]);
    }
    let text = std::str::from_utf8(&header)
        .map_err(|_| MediaError::new("invalid_http_request", "headers require UTF-8"))?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("").to_owned();
    let path = first.next().unwrap_or("").to_owned();
    let version = first.next().unwrap_or("");
    if first.next().is_some()
        || !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || !path.starts_with('/')
        || path.len() > 8192
    {
        return Err(MediaError::new(
            "invalid_http_request",
            "invalid request line",
        ));
    }
    let mut length = None;
    let mut authorization = None;
    let mut content_type = None;
    for line in lines.filter(|s| !s.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| MediaError::new("invalid_http_request", "invalid header"))?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => {
                if length.is_some() {
                    return Err(MediaError::new(
                        "invalid_http_request",
                        "duplicate content length",
                    ));
                }
                length = Some(value.parse::<usize>().map_err(|_| {
                    MediaError::new("invalid_http_request", "invalid content length")
                })?);
            }
            "authorization" => {
                if authorization.is_some() {
                    return Err(MediaError::new(
                        "invalid_http_request",
                        "duplicate authorization",
                    ));
                }
                authorization = Some(value.to_owned());
            }
            "content-type" => {
                if content_type.is_some() {
                    return Err(MediaError::new(
                        "invalid_http_request",
                        "duplicate content type",
                    ));
                }
                content_type = Some(value.to_owned());
            }
            "transfer-encoding" => {
                return Err(MediaError::new(
                    "invalid_http_request",
                    "chunked request bodies unsupported; use Content-Length",
                ));
            }
            _ => {}
        }
    }
    if method == "POST"
        && content_type
            .as_deref()
            .is_none_or(|s| s.split(';').next().map(str::trim) != Some("application/json"))
    {
        return Err(MediaError::new(
            "unsupported_media_type",
            "POST requires application/json",
        ));
    }
    let length = length.unwrap_or(0);
    if method == "POST" && length == 0 {
        return Err(MediaError::new(
            "invalid_http_request",
            "POST requires a nonempty Content-Length body",
        ));
    }
    if length > max {
        return Err(MediaError::new(
            "request_too_large",
            "body exceeds startup limit",
        ));
    }
    let mut body = vec![0; length];
    for chunk in body.chunks_mut(16384) {
        deadline(stream, start)?;
        stream.read_exact(chunk).map_err(|_| {
            MediaError::new("invalid_http_request", "incomplete HTTP body or deadline")
        })?;
    }
    Ok(Request {
        method,
        path,
        authorization,
        body,
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn api(dir: &std::path::Path, token: Option<String>) -> Api {
        Api::new(
            Analyzer::new(super::super::Profile::CpuLite, "cache".into(), false).unwrap(),
            RootPolicy::new(&[dir.to_path_buf()], None, false, &[]).unwrap(),
            65536,
            token,
        )
        .unwrap()
    }
    #[test]
    fn shared_record_compare_auth_size_and_root_boundaries() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("image.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([30, 50, 70, 255]))
            .save(&file)
            .unwrap();
        let api = api(d.path(), Some("test-token".into()));
        assert_eq!(api.handle("GET", "/v1/health", None, b"").0, 401);
        let auth = Some("Bearer test-token");
        assert_eq!(
            api.handle("GET", "/v1/health", auth, b"").1["schema"],
            HEALTH_SCHEMA
        );
        let request = serde_json::to_vec(&json!({"source":{"path":file}})).unwrap();
        let r = api.handle("POST", "/v1/analyze-media", auth, &request);
        assert_eq!(r.0, 200);
        assert_eq!(r.1["schema"], super::super::SCHEMA);
        let bytes = input::bytes(&file, input::MAX_BYTES).unwrap();
        let direct = api
            .analyzer
            .analyze_bytes(&bytes, &Options::default())
            .unwrap();
        assert_eq!(r.1["fingerprints"]["data"], direct.fingerprints.data);
        let cmp = serde_json::to_vec(&json!({"a":{"path":file},"b":{"path":file}})).unwrap();
        assert_eq!(
            api.handle("POST", "/v1/compare", auth, &cmp).1["metrics"]["mean"],
            0.
        );
        assert_eq!(
            api.handle("POST", "/v1/analyze-media", auth, &vec![b' '; 65537])
                .0,
            413
        );
        let escape = serde_json::to_vec(&json!({"source":{"path":"/outside/image.png"}})).unwrap();
        assert_eq!(
            api.handle("POST", "/v1/analyze-media", auth, &escape).0,
            403
        );
    }
    #[test]
    fn loopback_wire_health_and_length_rejection() {
        let d = tempfile::tempdir().unwrap();
        let a = api(d.path(), None);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::scope(|s| {
            s.spawn(|| {
                for _ in 0..2 {
                    a.connection(listener.accept().unwrap().0).unwrap();
                }
            });
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut body = String::new();
            stream.read_to_string(&mut body).unwrap();
            assert!(body.starts_with("HTTP/1.1 200"));
            assert!(body.contains(HEALTH_SCHEMA));
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(b"POST /v1/compare HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 999999\r\n\r\n").unwrap();
            let mut body = String::new();
            stream.read_to_string(&mut body).unwrap();
            assert!(body.starts_with("HTTP/1.1 413"));
        });
    }
}
