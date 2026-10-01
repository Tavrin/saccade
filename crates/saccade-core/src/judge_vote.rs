//! Human votes for judge mode: the vote run that `saccade judge` writes, the
//! store the vote page appends to, and the glue that turns votes back into
//! judgements.
//!
//! A vote run lives in `<decisions dir>/judge/<run id>/`: `run.json` (the
//! items), `strips/*.png` (blind crops, labelled 1 and 2) and `votes.jsonl`
//! (one line per vote, latest vote per voter and item wins). The run id is a
//! hash of the inputs, so running the same command again finds the votes cast
//! in between. `saccade serve` serves the page at `/vote/<run id>`.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::judge::{
    JudgeItem, JudgeQuestion, JudgeSpec, Judgement, Plan, now_ms, sha_hex, stable_for, vote_strips,
};
use crate::judge_stats::is_abstain;

/// Schema identifier of the vote-run file.
pub const VOTES_SCHEMA: &str = "saccade-judge-votes.v1";
/// Schema of vote API responses (items or progress).
pub const VOTE_API_SCHEMA: &str = "saccade-judge-vote-api.v1";

const PAGE: &str = include_str!("../assets/vote.html");
const CSS: &str = include_str!("../assets/vote.css");
const JS: &str = include_str!("../assets/vote.js");

/// Serialises appends to `votes.jsonl` across server threads.
static VOTE_LOCK: Mutex<()> = Mutex::new(());

/// One answer button on the vote page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Choice {
    /// The answer recorded.
    pub answer: String,
    /// The button text.
    pub label: String,
    /// The keyboard key.
    pub key: String,
}

/// One thing to vote on: an item in one presentation order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VoteItem {
    /// Opaque presentation id; candidate names and order stay server-side.
    pub id: String,
    /// The judge item.
    pub item: String,
    /// `ab` or `ba`.
    pub order: String,
    /// The question type.
    pub question: String,
    /// The question kind.
    pub kind: String,
    /// What the kind means for this vote.
    pub kind_limits: String,
    /// The fixed question wording.
    pub text: String,
    /// Extra context (the intent, which image is the reference).
    pub context: String,
    /// The buttons.
    pub choices: Vec<Choice>,
    /// The strip image file inside `strips/`; empty when there is no image.
    pub strip: String,
    /// Whether this is a pairwise preference (arrow keys apply).
    pub pairwise: bool,
    /// SHA-256 of the question, context, choices and exact strip bytes shown.
    pub evidence_hash: String,
}

/// The `run.json` of a vote run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VoteRun {
    /// Always [`VOTES_SCHEMA`].
    pub schema: String,
    /// The run id.
    pub run_id: String,
    /// Creation time.
    pub created_at_unix: u64,
    /// The items, one per item and order.
    pub items: Vec<VoteItem>,
}

/// A stored vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRec {
    /// The voter's name.
    pub voter: String,
    /// The vote item id.
    pub item: String,
    /// The answer.
    pub answer: String,
    /// Unix milliseconds.
    pub ts: u64,
}

/// Whether `id` can be a run id (16 lowercase hex digits).
pub fn is_run_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The directory of a run.
pub fn run_dir(decisions: &Path, run_id: &str) -> PathBuf {
    decisions.join("judge").join(run_id)
}

/// A valid voter name: 1 to 40 characters of letters, digits, space, `_`, `-`, `.`.
pub fn clean_voter(name: &str) -> Option<String> {
    let n = name.trim();
    (!n.is_empty()
        && n.chars().count() <= 40
        && n.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.')))
    .then(|| n.to_owned())
}

fn choices_for(item: &JudgeItem) -> Vec<Choice> {
    if item.question == JudgeQuestion::Preference {
        return vec![
            Choice {
                answer: "P1".into(),
                label: "Image 1".into(),
                key: "1".into(),
            },
            Choice {
                answer: "P2".into(),
                label: "Image 2".into(),
                key: "2".into(),
            },
            Choice {
                answer: "unsure".into(),
                label: "Unsure".into(),
                key: "u".into(),
            },
        ];
    }
    let mut out = Vec::new();
    let mut n = 0;
    for a in item.question.wire_answers() {
        if is_abstain(&a) {
            out.push(Choice {
                label: "Unsure".into(),
                key: "u".into(),
                answer: a,
            });
        } else {
            n += 1;
            out.push(Choice {
                label: a.replace('_', " "),
                key: n.to_string(),
                answer: a.clone(),
            });
        }
    }
    out.sort_by_key(|c| c.key == "u");
    out
}

fn context_for(item: &JudgeItem, order: &str) -> String {
    if item.question == JudgeQuestion::Preference {
        return format!(
            "Criterion: {}",
            item.intent.as_deref().unwrap_or("overall visual quality")
        );
    }
    let (r, c) = if order == "ab" {
        ("1", "2")
    } else {
        ("2", "1")
    };
    let mut s = format!("Image {r} is the approved reference; image {c} is the new capture.");
    if let Some(i) = &item.intent {
        s.push_str(&format!(" Intent: {i}"));
    }
    s
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replacing {}: {e}", path.display()))
}

/// Writes the vote run (items and strips) for the real items of `plan`, once.
fn ensure_run(dir: &Path, run_id: &str, plan: &Plan) -> Result<VoteRun, String> {
    let file = dir.join("run.json");
    if let Ok(text) = std::fs::read_to_string(&file)
        && let Ok(run) = serde_json::from_str::<VoteRun>(&text)
        && run.schema == VOTES_SCHEMA
        && run.run_id == run_id
    {
        return Ok(run);
    }
    std::fs::create_dir_all(dir.join("strips"))
        .map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let mut items = Vec::new();
    for item in plan.items.iter().filter(|i| i.canary.is_none()) {
        let kind = item.question.kind();
        for order in ["ab", "ba"] {
            let id = format!("i_{}", &sha_hex(&format!("{}|{order}", item.id))[..32]);
            let strip = match vote_strips(item, order).first() {
                Some(png) => {
                    let name = format!("s_{}.png", &sha_hex(&id)[..16]);
                    write_atomic(&dir.join("strips").join(&name), png)?;
                    name
                }
                None => String::new(),
            };
            let context = context_for(item, order);
            let choices = choices_for(item);
            let evidence_hash = sha_hex(&format!(
                "{}|{}|{}|{}",
                item.question.text(),
                context,
                serde_json::to_string(&choices).map_err(|e| e.to_string())?,
                strip_bytes(dir, &strip)
                    .map(|p| sha_hex(&format!("{p:?}")))
                    .unwrap_or_default()
            ));
            items.push(VoteItem {
                id,
                item: item.id.clone(),
                order: order.to_owned(),
                question: item.question.as_str().to_owned(),
                kind: kind.as_str().to_owned(),
                kind_limits: kind.limits().to_owned(),
                text: item.question.text().to_owned(),
                context,
                choices,
                strip,
                pairwise: item.question == JudgeQuestion::Preference,
                evidence_hash: format!("sha256:{evidence_hash}"),
            });
        }
    }
    let run = VoteRun {
        schema: VOTES_SCHEMA.to_owned(),
        run_id: run_id.to_owned(),
        created_at_unix: now_ms() / 1000,
        items,
    };
    let text = serde_json::to_vec_pretty(&run).map_err(|e| e.to_string())?;
    write_atomic(&file, &text)?;
    Ok(run)
}

/// Reads a vote run.
pub fn read_run(dir: &Path) -> Result<VoteRun, String> {
    let text =
        std::fs::read_to_string(dir.join("run.json")).map_err(|_| "unknown vote run".to_owned())?;
    serde_json::from_str(&text).map_err(|e| format!("vote run is damaged: {e}"))
}

/// Every stored vote, the latest per voter and item.
pub fn votes(dir: &Path) -> Vec<VoteRec> {
    if std::fs::symlink_metadata(dir.join("votes.jsonl")).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Vec::new();
    }
    let text = std::fs::read_to_string(dir.join("votes.jsonl")).unwrap_or_default();
    let mut latest: BTreeMap<(String, String), VoteRec> = BTreeMap::new();
    for line in text.lines() {
        if let Ok(v) = serde_json::from_str::<VoteRec>(line) {
            latest.insert((v.voter.clone(), v.item.clone()), v);
        }
    }
    latest.into_values().collect()
}

/// Records one vote, checking it against the run's items and choices.
pub fn record_vote(dir: &Path, voter: &str, item: &str, answer: &str) -> Result<Value, String> {
    let voter = clean_voter(voter)
        .ok_or("the voter name must be 1 to 40 letters, digits, spaces, `_`, `-` or `.`")?;
    let run = read_run(dir)?;
    let it = run
        .items
        .iter()
        .find(|i| i.id == item)
        .ok_or("this run has no such item")?;
    if !it.choices.iter().any(|c| c.answer == answer) {
        return Err(format!("{answer:?} is not one of the choices"));
    }
    let rec = VoteRec {
        voter: voter.clone(),
        item: item.to_owned(),
        answer: answer.to_owned(),
        ts: now_ms(),
    };
    {
        let _g = VOTE_LOCK
            .lock()
            .map_err(|_| "vote store is busy".to_owned())?;
        if std::fs::symlink_metadata(dir.join("votes.jsonl"))
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err("the vote file must not be a symlink".into());
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("votes.jsonl"))
            .map_err(|e| format!("opening the vote file: {e}"))?;
        let line = serde_json::to_string(&rec).map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| format!("writing the vote: {e}"))?;
    }
    Ok(progress(&run, dir, &voter))
}

fn progress(run: &VoteRun, dir: &Path, voter: &str) -> Value {
    let mine: Vec<VoteRec> = votes(dir)
        .into_iter()
        .filter(|v| v.voter == voter)
        .collect();
    json!({"schema": VOTE_API_SCHEMA, "ok": true, "voter": voter, "done": mine.len(), "total": run.items.len()})
}

/// The items a voter sees, shuffled per voter (both orders of an item are
/// rarely adjacent), with their saved answers and progress. Nothing about
/// other voters or the judges' answers is included: the vote is blind.
pub fn items_json(dir: &Path, run_id: &str, voter: &str) -> Result<Value, String> {
    let run = read_run(dir)?;
    let mine: BTreeMap<String, String> = votes(dir)
        .into_iter()
        .filter(|v| v.voter == voter)
        .map(|v| (v.item, v.answer))
        .collect();
    let mut order: Vec<&VoteItem> = run.items.iter().collect();
    order.sort_by_key(|i| sha_hex(&format!("{voter}|{run_id}|{}", i.id)));
    let items: Vec<Value> = order
        .iter()
        .map(|i| {
            json!({
                "id": i.id, "question": i.question, "kind": i.kind, "kind_limits": i.kind_limits,
                "text": i.text, "context": i.context, "choices": i.choices, "pairwise": i.pairwise,
                "strip": if i.strip.is_empty() { Value::Null } else { json!(format!("/vote/{run_id}/strip/{}", i.strip)) },
                "answer": mine.get(&i.id),
            })
        })
        .collect();
    Ok(json!({
        "schema": VOTE_API_SCHEMA,
        "run_id": run_id, "voter": voter, "total": run.items.len(),
        "done": mine.len(), "items": items,
    }))
}

/// The bytes of a strip image; the name must be one this module wrote.
pub fn strip_bytes(dir: &Path, name: &str) -> Option<Vec<u8>> {
    let ok = name.starts_with("s_")
        && name.ends_with(".png")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
    let file = dir.join("strips").join(name);
    (ok && file.canonicalize().ok()? == file)
        .then(|| std::fs::read(file).ok())
        .flatten()
}

/// The vote page for a run, with the server token embedded.
pub fn page(token: &str, run_id: &str) -> String {
    let data = json!({"token": token, "run": run_id})
        .to_string()
        .replace("</", "<\\/");
    PAGE.replace("/*__VOTE_CSS__*/", CSS)
        .replace("/*__VOTE_JS__*/", JS)
        .replace("__VOTE_PAGE__", &data)
}

/// Writes the vote run (when missing) and turns the votes cast so far into
/// judgements, one judge per voter. Returns them with a summary for the result.
pub(crate) fn collect(
    decisions: &Path,
    run_id: &str,
    spec: &JudgeSpec,
    plan: &Plan,
) -> (Vec<Judgement>, Value) {
    let dir = run_dir(decisions, run_id);
    let run = match ensure_run(&dir, run_id, plan) {
        Ok(r) => r,
        Err(e) => return (Vec::new(), json!({"error": e})),
    };
    let mut out = Vec::new();
    let mut voters: BTreeMap<String, usize> = BTreeMap::new();
    for v in votes(&dir) {
        let Some(vi) = run.items.iter().find(|i| i.id == v.item) else {
            continue;
        };
        let Some(item) = plan.items.iter().find(|i| i.id == vi.item) else {
            continue;
        };
        if !spec.answers(item.question) || !vi.choices.iter().any(|c| c.answer == v.answer) {
            continue;
        }
        *voters.entry(v.voter.clone()).or_insert(0) += 1;
        let stable = stable_for(item, &vi.order, &v.answer);
        let abstained = is_abstain(&stable);
        out.push(Judgement {
            judge: format!("human:{}", v.voter),
            provider: "human".into(),
            model: String::new(),
            answered_model: None,
            model_version: None,
            order: if vi.order == "ba" { "ba" } else { "ab" },
            item: item.id.clone(),
            answer: Some(stable),
            wire_answer: Some(v.answer),
            prob: None,
            confidence: None,
            prob_source: Some("human_vote"),
            probs: BTreeMap::new(),
            abstained,
            abstain_reason: abstained.then(|| "the voter was unsure".to_owned()),
            latency_ms: 0,
            usage: Value::Null,
            attempts: Vec::new(),
            fallback_used: false,
            rubric_version: spec.rubric_version(),
            evidence_hash: vi.evidence_hash.clone(),
            timestamp_ms: v.ts,
        });
    }
    let info = json!({
        "path": format!("/vote/{run_id}"),
        "dir": dir.display().to_string(),
        "items": run.items.len(),
        "voters": voters,
        "pending": voters.is_empty(),
        "note": "open the path on a running `saccade serve`; run the same command again to include votes cast since",
    });
    (out, info)
}
