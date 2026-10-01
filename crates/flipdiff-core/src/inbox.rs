//! Persistent, local ask-a-human questions. Answers never update baselines.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// An agent's question. The allowed answers form a closed set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Human-readable question.
    pub question: String,
    /// Unique nonempty answers.
    pub allowed_answers: Vec<String>,
    /// Optional supporting text.
    pub context: Option<String>,
    /// Optional local deep link, including its URL hash.
    pub link: Option<String>,
    /// Agent/source name.
    pub from: Option<String>,
}

/// Persisted `flipdiff-inbox-item.v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Item {
    /// Schema identifier.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "flipdiff-inbox-item.v1")))]
    pub schema: String,
    /// Opaque hex id.
    #[cfg_attr(feature = "schema", schemars(regex(pattern = "^[0-9a-f]{32}$")))]
    pub id: String,
    /// Question text.
    pub question: String,
    /// Allowed human answers.
    pub allowed_answers: Vec<String>,
    /// Supporting context.
    pub context: Option<String>,
    /// Local deep link.
    pub link: Option<String>,
    /// Asking agent.
    pub from: Option<String>,
    /// open or answered.
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["open", "answered"])))]
    pub status: String,
    /// Creation time.
    pub created_unix: u64,
    /// Human's answer, absent while open.
    pub answer: Option<String>,
    /// Human's note.
    pub note: Option<String>,
    /// Answer time.
    pub answered_unix: Option<u64>,
}

/// Result of posting or waiting for an answer (`flipdiff-ask-result.v1`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AskResult {
    /// Schema identifier.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "flipdiff-ask-result.v1")))]
    pub schema: String,
    /// Inbox item id.
    #[cfg_attr(feature = "schema", schemars(regex(pattern = "^[0-9a-f]{32}$")))]
    pub id: String,
    /// open or answered.
    #[cfg_attr(feature = "schema", schemars(extend("enum" = ["open", "answered"])))]
    pub status: String,
    /// Answer, when available.
    pub answer: Option<String>,
    /// Human note, when available.
    pub note: Option<String>,
    /// True when a wait elapsed without an answer.
    pub timed_out: bool,
    /// URL for the human's browser.
    pub url: String,
}

/// Per-process discovery file, private to the OS user.
#[derive(Serialize, Deserialize)]
pub struct ServeInfo {
    /// Loopback port.
    pub port: u16,
    /// POST token.
    pub token: String,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Validate before persisting. Links must remain on this server.
pub fn validate(q: &Question) -> Result<()> {
    if q.question.trim().is_empty()
        || q.question.len() > 16_384
        || q.allowed_answers.is_empty()
        || q.allowed_answers.len() > 32
    {
        return Err(Error::Config(
            "question needs text and 1 to 32 allowed answers".into(),
        ));
    }
    for (i, answer) in q.allowed_answers.iter().enumerate() {
        if answer.trim().is_empty() || answer.len() > 256 || q.allowed_answers[..i].contains(answer)
        {
            return Err(Error::Config(
                "allowed answers must be unique, nonempty, at most 256 bytes".into(),
            ));
        }
    }
    if q.context.as_ref().is_some_and(|s| s.len() > 65_536)
        || q.from.as_ref().is_some_and(|s| s.len() > 256)
    {
        return Err(Error::Config("context or source too long".into()));
    }
    if q.link.as_ref().is_some_and(|s| {
        s.len() > 8192
            || !s.starts_with('/')
            || s.starts_with("//")
            || s.contains('\\')
            || s.chars().any(char::is_control)
    }) {
        return Err(Error::Config(
            "link must be a local absolute-path deep link (for example /compare?runs=a,b#entry=x)"
                .into(),
        ));
    }
    Ok(())
}

fn path(dir: &Path, id: &str) -> Result<PathBuf> {
    if id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(Error::Config("invalid inbox id".into()));
    }
    let path = dir.join(format!("{id}.json"));
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Config("inbox item is a symlink".into()));
    }
    Ok(path)
}

fn save(dir: &Path, item: &Item) -> Result<()> {
    let dest = path(dir, &item.id)?;
    let tmp = dir.join(format!(".{}.tmp", crate::serve::random_token()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(io("creating inbox temporary file"))?;
    use std::io::Write;
    let result = file
        .write_all(&serde_json::to_vec_pretty(item)?)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&tmp, dest));
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result.map_err(io("persisting inbox item"))
}

/// Create an open item in the dedicated decisions/inbox directory.
pub fn post(dir: &Path, q: Question) -> Result<Item> {
    validate(&q)?;
    let item = Item {
        schema: "flipdiff-inbox-item.v1".into(),
        id: crate::serve::random_token(),
        question: q.question,
        allowed_answers: q.allowed_answers,
        context: q.context,
        link: q.link,
        from: q.from,
        status: "open".into(),
        created_unix: now(),
        answer: None,
        note: None,
        answered_unix: None,
    };
    save(dir, &item)?;
    Ok(item)
}

/// Read an item, refusing symlink files and unbounded bodies.
pub fn get(dir: &Path, id: &str) -> Result<Item> {
    let path = path(dir, id)?;
    let meta = std::fs::metadata(&path).map_err(io("reading inbox item"))?;
    if meta.len() > 1024 * 1024 {
        return Err(Error::Config("inbox item too large".into()));
    }
    let item: Item =
        serde_json::from_slice(&std::fs::read(&path).map_err(io("reading inbox item"))?)?;
    if item.id != id || item.schema != "flipdiff-inbox-item.v1" {
        return Err(Error::Config("invalid inbox item".into()));
    }
    Ok(item)
}

/// List open items first, then newest first within each group.
pub fn list(dir: &Path) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io("listing inbox"))? {
        let entry = entry.map_err(io("listing inbox entry"))?;
        if let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.strip_suffix(".json"))
        {
            items.push(get(dir, id)?);
        }
    }
    items.sort_by_key(|i| {
        (
            i.status != "open",
            std::cmp::Reverse(i.created_unix),
            i.id.clone(),
        )
    });
    Ok(items)
}

/// Record a valid human answer atomically. Caller serializes concurrent answers.
pub fn answer(dir: &Path, id: &str, answer: String, note: Option<String>) -> Result<Item> {
    let mut item = get(dir, id)?;
    if !item.allowed_answers.contains(&answer) || note.as_ref().is_some_and(|s| s.len() > 65_536) {
        return Err(Error::Config(
            "answer must be allowed; note at most 65536 bytes".into(),
        ));
    }
    item.status = "answered".into();
    item.answer = Some(answer);
    item.note = note;
    item.answered_unix = Some(now());
    save(dir, &item)?;
    Ok(item)
}

/// Write discovery atomically, with mode 0600 from creation on Unix.
pub fn write_discovery(cache: &Path, port: u16, token: String) -> Result<()> {
    let tmp = cache.join(format!(".serve-{}.tmp", crate::serve::random_token()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(io("creating private serve discovery"))?;
    use std::io::Write;
    let result = file
        .write_all(&serde_json::to_vec(&ServeInfo { port, token })?)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&tmp, cache.join("serve.json")));
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result.map_err(io("writing private serve discovery"))
}

fn io(context: &str) -> impl FnOnce(std::io::Error) -> Error {
    crate::run::io_err(context.into())
}
