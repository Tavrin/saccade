//! The run overview of `saccade serve`: `GET /runs` (the page) and
//! `GET /api/runs` (the `saccade-runs.v1` JSON, measured in a background
//! thread and cached on disk next to the sessions).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::State;
use super::browse::{PathError, rel_of, resolve_under};
use crate::runs::{
    AssetUrls, MAX_RUNS, PairResult, Pairing, Plan, RunInput, RunsOptions, assemble, compute_all,
    plan, unique_labels,
};

/// Most finished overviews kept in memory.
const KEEP_JOBS: usize = 16;

/// A running or finished overview.
pub(crate) struct Job {
    plan: Plan,
    results: Mutex<Vec<Option<PairResult>>>,
}

/// Why an overview request was refused: HTTP status and message.
pub(crate) type Refusal = (u16, String);

/// `%`-encodes every byte that is not unreserved.
pub(crate) fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

struct ServerAssets<'a> {
    state: &'a State,
}

impl AssetUrls for ServerAssets<'_> {
    fn image(&self, path: &Path, edge: u32) -> Option<String> {
        let rel = rel_of(self.state, path);
        (!rel.is_empty()).then(|| format!("/thumb?path={}&w={edge}", pct_encode(&rel)))
    }

    fn heat(&self, key: &str) -> Option<String> {
        Some(format!("/runs/heat?key={key}"))
    }
}

fn path_refusal(what: &str, e: &PathError) -> Refusal {
    match e {
        PathError::Invalid => (400, format!("{what}: invalid path.")),
        PathError::Escapes => (403, format!("{what}: escapes the archive root.")),
        PathError::Missing => (
            404,
            format!("{what}: no such run in this archive. It may have been moved or deleted."),
        ),
    }
}

/// The reference and the runs of an overview request (`ref=`, `runs=` and
/// `pair<i>=` where `i` counts the reference as 0), resolved under the roots.
pub(crate) fn parse_request(
    state: &State,
    q: &HashMap<String, String>,
) -> Result<(RunInput, Vec<RunInput>), Refusal> {
    let get = |k: &str| q.get(k).map_or("", String::as_str);
    let ref_rel = get("ref").to_owned();
    let run_rels: Vec<String> = get("runs")
        .split(',')
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect();
    if ref_rel.is_empty() || run_rels.is_empty() || run_rels.len() > MAX_RUNS {
        return Err((
            400,
            format!(
                "ref= names the reference run and runs= takes 1 to {MAX_RUNS} runs to compare against it."
            ),
        ));
    }
    let mut dirs = Vec::new();
    for rel in std::iter::once(&ref_rel).chain(&run_rels) {
        let abs = resolve_under(state, rel).map_err(|e| path_refusal(rel, &e))?;
        if !abs.is_dir() {
            return Err((400, format!("{rel}: not a directory.")));
        }
        dirs.push(abs);
    }
    let labels = unique_labels(&dirs);
    let mut inputs = Vec::new();
    for (i, dir) in dirs.into_iter().enumerate() {
        let pairing = if i == 0 {
            Pairing::Name
        } else {
            Pairing::parse(get(&format!("pair{i}"))).map_err(|e| (400, e))?
        };
        inputs.push(RunInput {
            display: rel_of(state, &dir),
            label: labels[i].clone(),
            dir,
            pairing,
        });
    }
    let reference = inputs.remove(0);
    Ok((reference, inputs))
}

fn options(state: &State) -> RunsOptions {
    RunsOptions {
        pixels_per_degree: state.view.pixels_per_degree,
        hdr: state.view.hdr,
        meta: state.view.meta.clone(),
    }
}

/// The overview for a request: starts the measuring thread when this exact
/// comparison is not already measured or running, and returns the model as
/// measured so far.
pub(crate) fn model_json(
    state: &Arc<State>,
    q: &HashMap<String, String>,
) -> Result<serde_json::Value, Refusal> {
    let (reference, runs) = parse_request(state, q)?;
    let plan = plan(&reference, &runs, &options(state)).map_err(|e| (422, e.to_string()))?;
    let key = plan.fingerprint();
    let job = {
        let mut jobs = state
            .overviews
            .lock()
            .map_err(|_| (500, "overview state is poisoned".to_owned()))?;
        if let Some(j) = jobs.get(&key) {
            j.clone()
        } else {
            if jobs.len() >= KEEP_JOBS {
                jobs.clear();
            }
            let total = plan.tasks().len();
            let job = Arc::new(Job {
                plan,
                results: Mutex::new(vec![None; total]),
            });
            jobs.insert(key, job.clone());
            let (worker, cache) = (job.clone(), state.cache.join("runs"));
            std::thread::spawn(move || {
                compute_all(&worker.plan, Some(&cache), &|i, r| {
                    if let Ok(mut v) = worker.results.lock() {
                        v[i] = Some(r);
                    }
                });
            });
            job
        }
    };
    let snapshot = job.results.lock().map(|v| v.clone()).unwrap_or_default();
    let model = assemble(&job.plan, &snapshot, &ServerAssets { state });
    serde_json::to_value(&model).map_err(|e| (500, e.to_string()))
}

/// The page shell of an overview request; the page script polls `/api/runs`.
pub(crate) fn page_html(state: &State, q: &HashMap<String, String>, token: &str) -> String {
    let mut keys: Vec<&String> = q
        .keys()
        .filter(|k| *k == "ref" || *k == "runs" || k.starts_with("pair"))
        .collect();
    keys.sort();
    let query = keys
        .iter()
        .map(|k| format!("{}={}", pct_encode(k), pct_encode(&q[*k])))
        .collect::<Vec<_>>()
        .join("&");
    let config = serde_json::json!({
        "mode": "serve",
        "token": token,
        "query": query,
        "api": format!("/api/runs?{query}"),
        "multi_root": state.multi(),
    });
    crate::runs::render_page(&config.to_string())
}
