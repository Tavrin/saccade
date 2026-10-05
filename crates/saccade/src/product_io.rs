//! Bounded product-adapter transport and immutable output helpers.
use crate::agent::CliError;
use serde::{Serialize, de::DeserializeOwned};
use std::{io::Read, path::Path, time::Duration};
use url::Url;

pub(crate) const MAX_BYTES: u64 = 32 * 1024 * 1024;
pub(crate) fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    let file = std::fs::File::open(path).map_err(|_| CliError::io("cannot open product input"))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| CliError::io("cannot read product input"))?;
    if bytes.len() as u64 > MAX_BYTES { return Err(CliError::new("input_budget", "product input exceeds 32 MiB")); }
    Ok(bytes)
}
pub(crate) fn json<T: DeserializeOwned>(path: &Path) -> Result<T, CliError> {
    serde_json::from_slice(&read(path)?).map_err(|_| CliError::new("config", "invalid product JSON"))
}
pub(crate) fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|_| CliError::io("cannot create output parent"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| CliError::io("cannot reserve output"))?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.persist_noclobber(path).map_err(|_| CliError::io("output already exists or cannot be written"))?;
    Ok(())
}
pub(crate) fn url(text: &str) -> Result<Url, CliError> {
    let parsed = Url::parse(text).map_err(|_| CliError::usage("invalid HTTP URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(CliError::usage("HTTP URLs require a host, no credentials or fragment"));
    }
    Ok(parsed)
}
pub(crate) struct Response { pub(crate) bytes: Vec<u8>, pub(crate) content_type: String, pub(crate) status: u16 }
pub(crate) fn request(method: &str, target: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Result<Response, CliError> {
    url(target)?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).max_redirects(0).http_status_as_error(false).build().into();
    for attempt in 0..4 {
        // No redirects: credentials must never be forwarded to a download host.
        let mut response = if method == "POST" {
            let mut request = agent.post(target);
            for (name, value) in headers { request = request.header(*name, *value); }
            request.send(body.unwrap_or_default())
        } else {
            let mut request = agent.get(target);
            for (name, value) in headers { request = request.header(*name, *value); }
            request.call()
        }.map_err(|_| CliError::new("transport", "HTTP request failed (URL and credentials redacted)"))?;
        let status = response.status().as_u16();
        if status == 429 || status == 503 {
            if attempt == 3 { return Err(CliError::new("rate_limit", "HTTP retry budget exhausted")); }
            let delay = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(1 << attempt).min(30);
            std::thread::sleep(Duration::from_secs(delay)); continue;
        }
        let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("unknown").to_owned();
        let bytes = response.body_mut().with_config().limit(MAX_BYTES).read_to_vec().map_err(|_| CliError::new("input_budget", "HTTP body unreadable or exceeds 32 MiB"))?;
        return Ok(Response { bytes, content_type, status });
    }
    Err(CliError::new("transport", "HTTP retry budget exhausted"))
}
pub(crate) fn get(target: &str, headers: &[(&str, &str)]) -> Result<Response, CliError> {
    let response = request("GET", target, headers, None)?;
    if !(200..300).contains(&response.status) { return Err(CliError::new("http_status", format!("HTTP status {} (URL redacted)", response.status))); }
    Ok(response)
}
pub(crate) fn emit<T: Serialize>(value: &T, json: bool, summary: &str) -> Result<(), CliError> {
    crate::emit(&if json { format!("{}\n", serde_json::to_string(value)?) } else { format!("{summary}\n") })
}
pub(crate) fn image(bytes: &[u8]) -> Result<image::DynamicImage, CliError> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|_| CliError::io("unknown image encoding"))?;
    let mut limits = image::Limits::default(); limits.max_image_width = Some(16384); limits.max_image_height = Some(16384); limits.max_alloc = Some(256 * 1024 * 1024); reader.limits(limits);
    reader.decode().map_err(|_| CliError::new("decode", "image cannot decode within limits"))
}
