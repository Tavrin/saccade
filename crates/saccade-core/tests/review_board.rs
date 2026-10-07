//! Procedural codec/UI images and simulated blind votes, MIT OR Apache-2.0.
//! No external imagery, font, human study or provider result is used.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use saccade_core::{Error, review_board::*};
use serde_json::{Value, json};
use std::path::Path;

fn write(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn fixture() -> (tempfile::TempDir, Vec<Ballot>) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let image = image::RgbImage::from_fn(48, 32, |x, y| {
        image::Rgb([(x * 5) as u8, (y * 7) as u8, ((x + y) * 3) as u8])
    });
    image.save(root.join("codec-source.png")).unwrap();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 35)
        .encode_image(&image)
        .unwrap();
    std::fs::write(root.join("codec-delivery.jpg"), jpeg).unwrap();
    for (side, offset) in [("a", 0), ("b", 5)] {
        image::RgbImage::from_fn(48, 32, |x, y| {
            if (8 + offset..32 + offset).contains(&x) && (10..22).contains(&y) {
                image::Rgb([40, 90, 180])
            } else {
                image::Rgb([240, 240, 240])
            }
        })
        .save(root.join(format!("ui-{side}.png")))
        .unwrap();
    }
    let plan = Plan {
        schema: PLAN_SCHEMA.into(),
        raters: vec![
            Rater {
                id: "human-a".into(),
                kind: "human".into(),
            },
            Rater {
                id: "human-b".into(),
                kind: "human".into(),
            },
            Rater {
                id: "offline-tool".into(),
                kind: "tool".into(),
            },
        ],
        pairs: vec![
            Pair {
                id: "codec-pair".into(),
                group: "codec".into(),
                first: "codec-source.png".into(),
                second: "codec-delivery.jpg".into(),
            },
            Pair {
                id: "ui-pair".into(),
                group: "ui".into(),
                first: "ui-a.png".into(),
                second: "ui-b.png".into(),
            },
        ],
    };
    write(&root.join("plan.json"), &plan);
    prepare(&plan, &root.join("plan.json"), &root.join("trial")).unwrap();
    let trial: Value =
        serde_json::from_slice(&std::fs::read(root.join("trial/trial.json")).unwrap()).unwrap();
    // Controlled oracle votes are first/first/second on codec and second/second/missing on UI.
    let mut ballots = Vec::new();
    for r in 0..3 {
        let mut ballot: Ballot = read(
            &root.join(format!("trial/rater-{:03}/ballot.json", r + 1)),
            BALLOT_SCHEMA,
        )
        .unwrap();
        ballot.blind_confirmed = true;
        for i in 0..2 {
            let stable = match (i, r) {
                (0, 2) => Some("second"),
                (0, _) => Some("first"),
                (1, 2) => None,
                _ => Some("second"),
            };
            let reverse = trial["raters"][r]["reversed"][i].as_bool().unwrap();
            ballot.responses[i].answer = stable.map(|s| {
                match (s, reverse) {
                    ("first", true) => "second",
                    ("second", true) => "first",
                    _ => s,
                }
                .into()
            });
            ballot.responses[i].note = format!("Simulated explanation {r}/{i}");
        }
        ballots.push(ballot);
    }
    (tmp, ballots)
}
#[test]
fn three_rater_codec_ui_trial_keeps_disagreement_notes_and_missingness() {
    let (tmp, ballots) = fixture();
    let board = collect(
        &tmp.path().join("trial/trial.json"),
        &ballots,
        &tmp.path().join("board"),
    )
    .unwrap();
    assert_eq!(board["schema"], "saccade-review-board.v2");
    assert_eq!(board["approval_authority"], false);
    assert_eq!(board["verdict"], "advisory");
    assert_eq!(board["items"][0]["agreement"], "disagreement");
    assert_eq!(board["items"][0]["counts"], json!({"first":2,"second":1}));
    assert_eq!(board["items"][1]["agreement"], "unanimous");
    assert_eq!(board["items"][1]["missing"], 1);
    assert!(board["items"][1]["votes"][2]["answer"].is_null());
    assert_eq!(board["items"][1]["votes"][2]["status"], "missing");
    assert_eq!(board["raters"][2]["missing"], 1);
    assert_eq!(board["raters"][0]["peer_matches"], 2);
    assert_eq!(board["raters"][0]["peer_disagreements"], 1);
    assert_eq!(board["raters"][0]["peer_agreement"].as_f64(), Some(2. / 3.));
    assert!((board["agreement"]["value"].as_f64().unwrap() - 1. / 3.).abs() < 1e-12);
    assert_eq!(board["agreement"]["coincidence_votes"], 5);
    let html = std::fs::read_to_string(tmp.path().join("board/index.html")).unwrap();
    for text in [
        "Missing",
        "disagreement",
        "Simulated explanation",
        "Per-rater agreement",
        "human-a",
        "offline-tool",
    ] {
        assert!(html.contains(text), "{text}");
    }
    assert!(!html.contains("<button"));
    assert!(saccade_core::manifest::build(&tmp.path().join("board"), Default::default()).is_ok());
}
#[test]
fn packets_are_blind_anonymous_and_reproducible() {
    let (tmp, _) = fixture();
    for r in 1..=3 {
        let dir = tmp.path().join(format!("trial/rater-{r:03}"));
        for file in ["packet.json", "ballot.json", "index.html"] {
            let text = std::fs::read_to_string(dir.join(file)).unwrap();
            for secret in [
                "codec-source",
                "codec-delivery",
                "ui-pair",
                "codec-pair",
                "human-a",
                "human-b",
                "offline-tool",
                "reversed",
                "peer_agreement",
                "verdict",
            ] {
                // Protocol text may mention the word verdict, but never a verdict value.
                if file == "index.html" && secret == "verdict" {
                    continue;
                }
                assert!(!text.contains(secret), "{file} leaked {secret}");
            }
        }
        let ballot: Ballot = read(&dir.join("ballot.json"), BALLOT_SCHEMA).unwrap();
        assert!(!ballot.blind_confirmed);
        assert!(
            ballot
                .responses
                .iter()
                .all(|v| v.answer.is_none() && v.note.is_empty())
        );
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 7);
    }
    let plan: Plan = read(&tmp.path().join("plan.json"), PLAN_SCHEMA).unwrap();
    prepare(
        &plan,
        &tmp.path().join("plan.json"),
        &tmp.path().join("repeated"),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(tmp.path().join("trial/trial.json")).unwrap(),
        std::fs::read(tmp.path().join("repeated/trial.json")).unwrap()
    );
    assert!(
        prepare(
            &plan,
            &tmp.path().join("plan.json"),
            &tmp.path().join("trial")
        )
        .is_err()
    );
}
#[test]
fn nominal_alpha_uses_variable_count_coincidences_and_keeps_undefined_visible() {
    let rows = vec![
        vec![
            Some("first".into()),
            Some("first".into()),
            Some("second".into()),
        ],
        vec![Some("second".into()), Some("second".into()), None],
    ];
    let alpha = nominal_alpha(&rows);
    assert_eq!(alpha["observed_disagreement"].as_f64(), Some(0.4));
    assert_eq!(alpha["expected_disagreement"].as_f64(), Some(0.6));
    assert!((alpha["value"].as_f64().unwrap() - 1. / 3.).abs() < 1e-12);
    for rows in [
        vec![],
        vec![vec![Some("first".into()), None]],
        vec![vec![Some("tie".into()), Some("tie".into())]],
    ] {
        let alpha = nominal_alpha(&rows);
        assert!(alpha["value"].is_null());
        assert!(alpha["unavailable_reason"].is_string());
    }
    let unanimous = nominal_alpha(&[
        vec![Some("first".into()), Some("first".into())],
        vec![Some("second".into()), Some("second".into())],
    ]);
    assert_eq!(unanimous["value"], 1.0);
    let disagreement = nominal_alpha(&[
        vec![Some("first".into()), Some("second".into())],
        vec![Some("tie".into()), Some("unsure".into())],
    ]);
    assert_eq!(disagreement["value"], 0.0);
}
#[test]
fn missing_ballot_omitted_response_unsure_and_notes_are_distinct() {
    let (tmp, mut ballots) = fixture();
    ballots.pop();
    ballots[0].responses[0].answer = Some("unsure".into());
    ballots[0].responses[0].note = "<script>alert('text')</script>".into();
    ballots[1].responses.pop();
    let board = collect(
        &tmp.path().join("trial/trial.json"),
        &ballots,
        &tmp.path().join("board"),
    )
    .unwrap();
    assert_eq!(board["items"][0]["counts"]["unsure"], 1);
    assert_eq!(board["items"][1]["missing"], 2);
    assert_eq!(board["items"][1]["agreement"], "insufficient");
    assert_eq!(board["raters"][2]["ballot_received"], false);
    assert!(board["raters"][2]["peer_agreement"].is_null());
    let html = std::fs::read_to_string(tmp.path().join("board/index.html")).unwrap();
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
    let empty = collect(
        &tmp.path().join("trial/trial.json"),
        &[],
        &tmp.path().join("empty"),
    )
    .unwrap();
    assert_eq!(empty["items"][0]["missing"], 3);
    assert!(empty["agreement"]["value"].is_null());
}
#[test]
fn collection_rejects_exposure_duplicates_foreign_or_stale_ballots_and_packet_mutations() {
    let (tmp, ballots) = fixture();
    let trial = tmp.path().join("trial/trial.json");
    let out = tmp.path().join("rejected");
    let mut invalid = ballots.clone();
    invalid[0].blind_confirmed = false;
    assert!(matches!(
        collect(&trial, &invalid, &out),
        Err(Error::ReviewBoard {
            code: "board_blind_protocol",
            ..
        })
    ));
    let mut invalid = ballots.clone();
    invalid.push(ballots[0].clone());
    assert!(collect(&trial, &invalid, &out).is_err());
    let mut invalid = ballots.clone();
    let repeated = invalid[0].responses[0].clone();
    invalid[0].responses.push(repeated);
    assert!(collect(&trial, &invalid, &out).is_err());
    for field in ["answer", "hash", "id", "rater", "trial"] {
        let mut invalid = ballots.clone();
        match field {
            "answer" => invalid[0].responses[0].answer = Some("approve".into()),
            "hash" => invalid[0].responses[0].evidence_hash = "stale".into(),
            "id" => invalid[0].responses[0].id = "foreign".into(),
            "rater" => invalid[0].rater_key = "unknown".into(),
            _ => invalid[0].trial_id = "foreign".into(),
        }
        assert!(collect(&trial, &invalid, &out).is_err(), "{field}");
    }
    assert!(!out.exists());
    assert!(collect(&trial, &ballots, &tmp.path().join("trial/rater-001/board")).is_err());
    let image_path = tmp.path().join("trial/rater-001/pair-001-1.png");
    let original = std::fs::read(&image_path).unwrap();
    std::fs::write(&image_path, b"changed").unwrap();
    assert!(matches!(
        collect(&trial, &ballots, &out),
        Err(Error::ReviewBoard {
            code: "board_evidence_changed",
            ..
        })
    ));
    std::fs::write(image_path, original).unwrap();
    std::fs::write(
        tmp.path().join("trial/rater-001/index.html"),
        b"peer answers",
    )
    .unwrap();
    assert!(collect(&trial, &ballots, &out).is_err());
    assert!(!out.exists());
}
#[test]
fn strict_bounded_versioned_input_and_invalid_plan_fail_before_publication() {
    let (tmp, _) = fixture();
    let path = tmp.path().join("bad.json");
    write(&path, &json!({"schema":"saccade-review-board-plan.v99"}));
    assert!(matches!(
        read::<Plan>(&path, PLAN_SCHEMA),
        Err(Error::VersionSkew { .. })
    ));
    let mut plan: Value =
        serde_json::from_slice(&std::fs::read(tmp.path().join("plan.json")).unwrap()).unwrap();
    plan["verdict"] = json!("pass");
    write(&path, &plan);
    assert!(read::<Plan>(&path, PLAN_SCHEMA).is_err());
    std::fs::write(&path, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    assert!(read::<Plan>(&path, PLAN_SCHEMA).is_err());
    let mut plan: Plan = read(&tmp.path().join("plan.json"), PLAN_SCHEMA).unwrap();
    plan.raters[1] = plan.raters[0].clone();
    assert!(prepare(&plan, &path, &tmp.path().join("bad-trial")).is_err());
    assert!(!tmp.path().join("bad-trial").exists());
}
