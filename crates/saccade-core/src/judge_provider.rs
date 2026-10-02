//! Judge providers: how a question reaches a model and how its answer comes
//! back, with retries, a fallback chain and the key policy.
//!
//! **Keys** are read from exactly two kinds of place: the files
//! `jev.env` (`JEV_API_KEY`) and `gemini.env` (`SACCADE_GEMINI_API_KEY`) in
//! `~/.config/saccade` (or the directory given with `--keys-dir`), and, for
//! the generic `openai_compatible` provider, a file named by the panel inside
//! that same directory. The ambient environment (`GEMINI_API_KEY` and the
//! like), other projects' `.env` files and global configs are never read. A
//! key is never printed, logged, stored in a result or sent anywhere but its
//! provider's endpoint, and it travels in an HTTP header inside this process,
//! never on a command line.
//!
//! Request shapes follow the providers' documentation, read as data:
//! <https://docs.typesafe.ai/api.md> (Jev `POST /v1/systemone`, choice
//! questions answer with a probability distribution) and
//! <https://ai.google.dev/gemini-api/docs> (Gemini `generateContent`, key in
//! the `x-goog-api-key` header, JSON mode through `responseMimeType`).

use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use crate::judge::{JudgeSpec, Provider};

/// A secret that never prints.
#[derive(Clone)]
pub struct Secret(String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl Secret {
    /// The secret text; use it only to build a request header.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `text` with every occurrence of the secret replaced by `***`.
    pub fn scrub(&self, text: &str) -> String {
        text.replace(&self.0, "***")
    }
}

/// Where keys are looked up.
#[derive(Debug, Clone)]
pub struct Keys {
    dir: PathBuf,
    explicit: bool,
}

impl Keys {
    /// Keys in `dir`, or in `~/.config/saccade` without one.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let explicit = dir.is_some();
        Self {
            dir: dir.unwrap_or_else(Self::default_dir),
            explicit,
        }
    }

    /// `~/.config/saccade` (`$HOME`, never `$XDG_CONFIG_HOME`: the policy names one place).
    pub fn default_dir() -> PathBuf {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(".config").join("saccade"))
            .unwrap_or_default()
    }

    /// The key variable `var` from the file `file` of the keys directory.
    /// `file` must be a plain file name. The error never contains a key.
    pub fn load(&self, file: &str, var: &str) -> Result<Secret, String> {
        if self.dir.as_os_str().is_empty() {
            return Err("no home directory: supply --keys-dir".into());
        }
        if file.is_empty() || file.contains(['/', '\\']) || file == "." || file == ".." {
            return Err(format!("key file name {file:?} must be a plain file name"));
        }
        let path = self.dir.join(file);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("key files must be ordinary files inside the keys directory".into());
        }
        let text = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "no key: cannot read {} ({e}); keys are read only from files in the keys directory, never from the environment",
                path.display()
            )
        })?;
        parse_env(&text, var)
            .map(Secret)
            .ok_or_else(|| format!("no key: {} has no {var} line", path.display()))
    }

    /// The key a provider needs, per the key policy.
    pub fn for_spec(&self, spec: &JudgeSpec) -> Result<Option<Secret>, String> {
        match spec.provider {
            Provider::Jev => self.load("jev.env", "JEV_API_KEY").map(Some),
            Provider::Gemini => self.load("gemini.env", "SACCADE_GEMINI_API_KEY").map(Some),
            Provider::OpenaiCompatible => match &spec.key_file {
                Some(_) if !self.explicit => {
                    Err("openai_compatible keys require an explicit --keys-dir".into())
                }
                Some(f) => self
                    .load(f, spec.key_var.as_deref().unwrap_or("SACCADE_API_KEY"))
                    .map(Some),
                None => Ok(None),
            },
            Provider::Opencode | Provider::Human => Ok(None),
        }
    }
}

/// The value of `var` in `KEY=VALUE` lines (`export` prefix, quotes and comments tolerated).
fn parse_env(text: &str, var: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let (k, v) = line.split_once('=')?;
        if line.starts_with('#') || k.trim() != var {
            return None;
        }
        let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        (!v.is_empty()).then(|| v.to_owned())
    })
}

/// The text a model is shown.
#[derive(Debug, Clone)]
pub struct Prompt {
    /// Role and rubric.
    pub system: String,
    /// The question, the allowed answers and the evidence.
    pub user: String,
}

/// One call to make.
pub struct AskRequest<'a> {
    /// The judge asked.
    pub spec: &'a JudgeSpec,
    /// The question type name.
    pub question: &'a str,
    /// The fixed wording of the question.
    pub question_text: &'a str,
    /// The question kind (`checkable`, `rubric`, `preference`).
    pub kind: &'a str,
    /// The answers the model may give, abstain option included.
    pub wire_answers: &'a [String],
    /// The evidence, with the sides labelled for this order.
    pub state: &'a Value,
    /// The text prompt (text and vision judges).
    pub prompt: &'a Prompt,
    /// PNG strips for a vision judge.
    pub images: &'a [Vec<u8>],
}

/// One model tried for a call.
#[derive(Debug, Clone, Serialize)]
pub struct Attempt {
    /// The model asked.
    pub model: String,
    /// Requests sent (first try plus retries).
    pub tries: u32,
    /// HTTP statuses seen, in order (empty for a CLI provider).
    pub statuses: Vec<u16>,
    /// Why it failed, secrets scrubbed; `None` on success.
    pub error: Option<String>,
    /// Whether this attempt produced the answer.
    pub ok: bool,
}

/// A model's raw answer.
#[derive(Debug, Clone)]
pub struct Raw {
    /// One of the wire answers.
    pub answer: String,
    /// Probability the model gives its answer (distributional or verbalised).
    pub prob: Option<f64>,
    /// A second confidence figure, when the provider reports one.
    pub confidence: Option<f64>,
    /// The full distribution, when the provider reports one.
    pub probs: BTreeMap<String, f64>,
    /// `model_distribution` or `verbalized`.
    pub prob_source: &'static str,
    /// The version string the provider reports.
    pub model_version: String,
    /// The model that answered (differs from the requested one after a fallback).
    pub model: String,
    /// Wall time of the successful request.
    pub latency_ms: u64,
    /// Provider-reported token usage (Gemini `usageMetadata`, Jev/chat `usage`).
    pub usage: Value,
}

/// What a call produced.
pub struct CallOutcome {
    /// The answer, or why there is none (the judge then abstains).
    pub result: Result<Raw, String>,
    /// Every model tried.
    pub attempts: Vec<Attempt>,
}

/// Something that can answer a question: the live providers, or a test double.
pub trait Backend {
    /// Actual network attempts when the backend tracks them; mocks may omit this.
    fn http_counts(&self) -> Option<[usize; 2]> {
        None
    }

    /// Asks one judge one question.
    fn ask(&self, req: &AskRequest<'_>) -> CallOutcome;
    /// Batch compatible questions. The default asks individually; Jev sends
    /// one indexed state array with many choice questions per HTTP call.
    fn ask_many(&self, reqs: &[AskRequest<'_>]) -> Vec<CallOutcome> {
        reqs.iter().map(|r| self.ask(r)).collect()
    }
}

/// A failed request.
#[derive(Debug)]
pub struct CallError {
    status: Option<u16>,
    message: String,
    retryable: bool,
    retry_after: Option<u64>,
}

impl CallError {
    fn fatal(message: impl Into<String>) -> Self {
        Self {
            status: None,
            message: message.into(),
            retryable: false,
            retry_after: None,
        }
    }
}

/// Retry settings.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    /// Retries after the first try (429, 5xx and transport errors only).
    pub max_retries: u32,
    /// First backoff in milliseconds; doubles per retry, capped at 30 s.
    pub base_ms: u64,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_ms: 1000,
        }
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(char::from(T[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The first JSON object in `text` (a model may wrap it in prose or fences).
fn first_object(text: &str) -> Option<Value> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(off) = text[start..].find('{') {
        let s = start + off;
        let (mut depth, mut in_str, mut esc) = (0i32, false, false);
        for (i, &b) in bytes[s..].iter().enumerate() {
            if in_str {
                match (esc, b) {
                    (true, _) => esc = false,
                    (false, b'\\') => esc = true,
                    (false, b'"') => in_str = false,
                    _ => {}
                }
                continue;
            }
            match b {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Ok(v) = serde_json::from_str::<Value>(&text[s..=s + i]) {
                            return Some(v);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        start = s + 1;
    }
    None
}

/// Reads `{"answer": ..., "probability": ...}` from a model's text.
pub fn parse_llm_answer(text: &str, allowed: &[String]) -> Result<(String, Option<f64>), String> {
    let v = first_object(text).ok_or_else(|| "the reply holds no JSON object".to_owned())?;
    let raw = v["answer"]
        .as_str()
        .ok_or_else(|| "the reply has no string `answer`".to_owned())?
        .trim();
    let answer = allowed
        .iter()
        .find(|a| a.eq_ignore_ascii_case(raw))
        .ok_or_else(|| format!("answer {raw:?} is not one of: {}", allowed.join(", ")))?
        .clone();
    let prob = ["probability", "prob", "confidence"]
        .iter()
        .find_map(|k| v[*k].as_f64())
        .map(|p| if p > 1.0 && p <= 100.0 { p / 100.0 } else { p })
        .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
        .ok_or_else(|| "the reply needs a finite probability from 0 to 1".to_owned())?;
    Ok((answer, Some(prob)))
}

/// What each answer means to Jev (the criteria of its choice question).
fn answer_help(a: &str) -> &'static str {
    match a {
        "accept" => "the visible change matches the intent text and is intended",
        "reject" => "the change is a regression or does not match the intent",
        "needs_human" | "unsure" => "the evidence does not settle it; a person must look",
        "noise" => "dithering or sampling noise with no real content change",
        "local_defect" => "a localised artefact in a small region",
        "global_shift" => "a whole-frame change in tone, exposure or colour",
        "config_mismatch" => "the two captures were made with different settings",
        "broken_frame" => "the capture is unusable: black, white or non-finite",
        "global_tone" => "one tone curve (exposure, tint) explains the difference",
        "local_structure" => "geometry or content changed in places",
        "misaligned" => "the same content, offset by a shift",
        "noise_region" => "this hotspot is noise worth masking",
        "real_change" => "this hotspot is a real change",
        "yes" => "a person must look at this",
        "no" => "it can be decided without a person",
        "P1" => "the first image is better by the stated criterion",
        "P2" => "the second image is better by the stated criterion",
        "tie" => "the two are equally good by the stated criterion",
        _ => "",
    }
}

/// The real providers.
pub struct LiveBackend {
    keys: Keys,
    retry: Retry,
    agent: ureq::Agent,
    limits: Option<(usize, usize, usize)>,
    counts: [Cell<usize>; 2],
    cache: Option<PathBuf>,
    cooldown_secs: u64,
    run_id: String,
    #[cfg(test)]
    mock_replies: Option<RefCell<Vec<(u16, Value)>>>,
}

impl LiveBackend {
    /// A backend with the given key directory and retry policy.
    pub fn new(keys: Keys, retry: Retry, timeout: Duration) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            keys,
            retry,
            agent,
            limits: None,
            counts: [Cell::new(0), Cell::new(0)],
            cache: None,
            cooldown_secs: 600,
            run_id: crate::local::random_token(),
            #[cfg(test)]
            mock_replies: None,
        }
    }

    /// Bound actual HTTP attempts, including retries and fallback probes.
    /// Cooldowns are persisted per Gemini model in the cache.
    pub fn with_policy(
        mut self,
        cache: PathBuf,
        cooldown_secs: u64,
        total: usize,
        jev: usize,
        gemini: usize,
    ) -> Self {
        self.cache = Some(cache);
        self.cooldown_secs = cooldown_secs;
        self.limits = Some((total, jev, gemini));
        self
    }

    /// Actual Jev and Gemini HTTP attempts sent.
    pub fn counts(&self) -> [usize; 2] {
        [self.counts[0].get(), self.counts[1].get()]
    }

    fn cooldown_path(&self, model: &str) -> Option<PathBuf> {
        use sha2::{Digest, Sha256};
        let hash: String = Sha256::digest(model.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.cache
            .as_ref()
            .map(|c| c.join("gemini-cooldowns").join(format!("{hash}.json")))
    }

    fn cooling(&self, model: &str) -> bool {
        self.cooldown_path(model)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| {
                v["until_ms"]
                    .as_u64()
                    .is_some_and(|t| t > crate::judge::now_ms())
            })
    }

    fn mark_failed(&self, model: &str) -> Result<(), String> {
        if let Some(p) = self.cooldown_path(model) {
            let parent = p.parent().ok_or("invalid cooldown path")?;
            std::fs::create_dir_all(parent).map_err(|e| format!("creating cooldown cache: {e}"))?;
            use std::io::Write;
            let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
            let body = json!({"model": model, "until_ms": crate::judge::now_ms().saturating_add(self.cooldown_secs.saturating_mul(1000))});
            tmp.write_all(body.to_string().as_bytes())
                .map_err(|e| e.to_string())?;
            tmp.persist(p)
                .map_err(|e| format!("persisting cooldown: {e}"))?;
        }
        Ok(())
    }

    fn post(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &Value,
        secret: Option<&Secret>,
    ) -> Result<Value, CallError> {
        let index = usize::from(url.contains("generativelanguage.googleapis.com"));
        let counts = self.counts();
        if self.limits.is_some_and(|(total, jev, gemini)| {
            counts.iter().sum::<usize>() >= total || counts[index] >= [jev, gemini][index]
        }) {
            return Err(CallError::fatal("HTTP call budget reached"));
        }
        if let Some(cache) = &self.cache {
            std::fs::create_dir_all(cache)
                .map_err(|_| CallError::fatal("cannot create provider attempt ledger"))?;
            use std::io::Write;
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(cache.join("http-attempts.jsonl"))
                .map_err(|_| CallError::fatal("cannot open provider attempt ledger"))?;
            let entry = json!({"run_id":self.run_id,"provider":if index==0 {"jev"}else{"gemini"},
                "model":body["model"].as_str().map(str::to_owned).unwrap_or_else(||url.rsplit('/').next().unwrap_or("").trim_end_matches(":generateContent").to_owned()),
                "started_ms":crate::judge::now_ms()});
            writeln!(log, "{entry}")
                .and_then(|()| log.flush())
                .map_err(|_| CallError::fatal("cannot persist provider attempt ledger"))?;
        }
        self.counts[index].set(counts[index] + 1);
        #[cfg(test)]
        if let Some(replies) = &self.mock_replies {
            let (status, value) = replies.borrow_mut().remove(0);
            if status == 200 {
                return Ok(value);
            }
            return Err(CallError {
                status: Some(status),
                message: format!("mock HTTP {status}"),
                retryable: matches!(status, 429 | 503),
                retry_after: None,
            });
        }
        let scrub = |s: String| secret.map_or(s.clone(), |k| k.scrub(&s));
        let mut req = self
            .agent
            .post(url)
            .header("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut resp = req.send(body.to_string().as_str()).map_err(|e| CallError {
            status: None,
            message: scrub(format!("transport error: {e}")),
            retryable: true,
            retry_after: None,
        })?;
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| CallError::fatal(scrub(format!("reading the reply: {e}"))))?;
        if !(200..300).contains(&status) {
            let snippet: String = text.chars().take(200).collect();
            return Err(CallError {
                status: Some(status),
                message: scrub(format!("HTTP {status}: {snippet}")),
                retryable: matches!(status, 429 | 500 | 502 | 503 | 504),
                retry_after,
            });
        }
        serde_json::from_str(&scrub(text))
            .map_err(|e| CallError::fatal(scrub(format!("the reply is not JSON: {e}"))))
    }

    fn ask_jev(&self, req: &AskRequest<'_>, model: &str, key: &Secret) -> Result<Raw, CallError> {
        let criteria: serde_json::Map<String, Value> = req
            .wire_answers
            .iter()
            .map(|a| (a.clone(), json!(answer_help(a))))
            .collect();
        let body = json!({
            "model": model,
            "state": req.state,
            "questions": {"q": {
                "type": "choice",
                "instructions": json!({
                    "question": req.question_text,
                    "question_kind": req.kind,
                    "role": req.spec.role,
                    "rubric": req.spec.rubric,
                    "note": "The state is evidence about two images; you see no pixels. Choose `unsure` when it does not settle the question.",
                }).to_string(),
                "criteria": criteria,
            }},
        });
        let started = Instant::now();
        let reply = self.post(
            "https://api.typesafe.ai/v1/systemone",
            &[("Authorization", &format!("Bearer {}", key.expose()))],
            &body,
            Some(key),
        )?;
        let a = &reply["answers"]["q"];
        let choice = a["choice"]
            .as_str()
            .ok_or_else(|| CallError::fatal("the reply has no `choice` answer".to_owned()))?;
        if !req.wire_answers.iter().any(|w| w == choice) {
            return Err(CallError::fatal(format!(
                "answer {choice:?} is not one of: {}",
                req.wire_answers.join(", ")
            )));
        }
        let probs: BTreeMap<String, f64> = a["probabilities"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_f64().map(|p| (k.clone(), p)))
                    .collect()
            })
            .unwrap_or_default();
        if probs.len() != req.wire_answers.len()
            || probs.iter().any(|(k, p)| {
                !req.wire_answers.contains(k) || !p.is_finite() || !(0.0..=1.0).contains(p)
            })
            || (probs.values().sum::<f64>() - 1.0).abs() > 0.02
        {
            return Err(CallError::fatal("invalid Jev probability distribution"));
        }
        Ok(Raw {
            answer: choice.to_owned(),
            prob: probs.get(choice).copied(),
            confidence: a["confidence"].as_f64(),
            probs,
            prob_source: "model_distribution",
            model_version: reply["model"].as_str().unwrap_or(model).to_owned(),
            model: model.to_owned(),
            latency_ms: started.elapsed().as_millis() as u64,
            usage: reply["usage"].clone(),
        })
    }

    fn ask_gemini(
        &self,
        req: &AskRequest<'_>,
        model: &str,
        key: &Secret,
    ) -> Result<Raw, CallError> {
        let mut parts = vec![json!({"text": req.prompt.user})];
        for png in req.images {
            parts.push(json!({"inline_data": {"mime_type": "image/png", "data": b64(png)}}));
        }
        let body = json!({
            "systemInstruction": {"parts": [{"text": req.prompt.system}]},
            "contents": [{"role": "user", "parts": parts}],
            "generationConfig": {
                "maxOutputTokens": 2048,
                "responseMimeType": "application/json",
                "responseSchema": {
                    "type": "OBJECT",
                    "properties": {
                        "answer": {"type": "STRING", "enum": req.wire_answers},
                        "probability": {"type": "NUMBER"},
                    },
                    "required": ["answer", "probability"],
                },
            },
        });
        let started = Instant::now();
        let reply = self.post(
            &format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ),
            &[("x-goog-api-key", key.expose())],
            &body,
            Some(key),
        )?;
        if let Some(r) = reply["promptFeedback"]["blockReason"].as_str() {
            return Err(CallError::fatal(format!("the prompt was blocked: {r}")));
        }
        let text: String = reply["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|ps| {
                ps.iter()
                    .filter(|p| p["thought"].as_bool() != Some(true))
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();
        let (answer, prob) = parse_llm_answer(&text, req.wire_answers).map_err(CallError::fatal)?;
        Ok(Raw {
            answer,
            prob,
            confidence: None,
            probs: BTreeMap::new(),
            prob_source: "verbalized",
            model_version: reply["modelVersion"].as_str().unwrap_or(model).to_owned(),
            model: model.to_owned(),
            latency_ms: started.elapsed().as_millis() as u64,
            usage: reply["usageMetadata"].clone(),
        })
    }

    fn ask_gemini_many(&self, reqs: &[AskRequest<'_>]) -> Vec<CallOutcome> {
        let spec = reqs[0].spec;
        let key = match self.keys.for_spec(spec) {
            Ok(Some(k)) => k,
            _ => return reqs.iter().map(|r| self.ask(r)).collect(),
        };
        let mut attempts = Vec::new();
        let mut last = "no available Gemini model".to_owned();
        let started = Instant::now();
        for model in std::iter::once(&spec.model).chain(&spec.fallback) {
            if self.cooling(model) {
                attempts.push(Attempt {
                    model: model.clone(),
                    tries: 0,
                    statuses: Vec::new(),
                    error: Some("model in persistent cooldown".into()),
                    ok: false,
                });
                continue;
            }
            let mut parts = Vec::new();
            for (i, r) in reqs.iter().enumerate() {
                parts.push(json!({"text":format!("Item {i}: {}\nThe following images belong only to item {i}.",r.prompt.user)}));
                for img in r.images {
                    parts.push(json!({"inline_data":{"mime_type":"image/png","data":b64(img)}}));
                }
            }
            let body = json!({
                "systemInstruction":{"parts":[{"text":format!("{}\nAnswer every indexed item independently. Return {{\"answers\":[{{\"index\":0,\"answer\":\"...\",\"probability\":0.9}}]}}. Do not omit any item.",reqs[0].prompt.system)}]},
                "contents":[{"role":"user","parts":parts}],
                "generationConfig":{"maxOutputTokens":8192,"responseMimeType":"application/json",
                    "responseSchema":{"type":"OBJECT","properties":{"answers":{"type":"ARRAY","items":{
                        "type":"OBJECT","properties":{"index":{"type":"INTEGER"},"answer":{"type":"STRING"},"probability":{"type":"NUMBER"}},
                        "required":["index","answer","probability"]}}},"required":["answers"]}}
            });
            let mut att = Attempt {
                model: model.clone(),
                tries: 0,
                statuses: Vec::new(),
                error: None,
                ok: false,
            };
            let mut backoff = self.retry.base_ms;
            loop {
                att.tries += 1;
                let parsed = self.post(&format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"),
                    &[("x-goog-api-key",key.expose())],&body,Some(&key)).and_then(|v| {
                    let text = v["candidates"][0]["content"]["parts"].as_array().into_iter().flatten()
                        .filter_map(|p|p["text"].as_str()).collect::<String>();
                    let value = first_object(&text).ok_or_else(||CallError::fatal("Gemini batch contains no JSON"))?;
                    let answers = value["answers"].as_array().ok_or_else(||CallError::fatal("missing Gemini batch answers"))?;
                    if answers.len()!=reqs.len() { return Err(CallError::fatal("incomplete Gemini batch")); }
                    reqs.iter().enumerate().map(|(i,r)| {
                        let matches: Vec<_> = answers.iter().filter(|a|a["index"].as_u64()==Some(i as u64)).collect();
                        if matches.len()!=1 { return Err(CallError::fatal("duplicate or missing Gemini batch index")); }
                        let (answer,prob) = parse_llm_answer(&matches[0].to_string(),r.wire_answers).map_err(CallError::fatal)?;
                        Ok(Raw { answer,prob,confidence:None,probs:BTreeMap::new(),prob_source:"verbalized",
                            model_version:v["modelVersion"].as_str().unwrap_or(model).into(),model:model.clone(),
                            latency_ms:started.elapsed().as_millis() as u64,usage:v["usageMetadata"].clone() })
                    }).collect::<Result<Vec<_>,CallError>>()
                });
                match parsed {
                    Ok(raws) => {
                        att.ok = true;
                        attempts.push(att);
                        return raws
                            .into_iter()
                            .map(|r| CallOutcome {
                                result: Ok(r),
                                attempts: attempts.clone(),
                            })
                            .collect();
                    }
                    Err(e) => {
                        if e.message == "HTTP call budget reached" {
                            att.tries = att.tries.saturating_sub(1);
                        }
                        att.statuses.extend(e.status);
                        att.error = Some(e.message.clone());
                        last = e.message;
                        if !e.retryable || att.tries > self.retry.max_retries {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(
                            e.retry_after
                                .map_or(backoff, |s| s.saturating_mul(1000))
                                .min(30_000),
                        ));
                        backoff = backoff.saturating_mul(2);
                    }
                }
            }
            if last != "HTTP call budget reached"
                && let Err(e) = self.mark_failed(model)
            {
                last.push_str(&format!("; {e}"));
            }
            attempts.push(att);
            if last == "HTTP call budget reached" {
                break;
            }
        }
        reqs.iter()
            .map(|_| CallOutcome {
                result: Err(last.clone()),
                attempts: attempts.clone(),
            })
            .collect()
    }

    fn ask_openai(
        &self,
        req: &AskRequest<'_>,
        model: &str,
        key: Option<&Secret>,
    ) -> Result<Raw, CallError> {
        let base = req
            .spec
            .base_url
            .as_deref()
            .ok_or_else(|| CallError::fatal("openai_compatible needs base_url"))?;
        let content = if req.images.is_empty() {
            json!(req.prompt.user)
        } else {
            let mut parts = vec![json!({"type": "text", "text": req.prompt.user})];
            parts.extend(req.images.iter().map(|png| json!({
                "type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", b64(png))}
            })));
            json!(parts)
        };
        let body = json!({
            "model": model,
            "temperature": 0,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role": "system", "content": req.prompt.system},
                {"role": "user", "content": content},
            ],
        });
        let auth = key.map(|k| format!("Bearer {}", k.expose()));
        let headers: Vec<(&str, &str)> = auth
            .as_deref()
            .map(|a| vec![("Authorization", a)])
            .unwrap_or_default();
        let started = Instant::now();
        let reply = self.post(
            &format!("{}/chat/completions", base.trim_end_matches('/')),
            &headers,
            &body,
            key,
        )?;
        let text = reply["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default();
        let (answer, prob) = parse_llm_answer(text, req.wire_answers).map_err(CallError::fatal)?;
        Ok(Raw {
            answer,
            prob,
            confidence: None,
            probs: BTreeMap::new(),
            prob_source: "verbalized",
            model_version: reply["model"].as_str().unwrap_or(model).to_owned(),
            model: model.to_owned(),
            latency_ms: started.elapsed().as_millis() as u64,
            usage: reply["usage"].clone(),
        })
    }

    fn ask_opencode(&self, req: &AskRequest<'_>, model: &str) -> Result<Raw, CallError> {
        let prompt = format!("{}\n\n{}", req.prompt.system, req.prompt.user);
        let started = Instant::now();
        // Free models need no credentials. Isolate all config and project files.
        let isolated = tempfile::tempdir().map_err(|e| CallError::fatal(e.to_string()))?;
        let mut command = Command::new("opencode");
        command.env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command
            .env("HOME", isolated.path())
            .env("XDG_CONFIG_HOME", isolated.path())
            .env("XDG_DATA_HOME", isolated.path())
            .env("XDG_CACHE_HOME", isolated.path())
            .env(
                "OPENCODE_CONFIG_CONTENT",
                "{\"permission\":{\"*\":\"deny\"}}",
            )
            .current_dir(isolated.path())
            .args(["run", "--pure", "-m", model]);
        let mut attachments = Vec::new();
        let stderr_path = isolated.path().join("stderr.txt");
        let stderr =
            std::fs::File::create(&stderr_path).map_err(|e| CallError::fatal(e.to_string()))?;
        for (i, png) in req.images.iter().enumerate() {
            let path = isolated.path().join(format!("strip-{i}.png"));
            std::fs::write(&path, png).map_err(|e| CallError::fatal(e.to_string()))?;
            attachments.push(path);
        }
        for path in &attachments {
            command.arg("--file").arg(path);
        }
        let mut child = command
            .args(["--", &prompt])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|e| CallError::fatal(format!("running `opencode`: {e}")))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| CallError::fatal("no stdout from opencode"))?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
            let _ = tx.send(buf);
        });
        let out = match rx.recv_timeout(Duration::from_secs(u64::from(req.spec.timeout_secs))) {
            Ok(o) => o,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CallError {
                    status: None,
                    message: "opencode timed out".into(),
                    retryable: true,
                    retry_after: None,
                });
            }
        };
        let status = child.wait().map_err(|e| CallError::fatal(e.to_string()))?;
        if !status.success() {
            let detail: String = std::fs::read_to_string(stderr_path)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect();
            return Err(CallError::fatal(format!(
                "opencode exited with {status}: {detail}"
            )));
        }
        let text = String::from_utf8_lossy(&out);
        let (answer, prob) = parse_llm_answer(&text, req.wire_answers).map_err(CallError::fatal)?;
        Ok(Raw {
            answer,
            prob,
            confidence: None,
            probs: BTreeMap::new(),
            prob_source: "verbalized",
            model_version: model.to_owned(),
            model: model.to_owned(),
            latency_ms: started.elapsed().as_millis() as u64,
            usage: Value::Null,
        })
    }

    fn once(
        &self,
        req: &AskRequest<'_>,
        model: &str,
        key: Option<&Secret>,
    ) -> Result<Raw, CallError> {
        match (req.spec.provider, key) {
            (Provider::Jev, Some(k)) => self.ask_jev(req, model, k),
            (Provider::Gemini, Some(k)) => self.ask_gemini(req, model, k),
            (Provider::OpenaiCompatible, k) => self.ask_openai(req, model, k),
            (Provider::Opencode, _) => self.ask_opencode(req, model),
            _ => Err(CallError::fatal("no key for this provider")),
        }
    }
}

impl Backend for LiveBackend {
    fn http_counts(&self) -> Option<[usize; 2]> {
        Some(self.counts())
    }

    fn ask_many(&self, reqs: &[AskRequest<'_>]) -> Vec<CallOutcome> {
        let Some(first) = reqs.first() else {
            return Vec::new();
        };
        if first.spec.provider == Provider::Gemini && reqs.len() > 1 {
            return self.ask_gemini_many(reqs);
        }
        if first.spec.provider != Provider::Jev || reqs.len() == 1 {
            return reqs.iter().map(|r| self.ask(r)).collect();
        }
        let key = match self.keys.for_spec(first.spec) {
            Ok(Some(k)) => k,
            _ => return reqs.iter().map(|r| self.ask(r)).collect(),
        };
        let questions: serde_json::Map<String, Value> = reqs.iter().enumerate().map(|(i,r)| {
            let criteria: serde_json::Map<String, Value> = r.wire_answers.iter()
                .map(|a| (a.clone(), json!(answer_help(a)))).collect();
            let noul=r.question=="needs_eyes";
            (format!("q{i}"), json!({"type":if noul {"noul"}else{"choice"}, "instructions":format!(
                "Use ONLY state[{i}] for this question. {}\n{}\n{}", r.question_text, r.prompt.system, r.prompt.user),
                "criteria":if noul {json!({"true":"A human must inspect the evidence, including any uncertainty.","false":"The encoded evidence settles this without a human."})}else{Value::Object(criteria)}}))
        }).collect();
        let mut attempts = Vec::new();
        let mut last = "no model to ask".to_owned();
        let started = Instant::now();
        for model in std::iter::once(&first.spec.model).chain(&first.spec.fallback) {
            let body = json!({"model":model, "state":reqs.iter().map(|r| r.state).collect::<Vec<_>>(), "questions":questions});
            let mut att = Attempt {
                model: model.clone(),
                tries: 0,
                statuses: Vec::new(),
                error: None,
                ok: false,
            };
            let mut backoff = self.retry.base_ms;
            loop {
                att.tries += 1;
                let parsed = self
                    .post(
                        "https://api.typesafe.ai/v1/systemone",
                        &[("Authorization", &format!("Bearer {}", key.expose()))],
                        &body,
                        Some(&key),
                    )
                    .and_then(|v| {
                        reqs.iter()
                            .enumerate()
                            .map(|(i, r)| {
                                let a = &v["answers"][format!("q{i}")];
                                if r.question == "needs_eyes" {
                                    let p = a["noul"]
                                        .as_f64()
                                        .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
                                        .ok_or_else(|| {
                                            CallError::fatal(
                                                "invalid Jev needs_eyes noul probability",
                                            )
                                        })?;
                                    let answer = if p >= 0.5 { "yes" } else { "no" };
                                    return Ok(Raw {
                                        answer: answer.into(),
                                        prob: Some(if p >= 0.5 { p } else { 1.0 - p }),
                                        confidence: None,
                                        probs: BTreeMap::from([
                                            ("yes".into(), p),
                                            ("no".into(), 1.0 - p),
                                        ]),
                                        prob_source: "model_distribution",
                                        model_version: v["model"].as_str().unwrap_or(model).into(),
                                        model: model.clone(),
                                        latency_ms: started.elapsed().as_millis() as u64,
                                        usage: v["usage"].clone(),
                                    });
                                }
                                let choice = a["choice"]
                                    .as_str()
                                    .ok_or_else(|| CallError::fatal("missing Jev batch choice"))?;
                                let probs: BTreeMap<String, f64> = serde_json::from_value(
                                    a["probabilities"].clone(),
                                )
                                .map_err(|_| CallError::fatal("invalid Jev batch probabilities"))?;
                                if !r.wire_answers.iter().any(|w| w == choice)
                                    || probs.len() != r.wire_answers.len()
                                    || probs.iter().any(|(k, p)| {
                                        !r.wire_answers.contains(k)
                                            || !p.is_finite()
                                            || !(0.0..=1.0).contains(p)
                                    })
                                    || (probs.values().sum::<f64>() - 1.0).abs() > 0.02
                                {
                                    return Err(CallError::fatal(
                                        "invalid Jev batch answer/distribution",
                                    ));
                                }
                                Ok(Raw {
                                    answer: choice.into(),
                                    prob: probs.get(choice).copied(),
                                    confidence: a["confidence"].as_f64(),
                                    probs,
                                    prob_source: "model_distribution",
                                    model_version: v["model"].as_str().unwrap_or(model).into(),
                                    model: model.clone(),
                                    latency_ms: started.elapsed().as_millis() as u64,
                                    usage: v["usage"].clone(),
                                })
                            })
                            .collect::<Result<Vec<_>, CallError>>()
                    });
                match parsed {
                    Ok(raws) => {
                        att.ok = true;
                        attempts.push(att);
                        return raws
                            .into_iter()
                            .map(|raw| CallOutcome {
                                result: Ok(raw),
                                attempts: attempts.clone(),
                            })
                            .collect();
                    }
                    Err(e) => {
                        if e.message == "HTTP call budget reached" {
                            att.tries = att.tries.saturating_sub(1);
                        }
                        att.statuses.extend(e.status);
                        att.error = Some(e.message.clone());
                        last = e.message;
                        if !e.retryable || att.tries > self.retry.max_retries {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(
                            e.retry_after
                                .map_or(backoff, |s| s.saturating_mul(1000))
                                .min(30_000),
                        ));
                        backoff = backoff.saturating_mul(2);
                    }
                }
            }
            attempts.push(att);
        }
        reqs.iter()
            .map(|_| CallOutcome {
                result: Err(last.clone()),
                attempts: attempts.clone(),
            })
            .collect()
    }

    fn ask(&self, req: &AskRequest<'_>) -> CallOutcome {
        let key = match self.keys.for_spec(req.spec) {
            Ok(k) => k,
            Err(e) => {
                return CallOutcome {
                    result: Err(e),
                    attempts: Vec::new(),
                };
            }
        };
        let mut attempts = Vec::new();
        let chain = std::iter::once(&req.spec.model).chain(&req.spec.fallback);
        let mut last = String::from("no model to ask");
        for model in chain {
            if req.spec.provider == Provider::Gemini && self.cooling(model) {
                attempts.push(Attempt {
                    model: model.clone(),
                    tries: 0,
                    statuses: Vec::new(),
                    error: Some("model in persistent cooldown".into()),
                    ok: false,
                });
                last = "all remaining models are in cooldown".into();
                continue;
            }
            let mut att = Attempt {
                model: model.clone(),
                tries: 0,
                statuses: Vec::new(),
                error: None,
                ok: false,
            };
            let mut backoff = self.retry.base_ms;
            loop {
                att.tries += 1;
                match self.once(req, model, key.as_ref()) {
                    Ok(raw) => {
                        att.ok = true;
                        attempts.push(att);
                        return CallOutcome {
                            result: Ok(raw),
                            attempts,
                        };
                    }
                    Err(e) => {
                        if e.message == "HTTP call budget reached" {
                            att.tries = att.tries.saturating_sub(1);
                        }
                        att.statuses.extend(e.status);
                        att.error = Some(e.message.clone());
                        last.clone_from(&e.message);
                        if !e.retryable || att.tries > self.retry.max_retries {
                            break;
                        }
                        let wait = e
                            .retry_after
                            .map_or(backoff, |s| s.saturating_mul(1000))
                            .min(30_000);
                        std::thread::sleep(Duration::from_millis(wait));
                        backoff = backoff.saturating_mul(2);
                    }
                }
            }
            if req.spec.provider == Provider::Gemini
                && last != "HTTP call budget reached"
                && let Err(e) = self.mark_failed(model)
            {
                last.push_str(&format!("; {e}"));
            }
            attempts.push(att);
            if last == "HTTP call budget reached" {
                break;
            }
        }
        CallOutcome {
            result: Err(last),
            attempts,
        }
    }
}

#[cfg(test)]
mod review_chain_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::review::Profile;

    #[test]
    fn bounded_fallback_persists_and_skips_cooldown_without_network() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("gemini.env"),
            "SACCADE_GEMINI_API_KEY=offline-secret",
        )
        .unwrap();
        let spec = Profile::default().spec(Provider::Gemini).unwrap();
        let replies = || {
            Some(RefCell::new(vec![
                (503, json!({})),
                (503, json!({})),
                (
                    200,
                    json!({"candidates":[{"content":{"parts":[{"text":"{\"answer\":\"P2\",\"probability\":0.9}"}]}}]}),
                ),
            ]))
        };
        let mut backend = LiveBackend::new(
            Keys::new(Some(tmp.path().into())),
            Retry {
                max_retries: 1,
                base_ms: 0,
            },
            Duration::from_secs(1),
        )
        .with_policy(tmp.path().join("cache"), 600, 10, 0, 10);
        backend.mock_replies = replies();
        let answers = vec!["P1".into(), "P2".into(), "tie".into(), "unsure".into()];
        let prompt = Prompt {
            system: "blind".into(),
            user: "choose".into(),
        };
        let state = json!({});
        let req = AskRequest {
            spec: &spec,
            question: "preference",
            question_text: "choose",
            kind: "preference",
            wire_answers: &answers,
            state: &state,
            prompt: &prompt,
            images: &[],
        };
        let out = backend.ask(&req);
        assert_eq!(out.result.unwrap().model, "gemini-3.7-flash");
        assert_eq!(out.attempts[0].statuses, vec![503, 503]);
        assert_eq!(backend.counts(), [0, 3]);
        let mut later = LiveBackend::new(
            Keys::new(Some(tmp.path().into())),
            Retry {
                max_retries: 0,
                base_ms: 0,
            },
            Duration::from_secs(1),
        )
        .with_policy(tmp.path().join("cache"), 600, 1, 0, 1);
        later.mock_replies = Some(RefCell::new(vec![(
            200,
            json!({"candidates":[{"content":{"parts":[{"text":"{\"answer\":\"P2\",\"probability\":0.9}"}]}}]}),
        )]));
        let out = later.ask(&req);
        assert_eq!(out.attempts[0].tries, 0);
        assert_eq!(out.result.unwrap().model, "gemini-3.7-flash");
        assert_eq!(later.counts(), [0, 1]);
        assert!(later.ask(&req).result.is_err());
        assert_eq!(later.counts(), [0, 1]);
    }
}
