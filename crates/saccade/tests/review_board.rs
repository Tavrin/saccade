//! CLI/schema/manifest proof using generated MIT OR Apache-2.0 images and votes.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn value(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)))
}
fn schema(value: &Value) {
    let id = value["schema"].as_str().unwrap();
    let schema: Value =
        serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{id}: {errors:?}");
}
fn load(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
fn offline_three_rater_trial_emits_bound_schemas_and_a_manifest_without_approval() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let pixels = image::RgbImage::from_fn(32, 24, |x, y| {
        image::Rgb([(x * 7) as u8, (y * 9) as u8, 100])
    });
    pixels.save(root.join("source.png")).unwrap();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 30)
        .encode_image(&pixels)
        .unwrap();
    std::fs::write(root.join("delivery.jpg"), jpeg).unwrap();
    for (name, offset) in [("layout-a.png", 0), ("layout-b.png", 3)] {
        image::RgbImage::from_fn(32, 24, |x, y| {
            if (5 + offset..20 + offset).contains(&x) && (6..18).contains(&y) {
                image::Rgb([40, 60, 160])
            } else {
                image::Rgb([240, 240, 240])
            }
        })
        .save(root.join(name))
        .unwrap();
    }
    let plan = json!({"schema":"saccade-review-board-plan.v1","raters":[{"id":"a","kind":"human"},{"id":"b","kind":"human"},{"id":"c","kind":"tool"}],"pairs":[{"id":"codec","group":"codec","first":"source.png","second":"delivery.jpg"},{"id":"ui","group":"ui","first":"layout-a.png","second":"layout-b.png"}]});
    schema(&plan);
    std::fs::write(root.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
    let output = run(
        root,
        &[
            "review",
            "board",
            "prepare",
            "plan.json",
            "--out",
            "trial",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    schema(&value(&output));
    let trial = load(root.join("trial/trial.json"));
    schema(&trial);
    for r in 0..3 {
        let dir = root.join(format!("trial/rater-{:03}", r + 1));
        schema(&load(dir.join("packet.json")));
        let mut ballot = load(dir.join("ballot.json"));
        schema(&ballot);
        ballot["blind_confirmed"] = json!(true);
        for i in 0..2 {
            let stable = if i == 0 || r == 0 { "first" } else { "second" };
            let reversed = trial["raters"][r]["reversed"][i].as_bool().unwrap();
            ballot["responses"][i]["answer"] = if (i, r) == (1, 2) {
                Value::Null
            } else {
                json!(if reversed {
                    if stable == "first" { "second" } else { "first" }
                } else {
                    stable
                })
            };
            ballot["responses"][i]["note"] = json!("Procedural fixture vote");
        }
        std::fs::write(
            root.join(format!("returned-{r}.json")),
            serde_json::to_vec(&ballot).unwrap(),
        )
        .unwrap();
    }
    let output = run(
        root,
        &[
            "review",
            "board",
            "collect",
            "trial/trial.json",
            "--ballot",
            "returned-0.json",
            "--ballot",
            "returned-1.json",
            "--ballot",
            "returned-2.json",
            "--out",
            "board",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    schema(&value(&output));
    let board = load(root.join("board/saccade-review-board.v2.json"));
    schema(&board);
    assert_eq!(board["items"][0]["agreement"], "unanimous");
    assert_eq!(board["items"][1]["agreement"], "disagreement");
    assert_eq!(board["items"][1]["missing"], 1);
    assert_eq!(board["verdict"], "advisory");
    assert_eq!(board["approval_authority"], false);
    assert_eq!(value(&output)["data"]["report_id"], board["report_id"]);
    let index = load_index(&root.join("board/reports/index.jsonl"));
    assert_eq!(index["report_id"], board["report_id"]);
    assert!(Path::new(index["report_path"].as_str().unwrap()).is_file());
    let unlinked = saccade_core::report_links::legacy_view(&board);
    schema(&unlinked);
    let output = run(root, &["manifest", "build", "board", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let manifest = load(root.join("board/saccade-manifest.json"));
    assert!(
        serde_json::to_string(&manifest)
            .unwrap()
            .contains(board["report_id"].as_str().unwrap())
    );
    assert!(manifest["approval"]["state"] != "approved");
    assert_eq!(
        run(root, &["manifest", "verify", "board", "--json"])
            .status
            .code(),
        Some(0)
    );
    let output = run(
        root,
        &[
            "approve",
            "--report",
            "board/saccade-review-board.v2.json",
            "--entry",
            "codec",
            "--dry-run",
            "--out",
            "forbidden-plan",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!root.join("forbidden-plan").exists());
    // A protocol error is typed and cannot partially publish a board.
    let mut ballot = load(root.join("returned-0.json"));
    ballot["blind_confirmed"] = json!(false);
    std::fs::write(
        root.join("exposed.json"),
        serde_json::to_vec(&ballot).unwrap(),
    )
    .unwrap();
    let output = run(
        root,
        &[
            "review",
            "board",
            "collect",
            "trial/trial.json",
            "--ballot",
            "exposed.json",
            "--out",
            "rejected",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stdout).contains("board_blind_protocol"));
    assert!(!root.join("rejected").exists());
}
#[test]
fn newer_inputs_and_missing_files_return_typed_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("future.json"),
        br#"{"schema":"saccade-review-board-plan.v99"}"#,
    )
    .unwrap();
    let output = run(
        root,
        &[
            "review",
            "board",
            "prepare",
            "future.json",
            "--out",
            "trial",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stdout).contains("version_skew"));
    let output = run(
        root,
        &[
            "review",
            "board",
            "prepare",
            "missing.json",
            "--out",
            "trial",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!root.join("trial").exists());
}

fn load_index(path: &Path) -> Value {
    serde_json::from_str(
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}
