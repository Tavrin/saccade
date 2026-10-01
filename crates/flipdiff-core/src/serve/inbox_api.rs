//! Inbox routes reached only after the shared Host/Origin/token gate.
use serde::Deserialize;
use tiny_http::Request;

use super::{Resp, State, read_body};
use crate::inbox;

fn error(e: crate::Error) -> Resp {
    let code = match &e {
        crate::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => 404,
        crate::Error::Config(_) | crate::Error::Json(_) => 400,
        _ => 500,
    };
    Resp::error(code, &e.to_string())
}

pub(super) fn get(state: &State, path: &str) -> Option<Resp> {
    if path == "/inbox" || path == "/inbox/" {
        return Some(Resp::html(
            include_str!("../../assets/inbox.html")
                .replace(
                    "/*__INBOX_CSS__*/",
                    &crate::render::shared::page_css(&[include_str!("../../assets/inbox.css")]),
                )
                .replace("/*__INBOX_JS__*/", include_str!("../../assets/inbox.js"))
                .replace(
                    "__INBOX_TOKEN__",
                    &serde_json::to_string(&state.token).unwrap_or_default(),
                ),
        ));
    }
    let dir = state.decisions.join("inbox");
    if path == "/api/inbox" {
        return Some(match inbox::list(&dir) {
            Ok(items) => Resp::json(&items),
            Err(e) => error(e),
        });
    }
    path.strip_prefix("/api/inbox/")
        .map(|id| match inbox::get(&dir, id) {
            Ok(item) => Resp::json(&item),
            Err(e) => error(e),
        })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    answer: String,
    note: Option<String>,
}

pub(super) fn post(state: &State, req: &mut Request, path: &str) -> Resp {
    let body = match read_body(req, 128 * 1024) {
        Ok(body) => body,
        Err(e) => return e,
    };
    let Ok(_lock) = state.inbox_lock.lock() else {
        return Resp::error(500, "inbox lock unavailable");
    };
    let dir = state.decisions.join("inbox");
    if path == "/api/inbox" {
        let mut question: inbox::Question = match serde_json::from_slice(&body) {
            Ok(q) => q,
            Err(e) => return Resp::error(400, &e.to_string()),
        };
        // Accept an absolute deep link only to this exact local server.
        if let Some(link) = &mut question.link {
            for host in ["127.0.0.1", "localhost"] {
                let origin = format!("http://{host}:{}", state.port);
                if let Some(local) = link.strip_prefix(&origin).filter(|p| p.starts_with('/')) {
                    *link = local.to_owned();
                    break;
                }
            }
        }
        return match inbox::post(&dir, question) {
            Ok(item) => Resp::json(&serde_json::json!({"id": item.id})),
            Err(e) => error(e),
        };
    }
    if let Some(id) = path
        .strip_prefix("/api/inbox/")
        .and_then(|p| p.strip_suffix("/answer"))
    {
        let answer: Answer = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return Resp::error(400, &e.to_string()),
        };
        return match inbox::answer(&dir, id, answer.answer, answer.note) {
            Ok(item) => Resp::json(&item),
            Err(e) => error(e),
        };
    }
    Resp::error(404, "unknown inbox route")
}
