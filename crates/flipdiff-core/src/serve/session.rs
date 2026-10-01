//! View sessions: `flipdiff view` output cached by content, built in the
//! background, plus the decisions files and uploads that belong to them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::State;
use super::browse::{hash128, is_image_name};
use crate::view::{Decisions, MAX_SEED, ViewOptions, build_view};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A process-unique suffix for temporary names.
pub(crate) fn unique() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Build state of one session.
pub(crate) enum SessionState {
    Building { started: Instant, hint_sets: usize },
    Ready,
    Failed(String),
}

/// What a session compares.
pub(crate) struct Spec {
    /// Directories to pair (2 to 6), the first is the reference.
    pub dirs: Vec<PathBuf>,
    /// Labels, one per directory; `None` derives them from the directory names.
    pub labels: Option<Vec<String>>,
    /// Display names of the inputs (archive-relative paths), kept in `session.json`.
    pub runs: Vec<String>,
    /// Blind judging.
    pub blind: bool,
}

/// `session.json`, written next to the view in the cache.
#[derive(Serialize, Deserialize)]
pub(crate) struct SessionInfo {
    pub runs: Vec<String>,
    pub labels: Vec<String>,
    pub blind: bool,
    pub created_unix: u64,
}

/// True for a 32-character lower-case hex session id.
pub(crate) fn is_session_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn session_dir(state: &State, id: &str) -> PathBuf {
    state.cache.join("sessions").join(id)
}

/// Whether the session's view is fully built on disk.
pub(crate) fn is_built(state: &State, id: &str) -> bool {
    session_dir(state, id).join("index.html").is_file()
}

/// Everything about a directory that changes the comparison: names, sizes and
/// modification times of its images and sidecars.
fn fingerprint(dir: &Path, meta_name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut files: Vec<(String, u64, u64)> = Vec::new();
    for e in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.file_type().is_file() || !(is_image_name(&name) || name.ends_with(meta_name)) {
            continue;
        }
        let (len, mtime) = e.metadata().map_or((0, 0), |m| {
            (
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos() as u64),
            )
        });
        let rel = e
            .path()
            .strip_prefix(dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(name);
        files.push((rel, len, mtime));
    }
    files.sort();
    out.extend_from_slice(dir.to_string_lossy().as_bytes());
    for (rel, len, mtime) in files {
        out.extend_from_slice(format!("|{rel}:{len}:{mtime}").as_bytes());
    }
    out
}

/// The id of the session `spec` produces: a digest of the directories'
/// contents fingerprint and every option that changes the output.
pub(crate) fn session_id(state: &State, spec: &Spec) -> String {
    let mut parts: Vec<Vec<u8>> = spec
        .dirs
        .iter()
        .map(|d| fingerprint(d, &state.view.meta.name))
        .collect();
    let template = ViewOptions {
        labels: None,
        ..state.view.clone()
    };
    parts.push(format!("{template:?}").into_bytes());
    parts.push(
        format!(
            "{:?}|{}|{}",
            spec.labels,
            spec.blind,
            env!("CARGO_PKG_VERSION")
        )
        .into_bytes(),
    );
    let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
    hash128(&refs)
}

/// Starts building the session unless it is built or already building.
pub(crate) fn ensure(state: &std::sync::Arc<State>, spec: Spec) -> String {
    let id = session_id(state, &spec);
    if is_built(state, &id) {
        if let Ok(mut map) = state.sessions.lock() {
            map.insert(id.clone(), SessionState::Ready);
        }
        return id;
    }
    {
        let Ok(mut map) = state.sessions.lock() else {
            return id;
        };
        if matches!(map.get(&id), Some(SessionState::Building { .. })) {
            return id;
        }
        let hint = walkdir::WalkDir::new(&spec.dirs[0])
            .follow_links(false)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file() && is_image_name(&e.file_name().to_string_lossy()))
            .count();
        map.insert(
            id.clone(),
            SessionState::Building {
                started: Instant::now(),
                hint_sets: hint,
            },
        );
    }
    let (state, build_id) = (state.clone(), id.clone());
    std::thread::spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            build(&state, &build_id, &spec)
        }))
        .unwrap_or_else(|_| Err("internal error while building the session".into()));
        if let Ok(mut map) = state.sessions.lock() {
            map.insert(
                build_id,
                match outcome {
                    Ok(()) => SessionState::Ready,
                    Err(e) => SessionState::Failed(e),
                },
            );
        }
    });
    id
}

fn build(state: &State, id: &str, spec: &Spec) -> Result<(), String> {
    let sessions = state.cache.join("sessions");
    let tmp = sessions.join(format!(".tmp-{id}-{}", unique()));
    let mut opts = state.view.clone();
    opts.labels.clone_from(&spec.labels);
    opts.blind = spec.blind;
    // A deterministic seed keeps a blind session's order stable across reuse.
    opts.seed = Some(u64::from_str_radix(&id[..13], 16).map_or(1, |s| s & MAX_SEED));
    let result = build_view(&spec.dirs, &tmp, &opts).map_err(|e| e.to_string());
    let model = match result {
        Ok(m) => m,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let info = SessionInfo {
        runs: spec.runs.clone(),
        labels: model.labels.clone(),
        blind: spec.blind,
        created_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    let write = serde_json::to_string(&info)
        .map_err(|e| e.to_string())
        .and_then(|j| std::fs::write(tmp.join("session.json"), j).map_err(|e| e.to_string()));
    let dest = sessions.join(id);
    let finish = write.and_then(|()| {
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
    });
    if finish.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    finish
}

/// Status of a session for the progress page.
#[derive(Serialize)]
pub(crate) struct Status {
    state: &'static str,
    elapsed_s: Option<u64>,
    /// Image count of the reference directory (an estimate of the sets).
    estimated_sets: Option<usize>,
    error: Option<String>,
    runs: Vec<String>,
    url: Option<String>,
}

pub(crate) fn status(state: &State, id: &str) -> Option<Status> {
    let info: Option<SessionInfo> =
        std::fs::read_to_string(session_dir(state, id).join("session.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
    let runs = info.map(|i| i.runs).unwrap_or_default();
    let map = state.sessions.lock().ok()?;
    let ready_url = Some(format!("/session/{id}/"));
    Some(match map.get(id) {
        Some(SessionState::Building { started, hint_sets }) => Status {
            state: "building",
            elapsed_s: Some(started.elapsed().as_secs()),
            estimated_sets: Some(*hint_sets),
            error: None,
            runs,
            url: None,
        },
        Some(SessionState::Failed(e)) => Status {
            state: "failed",
            elapsed_s: None,
            estimated_sets: None,
            error: Some(e.clone()),
            runs,
            url: None,
        },
        Some(SessionState::Ready) => Status {
            state: "ready",
            elapsed_s: None,
            estimated_sets: None,
            error: None,
            runs,
            url: ready_url,
        },
        None if is_built(state, id) => Status {
            state: "ready",
            elapsed_s: None,
            estimated_sets: None,
            error: None,
            runs,
            url: ready_url,
        },
        None => return None,
    })
}

/// File name of a session's decisions.
pub(crate) fn decisions_path(state: &State, id: &str) -> PathBuf {
    state
        .decisions
        .join(format!("{id}.flipdiff-decisions.v1.json"))
}

/// Writes a session's decisions atomically into the decisions directory.
pub(crate) fn save_decisions(state: &State, id: &str, d: &Decisions) -> Result<(), String> {
    let text = serde_json::to_string_pretty(d).map_err(|e| e.to_string())?;
    let dest = decisions_path(state, id);
    let tmp = state.decisions.join(format!(".{id}.{}.tmp", unique()));
    std::fs::write(&tmp, format!("{text}\n")).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

/// One saved decisions file, for the landing page.
#[derive(Serialize)]
pub(crate) struct DecisionSummary {
    file: String,
    session: String,
    mtime: u64,
    decided: usize,
    sets: usize,
    labels: Vec<String>,
    runs: Vec<String>,
    session_url: Option<String>,
}

/// The most recently written decisions files.
pub(crate) fn recent_decisions(state: &State, limit: usize) -> Vec<DecisionSummary> {
    let mut files: Vec<(SystemTime, PathBuf, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&state.decisions) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".flipdiff-decisions.v1.json") else {
                continue;
            };
            if !is_session_id(id) {
                continue;
            }
            let t = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            files.push((t, e.path(), id.to_owned()));
        }
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files
        .into_iter()
        .take(limit)
        .filter_map(|(t, path, id)| {
            let d = crate::view::read_decisions(&path).ok()?;
            let info: Option<SessionInfo> =
                std::fs::read_to_string(session_dir(state, &id).join("session.json"))
                    .ok()
                    .and_then(|j| serde_json::from_str(&j).ok());
            Some(DecisionSummary {
                file: path.file_name()?.to_string_lossy().into_owned(),
                mtime: t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
                decided: d
                    .sets
                    .iter()
                    .filter(|s| s.decision.is_some() || s.chosen_label.is_some() || s.no_difference)
                    .count(),
                sets: d.sets.len(),
                labels: d.labels,
                runs: info.map(|i| i.runs).unwrap_or_default(),
                session_url: is_built(state, &id).then(|| format!("/session/{id}/")),
                session: id,
            })
        })
        .collect()
}

/// Copies or re-encodes two images to `<dest>/a/image.<ext>` and
/// `<dest>/b/image.<ext>` so a view pairs them whatever their names. Equal
/// extensions are copied byte for byte; differing ones are decoded and written
/// as PNG (HDR formats must match).
pub(crate) fn stage_pair(a: &Path, b: &Path, dest: &Path) -> Result<(PathBuf, PathBuf), String> {
    let ext = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    let (ea, eb) = (ext(a), ext(b));
    let (da, db) = (dest.join("a"), dest.join("b"));
    for d in [&da, &db] {
        std::fs::create_dir_all(d).map_err(|e| format!("creating {}: {e}", d.display()))?;
    }
    if ea == eb {
        std::fs::copy(a, da.join(format!("image.{ea}"))).map_err(|e| e.to_string())?;
        std::fs::copy(b, db.join(format!("image.{eb}"))).map_err(|e| e.to_string())?;
    } else {
        if crate::hdr::is_hdr_path(a) || crate::hdr::is_hdr_path(b) {
            return Err("an HDR image can only be paired with an image of the same format".into());
        }
        for (src, d) in [(a, &da), (b, &db)] {
            let img = crate::run::decode(src).map_err(|e| e.to_string())?;
            img.save_with_format(d.join("image.png"), image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok((da, db))
}
