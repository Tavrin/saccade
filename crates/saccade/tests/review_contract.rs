//! Offline review acceptance: mocked providers only, at most ten new tests.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use saccade_core::judge::Provider;
use saccade_core::judge_provider::{AskRequest, Backend, CallOutcome, Raw};
use saccade_core::review::{self, Options, Profile};
use saccade_core::{Report, Status};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_saccade");
struct Mock {
    calls: Cell<usize>,
    uncertain: bool,
    disagree: bool,
    human_requested: bool,
}
impl Backend for Mock {
    fn ask(&self, r: &AskRequest<'_>) -> CallOutcome {
        self.calls.set(self.calls.get() + 1);
        let answer = if r.spec.provider == Provider::Gemini && r.question == "preference" {
            // The first image is the candidate only in ba; source state is blind.
            if r.prompt.user.contains("private-scene") {
                panic!("blind prompt leaked filename");
            }
            let ba = r
                .images
                .first()
                .and_then(|png| image::load_from_memory(png).ok())
                .is_some_and(|i| i.to_rgb8().get_pixel(0, 20)[0] > 90);
            if self.disagree {
                if ba { "P2" } else { "P1" }
            } else if ba {
                "P1"
            } else {
                "P2"
            }
        } else {
            match r.question {
                "cause" => "local_structure",
                "ask_human" | "needs_eyes" => {
                    if self.human_requested {
                        "yes"
                    } else {
                        "no"
                    }
                }
                _ => "accept",
            }
        };
        CallOutcome {
            result: Ok(Raw {
                answer: answer.into(),
                prob: Some(if self.uncertain && r.spec.provider == Provider::Jev {
                    0.5
                } else {
                    0.95
                }),
                confidence: Some(0.95),
                probs: Default::default(),
                prob_source: "verbalized",
                model_version: "offline".into(),
                model: r.spec.model.clone(),
                latency_ms: 12,
                usage: json!({}),
            }),
            attempts: Vec::new(),
        }
    }
    fn ask_many(&self, reqs: &[AskRequest<'_>]) -> Vec<CallOutcome> {
        let count = self.calls.get();
        let result = reqs.iter().map(|r| self.ask(r)).collect();
        self.calls.set(count + 1);
        result
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf, Report) {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("baseline");
    let b = tmp.path().join("capture");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    for (dir, red) in [(&a, 40), (&b, 140)] {
        let img = image::RgbImage::from_fn(64, 64, |x, y| {
            image::Rgb([
                if x < 24 && y < 24 {
                    red
                } else {
                    ((x + y) % 90 + 30) as u8
                },
                70,
                110,
            ])
        });
        img.save(dir.join("private-scene.png")).unwrap();
    }
    let out = tmp.path().join("report");
    let status = Command::new(BIN)
        .args(["compare"])
        .arg(&a)
        .arg(&b)
        .args(["--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(matches!(status.status.code(), Some(0 | 1)));
    let path = out.join("saccade-report.v1.json");
    let mut report: Report = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // Keep routing tests local instead of a frame-wide escalation.
    report.entries[0].hotspots[0].rect_frac = [0.0, 0.0, 0.3, 0.3];
    report.entries[0].hotspots[0].rect_px = [0, 0, 20, 20];
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    // This historical fixture edits its measured report. Remove the now-stale
    // canonical case so preview derives a case bound to that edited report.
    std::fs::remove_file(out.join("evidence.json")).unwrap();
    (tmp, path, report)
}
fn options(tmp: &Path, name: &str) -> Options {
    Options {
        profile: Profile::load(name).unwrap(),
        intent: Some("prefer the brighter red patch".into()),
        budget_calls: 10,
        max_gemini: None,
        dry_run: false,
        decisions_dir: tmp.join("decisions"),
        serve_root: tmp.into(),
        ocr_cmd: None,
    }
}
fn mock(uncertain: bool, disagree: bool) -> Mock {
    Mock {
        calls: Cell::new(0),
        uncertain,
        disagree,
        human_requested: false,
    }
}
fn schema(name: &str, v: &Value) {
    let expected = format!("saccade-{name}.v1");
    let successor = saccade_core::report_links::linked_schema(&expected);
    let id = if v["schema"] == successor {
        successor
    } else {
        &expected
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/schemas")
        .join(format!("{id}.schema.json"));
    let s: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(jsonschema::validator_for(&s).unwrap().is_valid(v), "{v}");
}
#[test]
fn deterministic_gate_skips_every_model_and_never_records() {
    let (tmp, path, mut report) = fixture();
    let normal = report.entries[0].clone();
    report.entries.clear();
    for n in 0..6 {
        let mut e = normal.clone();
        e.name = format!("{n}.png");
        match n {
            0 => e.bit_identical = Some(true),
            1 => {
                e.status = Status::Error;
                e.error = Some("decode".into());
            }
            2 => {
                e.properties.as_mut().unwrap().is_all_black = true;
            }
            3 => {
                e.meta_diff = vec![saccade_core::report::MetaDiff {
                    key: "resolution".into(),
                    baseline: "1".into(),
                    capture: "2".into(),
                }];
            }
            4 => {
                e.properties.as_mut().unwrap().nan_count = 1;
            }
            _ => {
                e.status = Status::Missing;
            }
        }
        report.entries.push(e);
    }
    let backend = mock(false, false);
    let v = review::run(&path, &report, &backend, &options(tmp.path(), "nightly")).unwrap();
    assert_eq!(backend.calls.get(), 0);
    assert_eq!(v["totals"]["deterministic"], 6);
    assert!(
        !path
            .parent()
            .unwrap()
            .join("saccade-decisions.v1.json")
            .exists()
    );
    schema("review", &v);
}
#[test]
fn jev_only_ci_keeps_the_comparison_verdict_and_proposes() {
    let (tmp, path, report) = fixture();
    let backend = mock(false, false);
    let v = review::run(&path, &report, &backend, &options(tmp.path(), "ci")).unwrap();
    assert_eq!(backend.calls.get(), 1);
    assert_eq!(v["entries"][0]["status"], "proposal");
    assert_eq!(v["gemini_entries"], 0);
    assert_eq!(v["calls_used"], 1);
    assert_eq!(v["entries"][0]["judges"].as_array().unwrap().len(), 3);
}
#[test]
fn escalation_is_blind_both_orders_and_strong_when_agreeing() {
    let (tmp, path, report) = fixture();
    let backend = mock(false, false);
    let v = review::run(&path, &report, &backend, &options(tmp.path(), "lookdev")).unwrap();
    assert_eq!(backend.calls.get(), 3);
    assert_eq!(v["entries"][0]["strong_proposal"], true, "{v}");
    assert_eq!(v["entries"][0]["status"], "proposal");
    assert!(v["vote_path"].as_str().unwrap().starts_with("/vote/"));
    let blind = review::blind(
        &saccade_core::judge::report_items(
            &report,
            path.parent().unwrap(),
            saccade_core::judge::JudgeQuestion::Decision(saccade_core::decision::Question::Accept),
            None,
            &["private-scene.png".into()],
            &Default::default(),
        )
        .unwrap()
        .0
        .remove(0),
    );
    assert_eq!(blind.states, [json!({}), json!({})]);
    let mut requested = mock(false, false);
    requested.human_requested = true;
    let v = review::run(&path, &report, &requested, &options(tmp.path(), "lookdev")).unwrap();
    assert_eq!(v["entries"][0]["status"], "needs_human");
    assert_eq!(v["entries"][0]["strong_proposal"], false);
    assert!(v["entries"][0]["inbox"].is_object());
}
#[test]
fn disagreement_creates_inbox_and_a_vote_page_with_exact_view() {
    let (tmp, path, report) = fixture();
    let backend = mock(false, true);
    let v = review::run(&path, &report, &backend, &options(tmp.path(), "lookdev")).unwrap();
    assert_eq!(v["entries"][0]["status"], "needs_human");
    let it = saccade_core::inbox::get(
        &tmp.path().join("decisions/inbox"),
        v["entries"][0]["inbox"]["id"].as_str().unwrap(),
    )
    .unwrap();
    assert!(
        it.link
            .unwrap()
            .contains("#set=private-scene.png&layout=swipe")
    );
    assert_eq!(
        saccade_core::judge_vote::read_run(&saccade_core::judge_vote::run_dir(
            &tmp.path().join("decisions"),
            v["run_id"].as_str().unwrap()
        ))
        .unwrap()
        .items
        .len(),
        2
    );
}
#[test]
fn budget_caps_and_dry_run_make_no_provider_calls() {
    let (tmp, path, report) = fixture();
    let backend = mock(true, false);
    let mut opts = options(tmp.path(), "nightly");
    opts.budget_calls = 1;
    let v = review::run(&path, &report, &backend, &opts).unwrap();
    assert_eq!(backend.calls.get(), 1);
    assert_eq!(v["gemini_entries"], 0);
    assert_eq!(v["entries"][0]["status"], "needs_human");
    opts.dry_run = true;
    let v = review::run(&path, &report, &backend, &opts).unwrap();
    assert_eq!(backend.calls.get(), 1);
    assert_eq!(v["calls_used"], 0);
    opts.dry_run = false;
    opts.budget_calls = 10;
    opts.max_gemini = Some(0);
    let v = review::run(&path, &report, &backend, &opts).unwrap();
    assert_eq!(v["gemini_entries"], 0);
}
#[test]
fn label_round_trip_excludes_models_and_collects_inbox_and_votes() {
    let (tmp, path, report) = fixture();
    let backend = mock(false, true);
    let opts = options(tmp.path(), "lookdev");
    let v = review::run(&path, &report, &backend, &opts).unwrap();
    let out = tmp.path().join("labels.json");
    assert!(
        saccade_core::labels::collect(Some(&opts.decisions_dir), std::slice::from_ref(&path), &out)
            .unwrap()
            .items
            .is_empty()
    );
    let id = v["entries"][0]["inbox"]["id"].as_str().unwrap();
    saccade_core::inbox::answer(&opts.decisions_dir.join("inbox"), id, "reject".into(), None)
        .unwrap();
    let set = saccade_core::labels::collect(Some(&opts.decisions_dir), &[], &out).unwrap();
    assert_eq!(set.items.len(), 1);
    schema("labels", &serde_json::to_value(&set).unwrap());
    assert_eq!(
        saccade_core::labels::Labels::read(&out).unwrap().items[0].answer,
        "reject"
    );
    let vote_dir =
        saccade_core::judge_vote::run_dir(&opts.decisions_dir, v["run_id"].as_str().unwrap());
    let run = saccade_core::judge_vote::read_run(&vote_dir).unwrap();
    for item in &run.items {
        saccade_core::judge_vote::record_vote(
            &vote_dir,
            "alice",
            &item.id,
            if item.order == "ab" { "P1" } else { "P2" },
        )
        .unwrap();
    }
    let combined = saccade_core::labels::collect(Some(&opts.decisions_dir), &[], &out).unwrap();
    assert_eq!(combined.items.len(), 1);
    assert_eq!(combined.items[0].provenance.len(), 2);
    let replay = saccade_core::labels::replay(&set.items[0], &out).unwrap();
    assert_eq!(replay.entry, "private-scene.png");
    std::fs::write(
        saccade_core::paths::resolve(&set.items[0].images[0], &out),
        b"changed",
    )
    .unwrap();
    assert!(saccade_core::labels::replay(&set.items[0], &out).is_err());
}
#[test]
fn bench_ordering_and_calibration_maths_count_missing_coverage() {
    let public =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/labels/showcase-truths.json");
    let public_labels = saccade_core::labels::Labels::read(&public).unwrap();
    assert_eq!(public_labels.items.len(), 20);
    for item in &public_labels.items {
        saccade_core::labels::replay(item, &public).unwrap();
    }
    let rows = vec![
        json!({"provider":"gemini","model":"older","accuracy":0.9,"coverage":1.0,"latency_ms":30}),
        json!({"provider":"gemini","model":"newer","accuracy":0.8,"coverage":1.0,"latency_ms":10}),
        json!({"provider":"gemini","model":"fast","accuracy":0.9,"coverage":1.0,"latency_ms":20}),
        json!({"provider":"gemini","model":"missing","accuracy":1.0,"coverage":0.5,"latency_ms":1}),
    ];
    assert_eq!(
        saccade_core::judge_bench::chain_order(&rows),
        vec!["fast", "older", "newer"]
    );
    assert!(
        (saccade_core::judge_stats::ece(&[
            saccade_core::judge_stats::Scored {
                conf: 0.8,
                correct: true
            },
            saccade_core::judge_stats::Scored {
                conf: 0.8,
                correct: false
            }
        ])
        .unwrap()
            - 0.3)
            .abs()
            < 1e-10
    );
    let (tmp, path, report) = fixture();
    let answer = saccade_core::decision::Answer {
        entry: "private-scene.png".into(),
        question: saccade_core::decision::Question::Accept,
        hotspot: None,
        answer: "accept".into(),
        prob: None,
        confidence: None,
        source: "human".into(),
        note: String::new(),
        request_hash: None,
    };
    saccade_core::decision::decide_report(&path, &report, &Default::default(), &answer).unwrap();
    seed_historical_final(&path);
    let labels_path = tmp.path().join("labels.json");
    let labels = saccade_core::labels::collect(None, &[path], &labels_path).unwrap();
    let models = vec!["older".into(), "newer".into()];
    let mut uncropped = labels.clone();
    uncropped.items[0].hotspots.clear();
    uncropped.items[0].evidence_hash = saccade_core::labels::identity(&uncropped.items[0]);
    let no_calls = mock(false, false);
    assert!(
        saccade_core::judge_bench::run(&labels_path, &uncropped, &models, &[], &no_calls, 6)
            .is_err()
    );
    assert_eq!(no_calls.calls.get(), 0);
    let result =
        saccade_core::judge_bench::run(&labels_path, &labels, &models, &[], &mock(false, false), 6)
            .unwrap();
    schema("judge-bench", &result);
    assert_eq!(result["calls_used"], 6);
    assert!(
        result["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["accuracy"] == 1.0 && r["coverage"] == 1.0)
    );
    let bench = tmp.path().join("bench.json");
    review::write_json(&bench, &result).unwrap();
    let calibration =
        saccade_core::judge_stats::calibrate(&[labels_path], &[bench], &Default::default())
            .unwrap();
    assert_eq!(calibration["labelled_units"], 1);
    assert_eq!(calibration["judges"].as_array().unwrap().len(), 6);
}
#[test]
fn never_final_and_local_preview_contracts_are_enforced() {
    let (tmp, path, report) = fixture();
    let backend = mock(false, false);
    review::run(&path, &report, &backend, &options(tmp.path(), "lookdev")).unwrap();
    let decisions = saccade_core::view::read_decisions(
        &path.parent().unwrap().join("saccade-decisions.v1.json"),
    )
    .unwrap();
    assert!(
        decisions
            .sets
            .iter()
            .all(|s| s.decision.is_none() && s.proposals.iter().all(|p| p.proposed && !p.promoted))
    );
    let output = Command::new(BIN)
        .arg("review")
        .arg(&path)
        .arg("--user-config")
        .arg(tmp.path().join("unconfigured-user.toml"))
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["schema"],
        saccade_core::report_links::linked_schema("saccade-result.v2")
    );
    assert_eq!(value["counts"]["dispatched_calls"], 0);
}

#[test]
fn offline_preview_writes_requests_and_estimates_configured_price() {
    let (tmp, path, _) = fixture();
    let config = tmp.path().join("user.toml");
    std::fs::write(
        &config,
        "[pricing.\"jev/jev-latest\"]\ninput_per_million_usd = 1.0\noutput_per_million_usd = 2.0\n",
    )
    .unwrap();
    let out = tmp.path().join("preview");
    let result = Command::new(BIN)
        .current_dir(tmp.path())
        .arg("review")
        .arg(&path)
        .args(["--out"])
        .arg(&out)
        .args(["--user-config"])
        .arg(&config)
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["counts"]["dispatched_calls"], 0);
    assert!(out.join("preview.json").is_file() && out.join("requests.json").is_file());
    let planned = &value["data"]["payloads"][0];
    assert!(planned["request_bytes"].as_u64().unwrap() > 0);
    assert!(planned["estimated_input_tokens"].as_u64().unwrap() > 0);
    assert!(planned["estimated_cost_usd"].as_f64().unwrap() > 0.0);
    assert!(
        planned["source_roots"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| !s.as_str().unwrap().starts_with('/'))
    );
}

fn seed_historical_final(report: &Path) {
    let path = report.parent().unwrap().join("saccade-decisions.v1.json");
    let mut d = saccade_core::view::read_decisions(&path).unwrap();
    for set in &mut d.sets {
        set.decision = Some(saccade_core::view::Verdict::Accept);
        for p in &mut set.proposals {
            if p.source == "human" {
                p.proposed = false;
            }
        }
    }
    std::fs::write(path, serde_json::to_vec(&d).unwrap()).unwrap();
}

#[test]
fn blind_ties_never_become_accept_proposals() {
    assert_eq!(saccade_core::review::candidate_answer("tie"), "needs_human");
    assert_eq!(saccade_core::review::candidate_answer("b"), "accept");
}
