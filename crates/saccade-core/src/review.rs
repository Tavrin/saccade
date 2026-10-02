//! Budgeted AI review cascade. Every model answer remains a proposal.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::decision::{Answer, Question, deterministic_failure, propose_report};
use crate::judge::{
    self, EvidenceOptions, JudgeItem, JudgeQuestion, JudgeSpec, Order, Panel, Plan, Provider,
};
use crate::judge_provider::{AskRequest, Backend, CallOutcome};
use crate::report::{Report, Status};
use crate::{Error, Result};

/// Default availability chain; benchmarking may suggest a different order.
pub const GEMINI_MODELS: [&str; 4] = [
    "gemini-3.8-flash",
    "gemini-3.7-flash",
    "gemini-3.6-flash",
    "gemini-3.5-flash",
];
/// Review document schema.
pub const SCHEMA: &str = "saccade-review.v1";
/// Review result filename.
pub const FILE: &str = "saccade-review.v1.json";

fn default_models() -> Vec<String> {
    GEMINI_MODELS.map(str::to_owned).to_vec()
}

/// Review profile. Unknown TOML fields are refused.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    /// Human-readable profile name.
    pub name: String,
    /// never, escalation, always, or layout.
    pub gemini: String,
    /// Minimum probability and reported confidence of a committed answer.
    pub confidence: f64,
    /// Most entries receiving Gemini calls.
    pub max_gemini: usize,
    /// Entries per Jev batch (three questions each).
    pub chunk_entries: usize,
    /// Availability chain in order.
    pub gemini_models: Vec<String>,
    /// Failed model cooldown duration.
    pub cooldown_secs: u64,
    /// Retries per model after the initial attempt.
    pub max_retries: u32,
    /// Always include a human vote page.
    pub human_votes: bool,
    /// Local OCR wrapper, only run for UI profiles or explicit CLI configuration.
    pub ocr_cmd: Option<String>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "nightly".into(),
            gemini: "escalation".into(),
            confidence: 0.75,
            max_gemini: 5,
            chunk_entries: 8,
            gemini_models: default_models(),
            cooldown_secs: 600,
            max_retries: 2,
            human_votes: false,
            ocr_cmd: None,
        }
    }
}
impl Profile {
    /// Built-in template text, also used by init.
    pub fn template(name: &str) -> Option<&'static str> {
        match name {
            "ci" => Some(include_str!("review_profiles/ci.toml")),
            "nightly" => Some(include_str!("review_profiles/nightly.toml")),
            "lookdev" => Some(include_str!("review_profiles/lookdev.toml")),
            "ui" => Some(include_str!("review_profiles/ui.toml")),
            _ => None,
        }
    }
    /// Parse and validate a profile.
    pub fn parse(text: &str) -> Result<Self> {
        let p: Self =
            toml::from_str(text).map_err(|e| Error::Config(format!("review profile: {e}")))?;
        if !["never", "escalation", "always", "layout"].contains(&p.gemini.as_str())
            || !p.confidence.is_finite()
            || !(0.0..=1.0).contains(&p.confidence)
            || p.chunk_entries == 0
            || p.chunk_entries > 64
            || p.max_retries > 5
            || p.gemini_models.is_empty()
        {
            return Err(Error::Config(
                "invalid review profile policy, threshold, chunk size or chain".into(),
            ));
        }
        // Apply the existing provider identifier validation.
        let _ = p.spec(Provider::Gemini)?;
        Ok(p)
    }
    /// Load a built-in name or a custom TOML file.
    pub fn load(name: &str) -> Result<Self> {
        let text = match Self::template(name) {
            Some(s) => s.to_owned(),
            None => std::fs::read_to_string(name)
                .map_err(crate::run::io_err(format!("reading profile {name}")))?,
        };
        Self::parse(&text)
    }
    /// Provider specification using the established key and model policy.
    pub fn spec(&self, provider: Provider) -> Result<JudgeSpec> {
        let models = if provider == Provider::Jev {
            vec!["jev-latest".to_owned()]
        } else {
            self.gemini_models.clone()
        };
        let text = format!(
            "[[judge]]\nprovider = {:?}\nmodel = {:?}\nfallback = {}\nvision = {}\nrole = \"a careful visual reviewer\"\nrubric = \"Use only the evidence and intent. Abstain when uncertain.\"",
            provider.as_str(),
            models.first().map_or("", String::as_str),
            serde_json::to_string(&models.get(1..).unwrap_or(&[]))?,
            provider == Provider::Gemini
        );
        Panel::parse(&text)?
            .judges
            .into_iter()
            .next()
            .ok_or_else(|| Error::Config("empty review provider".into()))
    }
}

/// Options independent of the provider transport.
pub struct Options {
    /// Profile.
    pub profile: Profile,
    /// Intent for the change.
    pub intent: Option<String>,
    /// Actual HTTP attempt cap.
    pub budget_calls: usize,
    /// Override the profile's entry cap.
    pub max_gemini: Option<usize>,
    /// Plan without calls or writes.
    pub dry_run: bool,
    /// The human inbox and vote store.
    pub decisions_dir: PathBuf,
    /// Root served by saccade serve, for exact deep links.
    pub serve_root: PathBuf,
    /// Explicit OCR command override.
    pub ocr_cmd: Option<String>,
}

struct Replay(RefCell<Option<CallOutcome>>);
impl Backend for Replay {
    fn ask(&self, _: &AskRequest<'_>) -> CallOutcome {
        self.0.borrow_mut().take().unwrap_or(CallOutcome {
            result: Err("missing batch result".into()),
            attempts: Vec::new(),
        })
    }
}

/// Content hash used by labels and provider-independent evidence identities.
pub fn hash(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.to_string().as_bytes());
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// Stable evidence identity, including image hashes, encoded state and intent.
pub fn evidence_hash(item: &JudgeItem, report: &Report) -> String {
    let entry = report.entries.iter().find(|e| e.name == item.entry);
    hash(&json!([
        item.entry,
        item.question.as_str(),
        item.states,
        item.intent,
        [
            entry.and_then(|e| e.baseline_sha256.as_ref()),
            entry.and_then(|e| e.capture_sha256.as_ref())
        ],
        item.hotspots
    ]))
}

/// Make a genuinely blind pairwise item: no filenames, sides, FLIP or diagnosis.
pub fn blind(item: &JudgeItem) -> JudgeItem {
    let mut out = item.clone();
    out.question = JudgeQuestion::Preference;
    out.states = [json!({}), json!({})];
    out.intent = Some(format!(
        "Which image best satisfies this intent: {}? Choose tie only when equally suitable; unsure when the crops cannot establish this.",
        item.intent
            .as_deref()
            .unwrap_or("preserve the approved appearance without new defects")
    ));
    out
}

/// Convert blind preference to a proposal about the candidate.
pub fn candidate_answer(answer: &str) -> &str {
    match answer {
        "a" => "reject",
        "b" | "tie" => "accept",
        _ => "needs_human",
    }
}

fn confident(j: &judge::Judgement, min: f64) -> bool {
    !j.abstained
        && j.prob.is_some_and(|p| p >= min)
        && j.confidence
            .is_none_or(|c| c.is_finite() && c >= min && c <= 1.0)
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn deep_link(report: &Report, document: &Path, entry: &str, root: &Path) -> String {
    let dirs = [&report.baseline_dir, &report.capture_dir]
        .into_iter()
        .filter_map(|p| p.as_ref())
        .map(|p| crate::paths::record(&crate::paths::resolve(p, document), root, false))
        .map(|p| format!("run={}", encode(&p)))
        .collect::<Vec<_>>()
        .join("&");
    let at = report
        .entries
        .iter()
        .find(|e| e.name == entry)
        .and_then(|e| e.hotspots.first())
        .map(|h| {
            format!(
                "&at={},{}",
                h.rect_px[0] + h.rect_px[2] / 2,
                h.rect_px[1] + h.rect_px[3] / 2
            )
        })
        .unwrap_or_default();
    format!(
        "/compare?{dirs}#set={}&layout=swipe&split=0.5{at}&zoom=2",
        encode(entry)
    )
}

/// Run the cascade with an injectable provider backend.
pub fn run(
    document: &Path,
    report: &Report,
    backend: &dyn Backend,
    opts: &Options,
) -> Result<Value> {
    let dir = document
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let spec = opts.profile.spec(Provider::Jev)?;
    let gemini = opts.profile.spec(Provider::Gemini)?;
    let mut rows = Vec::new();
    let mut names = Vec::new();
    for e in &report.entries {
        let deterministic = deterministic_failure(report, e)
            .or_else(|| (!e.meta_diff.is_empty()).then(|| "config mismatch".into()))
            .or_else(|| e.bit_identical.filter(|v| *v).map(|_| "identical".into()))
            .or_else(|| {
                matches!(e.status, Status::Missing | Status::New)
                    .then(|| format!("{:?}: no compared pair", e.status))
            });
        let deterministic = deterministic.or_else(|| {
            e.diagnostics
                .as_ref()
                .filter(|d| d.class == crate::diagnostics::ChangeClass::BrokenFrame)
                .map(|d| d.description.clone())
        });
        let reason = deterministic.clone();
        if deterministic.is_none() {
            names.push(e.name.clone());
        }
        rows.push(json!({"entry": e.name, "status": if deterministic.is_some() {"deterministic"} else {"needs_human"},
            "path": ["deterministic"], "reason": reason, "judges": [], "agreement": null,
            "strong_proposal": false, "answer": null, "inbox": null, "vote_path": null, "evidence_hash": null}));
    }
    let (items, _) = if names.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        judge::report_items(
            report,
            dir,
            JudgeQuestion::Decision(Question::Accept),
            opts.intent.as_deref(),
            &names,
            &EvidenceOptions {
                ocr_cmd: opts
                    .ocr_cmd
                    .clone()
                    .or_else(|| opts.profile.ocr_cmd.clone()),
            },
        )
        .map_err(|e| Error::Config(e.to_string()))?
    };
    let mut js: BTreeMap<String, Vec<judge::Judgement>> = BTreeMap::new();
    let mut used = 0;
    let mut jev_batches = 0;
    let mut gemini_entries = 0;
    for chunk in items.chunks(opts.profile.chunk_entries) {
        if opts.dry_run || used >= opts.budget_calls {
            continue;
        }
        let questions: Vec<_> = chunk
            .iter()
            .flat_map(|i| {
                [Question::Accept, Question::Cause, Question::AskHuman].map(|q| {
                    let mut it = i.clone();
                    it.question = JudgeQuestion::Decision(q);
                    it.id = format!(
                        "{}:{}",
                        i.entry,
                        if q == Question::AskHuman {
                            "needs_eyes"
                        } else {
                            q.as_str()
                        }
                    );
                    it.request_hash = hash(&json!([q.as_str(), i.states, i.request_hash]));
                    it
                })
            })
            .collect();
        let prepared: Vec<_> = questions
            .iter()
            .map(|i| {
                let wire = i.question.wire_answers();
                let (prompt, state) = judge::build_prompt(&spec, i, Order::Ab, &wire, 0);
                (wire, prompt, state)
            })
            .collect();
        let requests: Vec<_> = questions
            .iter()
            .zip(&prepared)
            .map(|(i, (wire, prompt, state))| AskRequest {
                spec: &spec,
                question: if i.question == JudgeQuestion::Decision(Question::AskHuman) {
                    "needs_eyes"
                } else {
                    i.question.as_str()
                },
                question_text: i.question.text(),
                kind: i.question.kind().as_str(),
                wire_answers: wire,
                state,
                prompt,
                images: &[],
            })
            .collect();
        let outcomes = backend.ask_many(&requests);
        used += 1;
        jev_batches += 1;
        for (index, i) in questions.iter().enumerate() {
            let out = outcomes.get(index);
            let replay = Replay(RefCell::new(Some(CallOutcome {
                result: out
                    .map(|o| o.result.clone())
                    .unwrap_or_else(|| Err("missing batch answer".into())),
                attempts: out.map(|o| o.attempts.clone()).unwrap_or_default(),
            })));
            let j = judge::judge_one(&replay, &spec, i, Order::Ab, &mut 0, usize::MAX);
            js.entry(i.entry.clone()).or_default().push(j);
        }
        if let Some(counts) = backend.http_counts() {
            used = counts.iter().sum();
        }
    }
    let mut escalation = Vec::new();
    for (index, i) in items.iter().enumerate() {
        let judges = js.get(&i.entry).map_or(&[][..], Vec::as_slice);
        let uncertain = judges.len() < 3
            || judges
                .iter()
                .any(|j| !confident(j, opts.profile.confidence))
            || judges
                .first()
                .is_none_or(|j| j.answer.as_deref() == Some("needs_human"))
            || judges
                .get(2)
                .is_none_or(|j| j.answer.as_deref() != Some("no"));
        let e = report.entries.iter().find(|e| e.name == i.entry);
        let high = e.is_some_and(|e| e.regions.iter().any(|r| r.status == Some(Status::Fail)))
            || i.states[0]["frame_wide"] == true;
        let layout = e.and_then(|e| e.diagnostics.as_ref()).is_some_and(|d| {
            matches!(
                d.class,
                crate::diagnostics::ChangeClass::LocalStructure
                    | crate::diagnostics::ChangeClass::Misaligned
            )
        });
        let ask = match opts.profile.gemini.as_str() {
            "never" => false,
            "always" => true,
            "layout" => uncertain || high || layout,
            _ => uncertain || high,
        };
        if ask {
            escalation.push((index, e.and_then(|e| e.value).unwrap_or(f64::INFINITY)));
        }
    }
    escalation.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let max = opts.max_gemini.unwrap_or(opts.profile.max_gemini);
    for (index, _) in &escalation {
        let i = &items[*index];
        if opts.dry_run || gemini_entries >= max || used + 2 > opts.budget_calls {
            continue;
        }
        let b = blind(i);
        if judge::strips_for(&b, Order::Ab).is_empty() {
            continue;
        }
        gemini_entries += 1;
        for order in [Order::Ab, Order::Ba] {
            let j = judge::judge_one(backend, &gemini, &b, order, &mut 0, usize::MAX);
            used += 1;
            if let Some(counts) = backend.http_counts() {
                used = counts.iter().sum();
            }
            js.entry(i.entry.clone()).or_default().push(j);
        }
    }
    let mut human_items = Vec::new();
    let mut recorded = Vec::new();
    for (index, i) in items.iter().enumerate() {
        let judges = js.remove(&i.entry).unwrap_or_default();
        let jev = judges.first();
        let gj: Vec<_> = judges.iter().filter(|j| j.provider == "gemini").collect();
        let needs_eyes = judges.get(2).is_none_or(|j| {
            j.answer.as_deref() != Some("no") || !confident(j, opts.profile.confidence)
        });
        let jev_confident = judges.iter().filter(|j| j.provider == "jev").count() == 3
            && judges
                .iter()
                .filter(|j| j.provider == "jev")
                .all(|j| confident(j, opts.profile.confidence));
        let agrees = jev_confident
            && !needs_eyes
            && gj.len() == 2
            && gj.iter().all(|j| confident(j, opts.profile.confidence))
            && gj[0].answer == gj[1].answer
            && jev.and_then(|j| j.answer.as_deref())
                == gj[0].answer.as_deref().map(candidate_answer);
        let pending = escalation.iter().any(|(n, _)| *n == index);
        let needs_human = if !gj.is_empty() {
            !agrees
        } else {
            pending || needs_eyes || !jev_confident
        };
        let status = if opts.dry_run || needs_human {
            "needs_human"
        } else {
            "proposal"
        };
        let final_answer = if status == "proposal" {
            jev.and_then(|j| j.answer.as_deref())
                .unwrap_or("needs_human")
        } else {
            "needs_human"
        };
        let eh = evidence_hash(i, report);
        let row = rows
            .iter_mut()
            .find(|r| r["entry"] == i.entry)
            .ok_or_else(|| Error::Config("missing review entry".into()))?;
        row["path"] = json!(if gj.is_empty() {
            vec!["deterministic", "jev"]
        } else {
            vec!["deterministic", "jev", "gemini_both_orders"]
        });
        row["status"] = json!(status);
        row["answer"] = json!(final_answer);
        row["strong_proposal"] = json!(agrees);
        row["agreement"] = if gj.is_empty() {
            Value::Null
        } else {
            json!(agrees)
        };
        row["evidence_hash"] = json!(eh);
        row["state"] = i.states[0].clone();
        row["hotspots"] = json!(i.hotspots);
        row["judges"] = json!(judges);
        row["high_stakes"] = json!(
            i.states[0]["frame_wide"] == true
                || report
                    .entries
                    .iter()
                    .find(|e| e.name == i.entry)
                    .is_some_and(|e| e.regions.iter().any(|r| r.status == Some(Status::Fail)))
        );
        if !opts.dry_run {
            for (n, j) in judges.iter().enumerate() {
                let Some(answer) = &j.answer else {
                    continue;
                };
                let question = if j.provider == "gemini" {
                    Question::Accept
                } else {
                    [Question::Accept, Question::Cause, Question::AskHuman][n.min(2)]
                };
                let answer = if j.provider == "gemini" {
                    candidate_answer(answer)
                } else if answer == "unsure" {
                    continue;
                } else {
                    answer
                };
                let a = Answer {
                    entry: i.entry.clone(),
                    question,
                    hotspot: None,
                    answer: answer.to_owned(),
                    prob: j.prob,
                    confidence: j.confidence,
                    source: format!("review:{}:{}", j.provider, j.order),
                    note: serde_json::to_string(&json!({"judgement": j, "evidence_hash": eh}))?,
                    request_hash: Some(i.request_hash.clone()),
                };
                let o = propose_report(document, report, &a)
                    .map_err(|e| Error::Config(e.to_string()))?;
                recorded.push(json!({"entry": i.entry, "source": a.source, "proposed": o.proposed, "decided": o.decided}));
            }
            let a = Answer {
                entry: i.entry.clone(),
                question: Question::Accept,
                hotspot: None,
                answer: final_answer.into(),
                prob: jev.and_then(|j| j.prob).or(Some(0.0)),
                confidence: None,
                source: "review".into(),
                note: format!(
                    "{}; evidence_hash={eh}; strong_proposal={agrees}",
                    opts.profile.name
                ),
                request_hash: Some(i.request_hash.clone()),
            };
            let _ =
                propose_report(document, report, &a).map_err(|e| Error::Config(e.to_string()))?;
            if needs_human {
                let inbox_dir = opts.decisions_dir.join("inbox");
                std::fs::create_dir_all(&inbox_dir)
                    .map_err(crate::run::io_err("creating review inbox".into()))?;
                let link = deep_link(report, document, &i.entry, &opts.serve_root);
                let item = crate::inbox::post(&inbox_dir, crate::inbox::Question {
                    question: format!("Is the change in {} acceptable for the intent?", i.entry),
                    allowed_answers: vec!["accept".into(), "reject".into(), "needs_human".into()],
                    context: Some(json!({"entry": i.entry, "report": crate::paths::portable(document), "evidence_hash": eh,
                        "request_hash": i.request_hash, "state": i.states[0], "judges": judges, "intent": i.intent}).to_string()),
                    link: Some(link.clone()), from: Some("saccade review".into())
                })?;
                row["inbox"] =
                    json!({"id": item.id, "path": format!("/inbox#{}", item.id), "view": link});
            }
        }
        if needs_human || opts.profile.human_votes {
            human_items.push(blind(i));
        }
    }
    let run_id = hash(&json!([
        opts.profile,
        items
            .iter()
            .map(|i| evidence_hash(i, report))
            .collect::<Vec<_>>()
    ]));
    let run_id = run_id.trim_start_matches("sha256:")[..16].to_owned();
    let vote_path = (!human_items.is_empty()).then(|| format!("/vote/{run_id}"));
    if !opts.dry_run && !human_items.is_empty() {
        let plan = Plan {
            items: human_items,
            skipped: Vec::new(),
            run_id: run_id.clone(),
        };
        crate::judge_vote::ensure_run(
            &crate::judge_vote::run_dir(&opts.decisions_dir, &run_id),
            &run_id,
            &plan,
        )
        .map_err(Error::Config)?;
        // Keep the evidence identity and candidate mapping server-side for final label collection.
        write_json(&crate::judge_vote::run_dir(&opts.decisions_dir, &run_id).join("review-labels.json"),
            &json!(items.iter().map(|i| json!({"item":i.id,"entry":i.entry,"evidence_hash":evidence_hash(i,report),
                "state":i.states[0],"intent":i.intent, "report":crate::paths::portable(document)})).collect::<Vec<_>>()))?;
    }
    for r in &mut rows {
        if r["status"] == "needs_human" || opts.profile.human_votes && r["status"] == "proposal" {
            r["vote_path"] = json!(vote_path);
        }
    }
    let counts = backend.http_counts();
    let totals = json!({"entries": rows.len(), "deterministic": rows.iter().filter(|r| r["status"] == "deterministic").count(),
        "proposals": rows.iter().filter(|r| r["status"] == "proposal").count(), "needs_human": rows.iter().filter(|r| r["status"] == "needs_human").count()});
    Ok(json!({"schema": SCHEMA,
        "dry_run_plan": if opts.dry_run { json!({"jev_chunks":items.len().div_ceil(opts.profile.chunk_entries),
            "questions":["accept","cause","needs_eyes"],"needs_eyes_wire":"ask_human",
            "gemini_entry_cap":max,"gemini_requires_both_orders":true,"budget_includes_retries":true}) } else { Value::Null }, "dry_run": opts.dry_run, "profile": opts.profile, "intent": opts.intent,
        "entries": rows, "totals": totals, "calls_used": used, "http_attempts": counts, "jev_batches": jev_batches,
        "gemini_entries": gemini_entries, "budget_calls": opts.budget_calls, "max_gemini": max,
        "estimated_cost_usd": null, "cost_note": "No pinned provider rates; usage and actual models are recorded per call.",
        "recorded": recorded, "vote_path": vote_path, "run_id": run_id,
        "escalation_order": escalation.iter().map(|(n,_)| &items[*n].entry).collect::<Vec<_>>(),
        "never_final": true}))
}

/// Write a JSON artifact atomically.
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .map_err(crate::run::io_err("creating JSON temporary file".into()))?;
    tmp.write_all(format!("{}\n", serde_json::to_string_pretty(value)?).as_bytes())
        .map_err(crate::run::io_err("writing JSON".into()))?;
    tmp.persist(path)
        .map_err(|e| Error::Config(format!("persisting {}: {e}", path.display())))?;
    Ok(())
}

/// Concise text audit including every model's answer, uncertainty and latency.
pub fn text(v: &Value) -> String {
    let mut s = String::new();
    for e in v["entries"].as_array().into_iter().flatten() {
        s.push_str(&format!(
            "{}: {} / {} ({})\n",
            e["entry"].as_str().unwrap_or(""),
            e["status"].as_str().unwrap_or(""),
            e["answer"].as_str().unwrap_or("-"),
            e["path"]
        ));
        for j in e["judges"].as_array().into_iter().flatten() {
            s.push_str(&format!(
                "  {} {} {}: {} prob={} confidence={} latency={}ms\n",
                j["provider"].as_str().unwrap_or(""),
                j["answered_model"].as_str().unwrap_or("unavailable"),
                j["item"].as_str().unwrap_or(""),
                j["answer"].as_str().unwrap_or("unsure"),
                j["prob"],
                j["confidence"],
                j["latency_ms"]
            ));
        }
        if let Some(link) = e["inbox"]["view"].as_str() {
            s.push_str(&format!("  human: {link}\n"));
        }
    }
    s.push_str(&format!(
        "totals: {}; calls={} / {}; estimated cost=unknown; never final\n",
        v["totals"], v["calls_used"], v["budget_calls"]
    ));
    s
}
