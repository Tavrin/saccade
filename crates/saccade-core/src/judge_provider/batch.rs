//! Evaluation-only Gemini Batch API. Interactive review continues through `transport`.
use super::Keys;
use crate::evidence::canonical::Digest;
use serde_json::{Value, json};
use std::time::Duration;

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const MAX_INLINE_BYTES: usize = 20 * 1024 * 1024;

/// A bounded HTTP response. Provider error bodies are never included in errors.
pub struct BatchReply {
    /// HTTP status.
    pub status: u16,
    /// Parsed JSON body.
    pub body: Value,
}

/// Injectable HTTP boundary for asynchronous batch transport.
pub trait BatchHttp {
    /// Send a request without following redirects or exposing credentials.
    fn send(
        &self,
        method: &str,
        url: &str,
        key: &str,
        body: Option<&[u8]>,
    ) -> Result<BatchReply, String>;
}

/// Production HTTPS boundary.
pub struct BatchNetwork;
impl BatchHttp for BatchNetwork {
    fn send(
        &self,
        method: &str,
        url: &str,
        key: &str,
        body: Option<&[u8]>,
    ) -> Result<BatchReply, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(240)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        let mut response = match (method, body) {
            ("POST", Some(bytes)) => agent
                .post(url)
                .header("x-goog-api-key", key)
                .header("Content-Type", "application/json")
                .send(bytes),
            ("GET", None) => agent.get(url).header("x-goog-api-key", key).call(),
            _ => return Err("invalid batch HTTP operation".into()),
        }
        .map_err(|_| "batch transport unavailable or timed out".to_owned())?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(32 * 1024 * 1024)
            .read_to_vec()
            .map_err(|_| "batch response unavailable or too large".to_owned())?;
        Ok(BatchReply {
            status,
            body: serde_json::from_slice(&body)
                .map_err(|_| "invalid batch JSON response".to_owned())?,
        })
    }
}

fn model_id(model: &str) -> Result<(), String> {
    if model.is_empty()
        || model.len() > 80
        || !model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("invalid Gemini batch model".into());
    }
    Ok(())
}
fn operation_id(name: &str) -> Result<(), String> {
    let Some(id) = name.strip_prefix("batches/") else {
        return Err("invalid Gemini batch ID".into());
    };
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("invalid Gemini batch ID".into());
    }
    Ok(())
}

/// One request and its immutable local identity. Metadata is copied into the
/// provider response and verified again before results are accepted.
pub fn inline_request(job_id: &str, request: Value) -> Result<Value, String> {
    if !job_id.starts_with("sha256:")
        || job_id.len() != 71
        || !job_id[7..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid frozen job ID".into());
    }
    if !request.is_object() || request.get("contents").is_none() {
        return Err("invalid Gemini request".into());
    }
    let hash = Digest::of_bytes(&serde_json::to_vec(&request).map_err(|_| "invalid request JSON")?);
    Ok(json!({"request":request,"metadata":{"job_id":job_id,"request_sha256":hash}}))
}

/// Build a small inline batch; larger corpora must be chunked by the runner.
pub fn submission(model: &str, name: &str, requests: &[Value]) -> Result<Vec<u8>, String> {
    model_id(model)?;
    if requests.is_empty() || name.is_empty() || name.len() > 80 {
        return Err("empty or invalid Gemini batch".into());
    }
    let body = serde_json::to_vec(&json!({"batch":{"displayName":name,
        "inputConfig":{"requests":{"requests":requests}}}}))
    .map_err(|_| "invalid batch JSON")?;
    if body.len() >= MAX_INLINE_BYTES {
        return Err("Gemini inline batch exceeds 20 MB".into());
    }
    Ok(body)
}

/// Fail closed when a response is missing, duplicated, or bound to another request.
pub fn collect(operation: &Value, requests: &[Value]) -> Result<Vec<(String, Value)>, String> {
    let name = operation["name"].as_str().ok_or("batch has no ID")?;
    operation_id(name)?;
    let state = operation
        .get("metadata")
        .and_then(|m| m.get("state"))
        .unwrap_or(&operation["state"]);
    if state != "BATCH_STATE_SUCCEEDED" && state != "JOB_STATE_SUCCEEDED" {
        return Err("Gemini batch is not successful".into());
    }
    let responses = operation["response"]["inlinedResponses"]
        .as_array()
        .or_else(|| operation["response"]["inlinedResponses"]["inlinedResponses"].as_array())
        .or_else(|| operation["output"]["inlinedResponses"]["inlinedResponses"].as_array())
        .ok_or("batch has no inline responses")?;
    if responses.len() != requests.len() {
        return Err("batch response count mismatch".into());
    }
    let mut expected = std::collections::BTreeMap::new();
    for r in requests {
        let id = r["metadata"]["job_id"]
            .as_str()
            .ok_or("request missing job ID")?;
        if expected
            .insert(id.to_owned(), r["metadata"]["request_sha256"].clone())
            .is_some()
        {
            return Err("duplicate batch request ID".into());
        }
    }
    let mut out = Vec::new();
    for (r, submitted) in responses.iter().zip(requests) {
        let metadata = &r["metadata"];
        let id = metadata["job_id"]
            .as_str()
            .ok_or("response missing job ID")?;
        if id != submitted["metadata"]["job_id"]
            || expected.remove(id).as_ref() != Some(&metadata["request_sha256"])
        {
            return Err("batch response identity mismatch".into());
        }
        if r["response"].is_object() && r["error"].is_null() {
            out.push((id.to_owned(), r["response"].clone()));
        } else if r["error"].is_object() && r["response"].is_null() {
            // Keep only a bounded error class; the provider body is untrusted.
            out.push((
                id.to_owned(),
                json!({"batch_error_status":r["error"]["status"]}),
            ));
        } else {
            return Err("invalid batch response cell".into());
        }
    }
    if !expected.is_empty() {
        return Err("batch responses incomplete".into());
    }
    Ok(out)
}

/// Submit, count tokens, and poll with the existing file-only key loader.
pub struct GeminiBatch<'a> {
    /// File-only key loader.
    pub keys: &'a Keys,
    /// HTTP boundary.
    pub http: &'a dyn BatchHttp,
}
impl GeminiBatch<'_> {
    fn call(&self, method: &str, url: &str, body: Option<&[u8]>) -> Result<Value, String> {
        let key = self.keys.load("gemini.env", "SACCADE_GEMINI_API_KEY")?;
        let reply = self.http.send(method, url, key.expose(), body)?;
        if reply.status != 200 {
            return Err(format!("Gemini Batch HTTP {}", reply.status));
        }
        Ok(reply.body)
    }
    /// Count prompt tokens before the spend reservation and submission.
    pub fn count_tokens(&self, model: &str, request: &Value) -> Result<u64, String> {
        model_id(model)?;
        let mut counted = request.clone();
        counted["model"] = json!(format!("models/{model}"));
        let body = serde_json::to_vec(&json!({"generateContentRequest":counted}))
            .map_err(|_| "invalid request JSON")?;
        let value = self.call(
            "POST",
            &format!("{BASE}/models/{model}:countTokens"),
            Some(&body),
        )?;
        value["totalTokens"]
            .as_u64()
            .ok_or("missing token count".into())
    }
    /// Submit an already budget-reserved batch.
    pub fn submit(&self, model: &str, body: &[u8]) -> Result<Value, String> {
        model_id(model)?;
        if body.len() >= MAX_INLINE_BYTES {
            return Err("Gemini inline batch exceeds 20 MB".into());
        }
        let result = self.call(
            "POST",
            &format!("{BASE}/models/{model}:batchGenerateContent"),
            Some(body),
        )?;
        operation_id(result["name"].as_str().ok_or("batch has no ID")?)?;
        Ok(result)
    }
    /// Poll a previously submitted operation; no provider request is repeated.
    pub fn get(&self, name: &str) -> Result<Value, String> {
        operation_id(name)?;
        self.call("GET", &format!("{BASE}/{name}"), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(ch: char) -> Result<Value, String> {
        inline_request(
            &format!("sha256:{}", ch.to_string().repeat(64)),
            json!({"contents":[{"role":"user","parts":[{"text":"bounded test"}]}]}),
        )
    }

    #[test]
    fn inline_batch_binds_request_hash_and_stays_bounded() -> Result<(), String> {
        let request = item('a')?;
        let body = submission(
            "gemini-3.8-flash",
            "r12q-test",
            std::slice::from_ref(&request),
        )?;
        let decoded: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded["batch"]["inputConfig"]["requests"]["requests"][0],
            request
        );
        assert!(submission("gemini/unsafe", "test", &[request]).is_err());
        assert!(submission("gemini-3.8-flash", "test", &[]).is_err());
        Ok(())
    }

    #[test]
    fn collection_rejects_missing_or_cross_bound_results() -> Result<(), String> {
        let a = item('a')?;
        let b = item('b')?;
        let envelope = |first: &Value, second: &Value| {
            json!({"name":"batches/abc123", "state":"BATCH_STATE_SUCCEEDED",
            "output":{"inlinedResponses":{"inlinedResponses":[
                {"metadata":first["metadata"],"response":{"modelVersion":"gemini-3.8-flash"}},
                {"metadata":second["metadata"],"response":{"modelVersion":"gemini-3.8-flash"}}
            ]}}})
        };
        assert_eq!(
            collect(&envelope(&a, &b), &[a.clone(), b.clone()])?.len(),
            2
        );
        let mut partial = envelope(&a, &b);
        partial["output"]["inlinedResponses"]["inlinedResponses"][1] =
            json!({"metadata":b["metadata"],"error":{"status":"UNAVAILABLE"}});
        assert_eq!(
            collect(&partial, &[a.clone(), b.clone()])?[1].1["batch_error_status"],
            "UNAVAILABLE"
        );
        assert!(collect(&envelope(&b, &a), &[a.clone(), b.clone()]).is_err());
        assert!(collect(&envelope(&a, &b), &[a]).is_err());
        Ok(())
    }
}
