//! Routes of the human vote page (`/vote/<run id>`), reached only after the
//! shared Host, Origin and token gate. A run is a directory under
//! `<decisions dir>/judge/`; nothing outside it is ever read.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use tiny_http::Request;

use super::{Resp, State, read_body};
use crate::judge_vote as vote;

/// Largest vote body accepted.
const MAX_VOTE_BYTES: u64 = 4 * 1024;

fn run_dir(state: &State, id: &str) -> Option<PathBuf> {
    if !vote::is_run_id(id) {
        return None;
    }
    let dir = vote::run_dir(&state.decisions, id);
    if crate::paths::canonicalize(&dir).ok()? != dir
        || crate::paths::canonicalize(dir.join("run.json")).ok()? != dir.join("run.json")
    {
        return None;
    }
    dir.join("run.json").is_file().then_some(dir)
}

pub(super) fn get(state: &State, path: &str, q: &HashMap<String, String>) -> Option<Resp> {
    if let Some(rest) = path.strip_prefix("/api/vote/") {
        let id = rest.strip_suffix("/items")?;
        let Some(dir) = run_dir(state, id) else {
            return Some(Resp::error(404, "unknown vote run"));
        };
        let Some(voter) = q.get("voter").and_then(|v| vote::clean_voter(v)) else {
            return Some(Resp::error(400, "a valid voter name is required"));
        };
        return Some(match vote::items_json(&dir, id, &voter) {
            Ok(v) => Resp::json(&v),
            Err(e) => Resp::error(404, &e),
        });
    }
    let rest = path.strip_prefix("/vote/")?;
    let (id, tail) = rest.split_once('/').unwrap_or((rest, ""));
    let Some(dir) = run_dir(state, id) else {
        return Some(Resp::error_page(
            404,
            "Unknown vote run",
            "This vote link is not valid, or the run was never written.",
        ));
    };
    Some(match tail {
        "" => Resp::html(vote::page(&state.token, id)),
        t => match t
            .strip_prefix("strip/")
            .and_then(|f| vote::strip_bytes(&dir, f))
        {
            Some(bytes) => Resp::bytes(200, "image/png", bytes),
            None => Resp::text(404, "not found"),
        },
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vote {
    voter: String,
    item: String,
    answer: String,
}

pub(super) fn post(state: &State, req: &mut Request, path: &str) -> Resp {
    let Some(id) = path
        .strip_prefix("/api/vote/")
        .and_then(|r| r.strip_suffix("/vote"))
    else {
        return Resp::error(404, "unknown vote route");
    };
    let Some(dir) = run_dir(state, id) else {
        return Resp::error(404, "unknown vote run");
    };
    let body = match read_body(req, MAX_VOTE_BYTES) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let v: Vote = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return Resp::error(400, &format!("not a vote: {e}")),
    };
    match vote::record_vote(&dir, &v.voter, &v.item, &v.answer) {
        Ok(p) => Resp::json(&p),
        Err(e) => Resp::error(400, &e),
    }
}
