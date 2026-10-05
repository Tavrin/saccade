//! Explicit loopback-only OpenAI-compatible local VLM adapter.
use super::{
    models::{Result, VisionError, digest},
    observation::*,
    vision::Provenance,
};
use base64::Engine;
use serde_json::{Value, json};
/// Feature-gated local external runtime, with no weight download mechanism.
pub struct LocalVlm {
    /// Explicit HTTP loopback endpoint (no remote egress, credentials or redirects).
    pub endpoint: String,
    /// Caller-recorded runtime revision (processor/quantization/template identity).
    pub runtime_revision: String,
}
impl LocalVlm {
    /// Validate a literal loopback HTTP endpoint ending in /v1/chat/completions.
    pub fn validate(&self) -> Result<()> {
        let authority = self
            .endpoint
            .strip_prefix("http://")
            .and_then(|s| s.strip_suffix("/v1/chat/completions"))
            .ok_or_else(|| {
                VisionError::Invalid(
                    "local endpoint must be literal loopback HTTP /v1/chat/completions".into(),
                )
            })?;
        let port = authority
            .strip_prefix("127.0.0.1:")
            .or_else(|| authority.strip_prefix("[::1]:"))
            .ok_or_else(|| {
                VisionError::Invalid("local endpoint must use 127.0.0.1 or [::1]".into())
            })?;
        if port.parse::<u16>().ok().is_none_or(|p| p == 0)
            || self.runtime_revision.trim().is_empty()
        {
            return Err(VisionError::Invalid("local port/runtime revision".into()));
        }
        Ok(())
    }
    /// Build a bounded structured request without performing transport.
    pub fn request(&self, r: &ObservationRequest) -> Result<Value> {
        self.validate()?;
        r.validate()?;
        if !matches!(r.model.as_str(), "qwen3.5-4b" | "florence-2-base-ft")
            || (r.model == "florence-2-base-ft" && r.task == Task::Reasoning)
        {
            return Err(VisionError::Invalid("local model/task selection".into()));
        }
        let mut content = vec![
            json!({"type":"text","text":serde_json::to_string(&json!({"task":r.task,"data":r.data,"images":r.images.iter().map(|i|json!({"id":i.id,"presented_size":i.presented_size})).collect::<Vec<_>>()}))?}),
        ];
        for i in &r.images {
            content.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}",i.media_type,base64::engine::general_purpose::STANDARD.encode(&i.bytes))}}));
        }
        Ok(
            json!({"model":r.model,"messages":[{"role":"system","content":SYSTEM},{"role":"user","content":content}],"temperature":0,"max_tokens":r.max_output_tokens,"response_format":{"type":"json_schema","json_schema":{"name":"vision_observations","strict":true,"schema":statement_schema()}}}),
        )
    }
    /// Decode a recorded or runtime response, including identity/geometry/usage checks.
    pub fn decode(&self, r: &ObservationRequest, bytes: &[u8]) -> Result<ObservationReport> {
        self.validate()?;
        r.validate()?;
        if bytes.len() > 1024 * 1024 {
            return Err(VisionError::Invalid("local response limit".into()));
        }
        let v: Value = serde_json::from_slice(bytes)?;
        if v["choices"][0]["finish_reason"] != "stop"
            || v["choices"][0]["message"]["refusal"].as_str().is_some()
        {
            return Err(VisionError::Unavailable(
                "local runtime returned refusal/incomplete output".into(),
            ));
        }
        let content = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| VisionError::Invalid("local response content".into()))?;
        let w: WireStatements = serde_json::from_str(content)?;
        let returned = v["model"]
            .as_str()
            .ok_or_else(|| VisionError::Invalid("local returned model missing".into()))?;
        let out = ObservationReport {
            schema: OBSERVATION_SCHEMA.into(),
            advisory_only: true,
            request_sha256: r.hash()?,
            response_sha256: digest(bytes),
            data_sha256: digest(r.data.as_bytes()),
            requested_model: r.model.clone(),
            returned_model: returned.into(),
            returned_revision: Some(self.runtime_revision.clone()),
            provider: "local-http".into(),
            provenance: Provenance {
                model_id: r.model.clone(),
                version: self.runtime_revision.clone(),
                artifact_sha256: vec![],
                runtime: "external-http".into(),
                input_resolution: r.images[0].presented_size,
                resolution_handling: format!("caller transforms; encoder {}", r.encoder_version),
                source_parity: false,
            },
            statements: map_statements(w, r, false)?,
            usage: Usage {
                input_tokens: v["usage"]["prompt_tokens"].as_u64(),
                output_tokens: v["usage"]["completion_tokens"].as_u64(),
                cached_input_tokens: v["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64(),
                reasoning_tokens: v["usage"]["completion_tokens_details"]["reasoning_tokens"]
                    .as_u64(),
                total_tokens: v["usage"]["total_tokens"].as_u64(),
                ..Usage::default()
            },
            cost_usd: None,
        };
        out.validate(r)?;
        Ok(out)
    }
}
impl ObservationProvider for LocalVlm {
    fn observe(&mut self, r: &ObservationRequest) -> Result<ObservationReport> {
        let payload = self.request(r)?;
        let agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(120)))
            .build()
            .new_agent();
        let mut response = agent
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .send(serde_json::to_vec(&payload)?)
            .map_err(|_| {
                VisionError::Unavailable("local runtime request failed (detail redacted)".into())
            })?;
        use std::io::Read;
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        self.decode(r, &bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_endpoint_rejects_remote_and_ambiguous_authority() {
        for endpoint in [
            "https://127.0.0.1:9000/v1/chat/completions",
            "http://example.org:9000/v1/chat/completions",
            "http://127.0.0.1:9000@evil/v1/chat/completions",
            "http://127.0.0.1:0/v1/chat/completions",
        ] {
            assert!(
                LocalVlm {
                    endpoint: endpoint.into(),
                    runtime_revision: "fixture".into()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            LocalVlm {
                endpoint: "http://[::1]:9000/v1/chat/completions".into(),
                runtime_revision: "fixture".into()
            }
            .validate()
            .is_ok()
        );
    }
}
