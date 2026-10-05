//! Startup env-file endpoint mappings; no live provider transport or authority is created.
use crate::wave7::{
    models::{self, Result, VisionError},
    observation::{ObservationReport, ObservationRequest, SYSTEM, statement_schema},
    providers::{Provider, ProviderAdapter},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
/// Endpoint deployment kind, selected only from startup-owned configuration.
#[derive(Clone, Copy)]
pub enum Kind {
    /// Responses or Chat Completions at a caller-selected OpenAI-compatible base URL.
    OpenaiCompatible,
    /// Classic Azure OpenAI deployment URL and api-key header (Chat Completions).
    AzureOpenai,
}
/// Endpoint config intentionally has no Debug/Serialize, keeping its optional secret private.
pub struct Endpoint {
    url: String,
    model: String,
    chat: bool,
    kind: Kind,
    key: Option<String>,
}
fn invalid() -> VisionError {
    VisionError::Invalid("provider endpoint configuration invalid (redacted)".into())
}
impl Endpoint {
    /// Load ~/.config/saccade/{openai-compatible,azure-openai}.env; no ambient env fallback.
    pub fn load(kind: Kind) -> Result<Self> {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| VisionError::Unavailable("credential HOME unavailable".into()))?;
        let file = match kind {
            Kind::OpenaiCompatible => "openai-compatible.env",
            Kind::AzureOpenai => "azure-openai.env",
        };
        let path = std::path::PathBuf::from(home)
            .join(".config/saccade")
            .join(file);
        Self::parse(
            kind,
            &models::read_bounded(&path, 65536).map_err(|_| {
                VisionError::Unavailable("endpoint env file unavailable (redacted)".into())
            })?,
        )
    }
    /// Parse explicit configuration bytes (also the fixture entry point). Never interpolate env values.
    pub fn parse(kind: Kind, bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
        let mut fields = BTreeMap::new();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            let (name, value) = line
                .strip_prefix("export ")
                .unwrap_or(line)
                .split_once('=')
                .ok_or_else(invalid)?;
            let value = value.trim();
            let value = if value.len() >= 2
                && (value.starts_with('"') && value.ends_with('"')
                    || value.starts_with('\'') && value.ends_with('\''))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };
            if value.is_empty()
                || value.len() > 8192
                || value.chars().any(char::is_control)
                || fields
                    .insert(name.trim().to_owned(), value.to_owned())
                    .is_some()
            {
                return Err(invalid());
            }
        }
        let get = |name: &str| fields.get(name).map(String::as_str).ok_or_else(invalid);
        let model = get("OPENAI_MODEL")?.to_owned();
        if model.len() > 256 || model.chars().any(char::is_whitespace) {
            return Err(invalid());
        }
        let base = match kind {
            Kind::OpenaiCompatible => get("OPENAI_BASE_URL")?,
            Kind::AzureOpenai => get("AZURE_OPENAI_ENDPOINT")?,
        };
        let mut url = url::Url::parse(base).map_err(|_| invalid())?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.host_str().is_none()
        {
            return Err(invalid());
        }
        let chat = match kind {
            Kind::AzureOpenai => {
                if fields
                    .get("OPENAI_API_STYLE")
                    .is_some_and(|s| s != "chat-completions")
                {
                    return Err(invalid());
                }
                true
            }
            Kind::OpenaiCompatible => match fields
                .get("OPENAI_API_STYLE")
                .map(String::as_str)
                .unwrap_or("responses")
            {
                "responses" => false,
                "chat-completions" => true,
                _ => return Err(invalid()),
            },
        };
        let key_name = match kind {
            Kind::OpenaiCompatible => "OPENAI_API_KEY",
            Kind::AzureOpenai => "AZURE_OPENAI_API_KEY",
        };
        let key = fields.get(key_name).cloned();
        if key.as_ref().is_some_and(|s| {
            s.len() > 4096 || s.chars().any(|c| c.is_control() || c.is_whitespace())
        }) {
            return Err(invalid());
        }
        match kind {
            Kind::OpenaiCompatible => {
                let mut parts = url.path_segments_mut().map_err(|_| invalid())?;
                parts.pop_if_empty();
                if chat {
                    parts.extend(["chat", "completions"]);
                } else {
                    parts.push("responses");
                }
            }
            Kind::AzureOpenai => {
                let deployment = get("AZURE_OPENAI_DEPLOYMENT")?;
                let version = get("AZURE_OPENAI_API_VERSION")?;
                if deployment.len() > 128
                    || !deployment
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
                    || version.len() > 64
                    || !version
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
                {
                    return Err(invalid());
                }
                {
                    let mut parts = url.path_segments_mut().map_err(|_| invalid())?;
                    parts.pop_if_empty().extend([
                        "openai",
                        "deployments",
                        deployment,
                        "chat",
                        "completions",
                    ]);
                }
                url.query_pairs_mut().append_pair("api-version", version);
            }
        }
        Ok(Self {
            url: url.to_string(),
            model,
            chat,
            kind,
            key,
        })
    }
    /// Safe endpoint URL (credentials/query fragments were refused during parsing).
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Header name; authentication values are never present in a mapping/receipt.
    pub fn auth_header_name(&self) -> &'static str {
        match self.kind {
            Kind::OpenaiCompatible => "Authorization",
            Kind::AzureOpenai => "api-key",
        }
    }
    /// Explicit secret handoff for a separately authorized transport. Never log this result.
    pub fn header_for_transport(&self) -> Result<(&'static str, String)> {
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| VisionError::Unavailable("provider key missing (redacted)".into()))?;
        Ok((
            self.auth_header_name(),
            match self.kind {
                Kind::OpenaiCompatible => format!("Bearer {key}"),
                Kind::AzureOpenai => key.clone(),
            },
        ))
    }
    fn request(&self, adapter: &ProviderAdapter, r: &ObservationRequest) -> Result<Value> {
        if !matches!(adapter.provider, Provider::Gpt) || r.model != self.model {
            return Err(VisionError::Invalid(
                "configured GPT model/adapter mismatch".into(),
            ));
        }
        let body = adapter.request_checked(r, false)?;
        if !self.chat {
            return Ok(body);
        }
        let content = body["input"][0]["content"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|v| match v["type"].as_str() {
                Some("input_text") => json!({"type":"text","text":v["text"]}),
                _ => json!({"type":"image_url","image_url":{"url":v["image_url"],"detail":"auto"}}),
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"model":r.model,"messages":[{"role":"system","content":SYSTEM},{"role":"user","content":content}],"max_completion_tokens":r.max_output_tokens,"response_format":{"type":"json_schema","json_schema":{"name":"vision_observations","strict":true,"schema":statement_schema()}}}),
        )
    }
    /// Fixture/request mapping only; no key value, provider call or transport authorization.
    pub fn mapping(&self, adapter: &ProviderAdapter, r: &ObservationRequest) -> Result<Value> {
        Ok(
            json!({"schema":"saccade-provider-mapping.v1","interface_only":true,"endpoint":self.url,"auth_header":self.auth_header_name(),"request_sha256":r.hash()?,"body":self.request(adapter,r)?}),
        )
    }
    /// Decode a recorded Responses/Chat fixture through wave 7's closed observation checks.
    pub fn decode(
        &self,
        adapter: &ProviderAdapter,
        r: &ObservationRequest,
        bytes: &[u8],
    ) -> Result<ObservationReport> {
        self.request(adapter, r)?;
        if bytes.len() > 1024 * 1024 {
            return Err(VisionError::Invalid("provider response bound".into()));
        }
        let normalized = if self.chat {
            let v: Value = serde_json::from_slice(bytes)?;
            let choices = v["choices"].as_array().ok_or_else(invalid)?;
            if choices.len() != 1
                || choices[0]["finish_reason"] != "stop"
                || choices[0]["message"]["role"] != "assistant"
                || !choices[0]["message"]["refusal"].is_null()
                || !choices[0]["message"]["tool_calls"].is_null()
            {
                return Err(VisionError::Unavailable(
                    "provider refused or returned incomplete/non-observation output".into(),
                ));
            }
            let text = choices[0]["message"]["content"]
                .as_str()
                .ok_or_else(invalid)?;
            serde_json::to_vec(
                &json!({"status":"completed","model":v["model"],"system_fingerprint":v["system_fingerprint"],"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}],"usage":{"input_tokens":v["usage"]["prompt_tokens"],"output_tokens":v["usage"]["completion_tokens"],"total_tokens":v["usage"]["total_tokens"],"input_tokens_details":v["usage"]["prompt_tokens_details"],"output_tokens_details":v["usage"]["completion_tokens_details"]}}),
            )?
        } else {
            bytes.to_vec()
        };
        let mut report = adapter.decode_checked(r, &normalized, false)?;
        report.response_sha256 = models::digest(bytes);
        report.provider = match self.kind {
            Kind::OpenaiCompatible => "openai-compatible-fixture",
            Kind::AzureOpenai => "azure-openai-fixture",
        }
        .into();
        report.validate(r)?;
        Ok(report)
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::wave7::{
        observation::{ImageInput, Task},
        providers::Coordinates,
    };
    fn request() -> ObservationRequest {
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([50, 70, 90]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        ObservationRequest {
            task: Task::Caption,
            data: "describe image data".into(),
            images: vec![ImageInput {
                id: "image".into(),
                bytes: bytes.into_inner(),
                media_type: "image/png".into(),
                original_size: [8, 8],
                presented_size: [8, 8],
                scale: [1., 1.],
                offset: [0., 0.],
            }],
            model: "configured-model".into(),
            encoder_version: "fixture/1".into(),
            max_output_tokens: 128,
        }
    }
    #[test]
    fn compatible_responses_and_azure_chat_urls_auth_and_mapping() {
        let r = request();
        let a = ProviderAdapter {
            provider: Provider::Gpt,
            coordinates: Coordinates::Pixels,
        };
        let endpoint=Endpoint::parse(Kind::OpenaiCompatible,b"OPENAI_BASE_URL=https://api.example.org/v1/\nOPENAI_MODEL=configured-model\nOPENAI_API_KEY=fixture-secret\n").unwrap();
        assert_eq!(endpoint.url(), "https://api.example.org/v1/responses");
        let mapping = endpoint.mapping(&a, &r).unwrap();
        assert!(!mapping.to_string().contains("fixture-secret"));
        assert_eq!(endpoint.header_for_transport().unwrap().0, "Authorization");
        let azure=Endpoint::parse(Kind::AzureOpenai,b"AZURE_OPENAI_ENDPOINT=https://resource.example.org/\nAZURE_OPENAI_DEPLOYMENT=deployment-one\nAZURE_OPENAI_API_VERSION=2025-04-01-preview\nOPENAI_MODEL=configured-model\nAZURE_OPENAI_API_KEY=fixture-secret\n").unwrap();
        assert_eq!(
            azure.url(),
            "https://resource.example.org/openai/deployments/deployment-one/chat/completions?api-version=2025-04-01-preview"
        );
        assert_eq!(azure.header_for_transport().unwrap().0, "api-key");
        assert_eq!(
            azure.mapping(&a, &r).unwrap()["body"]["messages"][1]["content"][1]["type"],
            "image_url"
        );
        let statement=serde_json::to_string(&json!({"statements":[{"image_id":"image","text":"draft","bbox":null,"point":null,"confidence":null}]})).unwrap();
        let response=serde_json::to_vec(&json!({"model":r.model,"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":statement}}],"usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}})).unwrap();
        let report = azure.decode(&a, &r, &response).unwrap();
        assert_eq!(report.response_sha256, models::digest(&response));
        assert_eq!(report.statements[0].text, "draft");
        assert_eq!(report.usage.total_tokens, Some(20));
    }
    #[test]
    fn invalid_config_redacts_and_truncation_fails() {
        for bytes in [
            b"OPENAI_BASE_URL=https://user:secret@example.org\nOPENAI_MODEL=configured-model"
                .as_slice(),
            b"OPENAI_BASE_URL=https://example.org/?key=secret\nOPENAI_MODEL=configured-model"
                .as_slice(),
        ] {
            let error = Endpoint::parse(Kind::OpenaiCompatible, bytes)
                .err()
                .unwrap()
                .to_string();
            assert!(!error.contains("secret"));
        }
        let r = request();
        let a = ProviderAdapter {
            provider: Provider::Gpt,
            coordinates: Coordinates::Pixels,
        };
        let e=Endpoint::parse(Kind::OpenaiCompatible,b"OPENAI_BASE_URL=http://127.0.0.1:1234/v1\nOPENAI_MODEL=configured-model\nOPENAI_API_STYLE=chat-completions").unwrap();
        assert!(e.decode(&a,&r,&serde_json::to_vec(&json!({"model":r.model,"choices":[{"finish_reason":"length","message":{"role":"assistant","content":"{}"}}]})).unwrap()).is_err());
    }
}
