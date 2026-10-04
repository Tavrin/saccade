//! Ten offline acceptance checks for judge mode. No provider calls or keys.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use saccade_core::decision::{DecisionsConfig, Question};
use saccade_core::judge::*;
use saccade_core::judge_canary;
use saccade_core::judge_evidence::*;
use saccade_core::judge_provider::*;
use saccade_core::judge_stats::*;
use saccade_core::judge_vote as vote;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_saccade");

fn panel() -> Panel {
    Panel::parse(
        r#"
[panel]
canary_rate = 0
[[judge]]
provider = "jev"
model = "jev-latest"
[[judge]]
provider = "human"
role = "a reviewer"
"#,
    )
    .unwrap()
}
fn options(dir: Option<PathBuf>) -> RunOptions {
    RunOptions {
        question: JudgeQuestion::Decision(Question::Accept),
        both_orders: true,
        canary_rate: Some(0.0),
        max_calls: 100,
        decisions_dir: dir,
        calibration: None,
    }
}
fn item() -> JudgeItem {
    let mut items = canary_items(2);
    let mut i = items.remove(1);
    i.canary = None;
    i
}
struct Mock {
    calls: AtomicUsize,
    flip: bool,
}
impl Backend for Mock {
    fn ask(&self, r: &AskRequest<'_>) -> CallOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = if self.flip && r.state["sides"]["reference"] == "P2" {
            "accept"
        } else {
            "reject"
        };
        CallOutcome {
            result: Ok(Raw {
                answer: answer.into(),
                prob: Some(0.9),
                confidence: Some(0.8),
                probs: BTreeMap::new(),
                prob_source: "verbalized",
                model_version: "fixture-v1".into(),
                model: r.spec.model.clone(),
                latency_ms: 1,
                usage: Value::Null,
            }),
            attempts: vec![],
        }
    }
}
fn mock() -> Mock {
    Mock {
        calls: AtomicUsize::new(0),
        flip: false,
    }
}
fn check_schema(name: &str, v: &Value) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../saccade-core/schemas")
        .join(format!("saccade-{name}.v1.schema.json"));
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = validator.iter_errors(v).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}
fn report(tmp: &Path) -> (saccade_core::Report, PathBuf) {
    let c = judge_canary::gold_set().remove(1);
    let (a, b, out) = (tmp.join("base"), tmp.join("cap"), tmp.join("report"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    c.baseline.save(a.join("scene.png")).unwrap();
    c.capture.save(b.join("scene.png")).unwrap();
    let result = Command::new(BIN)
        .args(["compare"])
        .arg(a)
        .arg(b)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let path = out.join("saccade-report.v1.json");
    (
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap(),
        path,
    )
}
fn http(port: u16, method: &str, path: &str, headers: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream,"{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n{body}",body.len()).unwrap();
    let mut s = String::new();
    stream.read_to_string(&mut s).unwrap();
    s
}
fn body(s: &str) -> Value {
    serde_json::from_str(s.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn encoder_is_deterministic_pixel_free_and_ocr_is_optional() {
    let tmp = tempfile::tempdir().unwrap();
    let (r, p) = report(tmp.path());
    let encode = || {
        report_items(
            &r,
            p.parent().unwrap(),
            JudgeQuestion::Decision(Question::Cause),
            Some("fix shadow"),
            &[],
            &EvidenceOptions::default(),
        )
        .unwrap()
        .0
    };
    let a = encode();
    let b = encode();
    assert_eq!(a[0].states, b[0].states);
    let s = &a[0].states[0];
    assert_eq!(s["flip_grid_8x8"].as_array().unwrap().len(), 8);
    assert!(
        s["flip_grid_8x8"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r.as_array().unwrap().len() == 8)
    );
    assert!(s["diagnostics"]["tone"].is_object());
    assert!(s["metadata_differences"].is_array());
    let text = s.to_string();
    for banned in [
        "pixels",
        "base64",
        "data:image",
        "estimate_ms",
        "elapsed_ms",
    ] {
        assert!(!text.contains(banned));
    }
    assert_eq!(s["intent"], "fix shadow");
    let script = tmp.path().join("ocr.py");
    std::fs::write(
        &script,
        "import sys\nprint(sys.argv[1].endswith('capture.png'))\n",
    )
    .unwrap();
    let o = ocr_diff(
        &format!("python3 {} {{image}}", script.display()),
        Path::new("baseline.png"),
        Path::new("capture.png"),
    )
    .unwrap();
    assert_eq!(o.removed, vec!["False"]);
    assert_eq!(o.added, vec!["True"]);
    assert!(
        strips(
            &judge_canary::gold_set()[0].baseline,
            &judge_canary::gold_set()[0].capture,
            &[],
            3,
            false
        )
        .is_empty()
    );
    let opts = options(None);
    let mut plan = make_plan(a.clone(), vec![], &panel(), &opts);
    let id = plan.run_id.clone();
    if let Some(PixelSource::Files(_, p)) = plan.items[0].source.as_mut() {
        let mut img = image::open(&*p).unwrap().to_rgb8();
        img.put_pixel(0, 0, image::Rgb([1, 2, 3]));
        img.save(p).unwrap();
    }
    assert_ne!(id, make_plan(plan.items, vec![], &panel(), &opts).run_id);
}

#[test]
fn colour_names_use_lab_and_describe_tint_in_words() {
    assert_eq!(colour_name([0.0; 3]), "black");
    assert_eq!(colour_name([80.0; 3]), "dark grey");
    assert_eq!(colour_name([35.0, 70.0, 220.0]), "blue");
    let s = describe_shift([80.0; 3], [20.0, 20.0, 24.0]);
    assert!(s.contains("dark grey → near-black"));
    assert!(describe_shift([80.0; 3], [80.0, 80.0, 110.0]).contains("blue"));
}

#[test]
fn aggregation_escalates_and_panel_members_remain_proposals() {
    let v = |id: &str, a: &str, p: f64| AggVote {
        judge: id.into(),
        weight: 1.0,
        answer: Some(a.into()),
        probs: BTreeMap::from([(a.into(), p)]),
    };
    let cfg = AggParams {
        min_prob: 0.6,
        min_agreement: 0.67,
        min_judges: 2,
    };
    assert_eq!(
        aggregate(&[v("a", "accept", 0.9), v("b", "accept", 0.8)], &cfg)
            .answer
            .as_deref(),
        Some("accept")
    );
    assert!(aggregate(&[v("a", "accept", 0.9), v("b", "reject", 0.9)], &cfg).escalated);
    assert!(aggregate(&[v("a", "accept", 0.2), v("b", "accept", 0.2)], &cfg).escalated);
    let tmp = tempfile::tempdir().unwrap();
    let (mut r, p) = report(tmp.path());
    let answer = saccade_core::decision::Answer {
        entry: "scene.png".into(),
        question: Question::Accept,
        hotspot: None,
        answer: "accept".into(),
        prob: Some(1.0),
        confidence: None,
        source: "human:fixture".into(),
        note: String::new(),
        request_hash: None,
    };
    let out = saccade_core::decision::propose_report(&p, &r, &answer).unwrap();
    assert!(out.proposed && !out.decided);
    let items = report_items(
        &r,
        p.parent().unwrap(),
        JudgeQuestion::Decision(Question::Accept),
        None,
        &[],
        &EvidenceOptions::default(),
    )
    .unwrap()
    .0;
    let opts = options(None);
    let panel = Panel::parse("[panel]\nmin_judges=1\nmin_agreement=0.0\nmin_prob=0.0\ncanary_rate=0\n[[judge]]\nprovider='jev'\nmodel='fixture'\n").unwrap();
    let plan = make_plan(items, Vec::new(), &panel, &opts);
    let run = execute(&plan, &panel, &mock(), &opts);
    let rows = record(&run, &p, &r, &DecisionsConfig::default());
    assert!(rows.iter().any(|row| row["source"] == "panel"));
    assert!(rows.iter().all(|row| row["decided"] == false));
    let saved =
        saccade_core::view::read_decisions(&p.parent().unwrap().join("saccade-decisions.v1.json"))
            .unwrap();
    assert!(
        saved
            .sets
            .iter()
            .all(|s| s.decision.is_none() && s.proposals.iter().all(|p| p.proposed && !p.promoted))
    );
    r.config.mode = saccade_core::report::Mode::Identity;
    let model = saccade_core::decision::Answer {
        source: "panel".into(),
        ..answer
    };
    assert!(
        saccade_core::decision::decide_report(&p, &r, &DecisionsConfig::default(), &model).is_err()
    );
}

#[test]
fn generated_canaries_flag_and_downweight_failures() {
    let gold = judge_canary::gold_set();
    assert_eq!(gold.len(), 4);
    assert_eq!(gold[0].baseline, gold[0].capture);
    let failed = judge_canary::score(
        &[(Some("accept"), "reject"), (Some("unsure"), "noise")],
        0.75,
    );
    assert!(failed.flagged);
    assert_eq!(failed.abstained, 1);
    assert!(failed.weight_factor < 1.0);
    let passed = judge_canary::score(&[(Some("reject"), "reject")], 0.75);
    assert!(!passed.flagged);
    let mut opts = options(None);
    opts.canary_rate = Some(1.0);
    let plan = make_plan(vec![item()], vec![], &panel(), &opts);
    let run = execute(&plan, &panel(), &mock(), &opts);
    assert_eq!(run.value["canaries"]["judges"][0]["flagged"], true);
    assert!(
        !run.value["canaries"]["judgements"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    check_schema("judge", &run.value);
}

#[test]
fn calibration_ece_alpha_and_human_final_labels() {
    let scored = [
        Scored {
            conf: 0.9,
            correct: true,
        },
        Scored {
            conf: 0.9,
            correct: false,
        },
        Scored {
            conf: 0.2,
            correct: false,
        },
    ];
    assert!((ece(&scored).unwrap() - 1.0 / 3.0).abs() < 1e-10);
    let labels = |rows: &[&[&str]]| {
        rows.iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        krippendorff_alpha(&labels(&[&["a", "a"], &["b", "b"]])),
        Some(1.0)
    );
    assert!(
        (krippendorff_alpha(&labels(&[&["a", "a"], &["a", "b"], &["b", "b"]])).unwrap()
            - 4.0 / 9.0)
            .abs()
            < 1e-10
    );
    assert_eq!(
        krippendorff_alpha(&labels(&[&["a", "b"], &["a", "b"]])),
        Some(-0.5)
    );
    let tmp = tempfile::tempdir().unwrap();
    let (r, p) = report(tmp.path());
    let a = |source: &str| saccade_core::decision::Answer {
        entry: "scene.png".into(),
        question: Question::Accept,
        hotspot: None,
        answer: "reject".into(),
        prob: Some(0.9),
        confidence: None,
        source: source.into(),
        note: String::new(),
        request_hash: None,
    };
    for source in ["jev", "human:alice", "human:bob"] {
        saccade_core::decision::decide_report(&p, &r, &Default::default(), &a(source)).unwrap();
    }
    let file = p.parent().unwrap().join("saccade-decisions.v1.json");
    // Read-only historical calibration fixture, not a current promotion path.
    let mut old = saccade_core::view::read_decisions(&file).unwrap();
    old.sets[0].decision = Some(saccade_core::view::Verdict::Reject);
    for p in &mut old.sets[0].proposals {
        if p.source.starts_with("human:") {
            p.proposed = false;
        }
    }
    std::fs::write(&file, serde_json::to_vec(&old).unwrap()).unwrap();
    let cal = calibrate(
        &[file],
        &[],
        &CalibrateOptions {
            target_accuracy: 0.95,
            min_support: 1,
        },
    )
    .unwrap();
    assert_eq!(cal["judges"][0]["accuracy"], 1.0);
    assert_eq!(cal["judges"][0]["suggested_gate"]["min_prob"], 0.9);
    assert_eq!(cal["human_ceiling"][0]["max_raters"], 2);
    check_schema("calibration", &cal);
}

#[test]
fn bradley_terry_orders_votes_and_bootstraps_intervals() {
    let mut votes = Vec::new();
    for _ in 0..20 {
        votes.extend([(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)]);
    }
    votes.extend([(0, 1, 0.0), (1, 2, 0.0)]);
    let r = bradley_terry(3, &votes, 100, 123);
    assert!(r.strength[0] > r.strength[1] && r.strength[1] > r.strength[2]);
    assert!(
        r.ci.iter()
            .all(|(lo, hi)| lo.is_finite() && hi.is_finite() && lo <= hi)
    );
    assert!(r.rank_ci.iter().all(|(lo, hi)| *lo >= 1 && *hi <= 3));
    assert_eq!(r.ci, bradley_terry(3, &votes, 100, 123).ci);
    assert!(
        !bradley_terry(3, &[(0, 1, 1.0)], 10, 123)
            .warnings
            .is_empty()
    );
}

#[test]
fn both_orders_detect_bias_and_escalate_flips() {
    let b = position_bias(&[PairObs {
        ab: Some("a".into()),
        ba: Some("b".into()),
        ab_slot: Some("P1".into()),
        ba_slot: Some("P1".into()),
    }]);
    assert_eq!(b.flip_rate, Some(1.0));
    assert_eq!(b.first_slot_rate, Some(1.0));
    let p = panel();
    let opts = options(None);
    let plan = make_plan(vec![item()], vec![], &p, &opts);
    let backend = Mock {
        calls: AtomicUsize::new(0),
        flip: true,
    };
    let run = execute(&plan, &p, &backend, &opts);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
    assert_eq!(run.value["items"][0]["result"]["status"], "needs_human");
    let v = selftest(
        &plan.items,
        &p,
        &mock(),
        &SelftestOptions {
            items: 1,
            offset_px: 7,
            max_calls: 5,
        },
        false,
    );
    assert_eq!(v["calls_made"], 5);
    assert_eq!(v["judges"][0]["flip_rate"]["order_swap"], 0.0);
    check_schema("judge-selftest", &v);
}

#[test]
fn keys_come_only_from_allowed_files_and_ignore_ambient_values() {
    let tmp = tempfile::tempdir().unwrap();
    let (r, p) = report(tmp.path());
    let out = Command::new(BIN)
        .arg("review")
        .arg(&p)
        .arg("--json")
        .env("JEV_API_KEY", "ambient-secret-must-be-ignored")
        .env("GEMINI_API_KEY", "ignored")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["counts"]["dispatched_calls"], 0);
    assert!(!v.to_string().contains("ambient-secret"));
    let keys = Keys::new(Some(tmp.path().into()));
    assert!(keys.load("../secret", "JEV_API_KEY").is_err());
    std::fs::write(
        tmp.path().join("jev.env"),
        "export JEV_API_KEY='file-only'\n",
    )
    .unwrap();
    let key = keys.load("jev.env", "JEV_API_KEY").unwrap();
    assert_eq!(key.expose(), "file-only");
    assert!(!format!("{key:?}").contains("file-only"));
    assert_eq!(r.entries[0].name, "scene.png");
}

#[test]
fn vote_api_round_trip_reuses_security_and_requires_both_orders() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("archive");
    std::fs::create_dir(&archive).unwrap();
    let decisions = tmp.path().join("decisions");
    let opts = options(Some(decisions.clone()));
    let p = panel();
    let plan = make_plan(vec![item()], vec![], &p, &opts);
    execute(&plan, &p, &mock(), &opts);
    let dir = vote::run_dir(&decisions, &plan.run_id);
    let run = vote::read_run(&dir).unwrap();
    check_schema("judge-votes", &serde_json::to_value(&run).unwrap());
    assert_eq!(run.items.len(), 2);
    let server = saccade_core::serve::start(saccade_core::serve::ServeOptions {
        root: archive.clone(),
        port: 0,
        cache_dir: tmp.path().join("cache"),
        decisions_dir: decisions,
        ..saccade_core::serve::ServeOptions::new(archive)
    })
    .unwrap();
    let port = server.port();
    let page = http(port, "GET", &format!("/vote/{}", plan.run_id), "", "");
    assert!(page.starts_with("HTTP/1.1 200"));
    let start = page.find("\"token\":\"").unwrap() + 9;
    let token = page[start..].split('"').next().unwrap();
    let path = format!("/api/vote/{}/vote", plan.run_id);
    let data = json!({"voter":"simulated","item":run.items[0].id,"answer":"reject"}).to_string();
    assert!(http(port, "POST", &path, "", &data).starts_with("HTTP/1.1 403"));
    let headers = format!(
        "Origin: http://127.0.0.1:{port}\r\nX-Saccade-Token: {token}\r\nContent-Type: application/json\r\n"
    );
    assert!(
        http(
            port,
            "POST",
            &path,
            &headers.replace("127.0.0.1", "evil.example"),
            &data
        )
        .starts_with("HTTP/1.1 403")
    );
    let result = http(port, "POST", &path, &headers, &data);
    assert_eq!(body(&result)["done"], 1);
    let first = execute(&plan, &p, &mock(), &opts);
    assert!(
        first.value["items"][0]["per_judge"]
            .as_array()
            .unwrap()
            .iter()
            .any(|j| j["position"] == "incomplete_orders")
    );
    let data = json!({"voter":"simulated","item":run.items[1].id,"answer":"reject"}).to_string();
    assert_eq!(body(&http(port, "POST", &path, &headers, &data))["done"], 2);
    let list = body(&http(
        port,
        "GET",
        &format!("/api/vote/{}/items?voter=simulated", plan.run_id),
        "",
        "",
    ));
    assert_eq!(list["done"], 2);
    check_schema("judge-vote-api", &list);
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["id"].as_str().unwrap().starts_with("i_"))
    );
    let result = execute(&plan, &p, &mock(), &opts);
    assert_eq!(result.value["items"][0]["result"]["answering_judges"], 2);
    assert!(!list.to_string().contains("human:simulated"));
    check_schema("judge", &result.value);
    drop(server);
}

#[test]
fn retired_judge_transport_has_no_provider_dispatch() {
    let output = Command::new(BIN)
        .args(["judge", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("deprecated command; use saccade review")
    );
}
