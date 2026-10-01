//! The JSON Schemas under `schemas/`, one per emitted schema id. Those of the
//! types in this crate are generated from them, and this test fails when a
//! committed file drifts; regenerate with
//! `UPDATE_SCHEMAS=1 cargo test -p saccade-core --test schemas`. The others
//! (the CLI's own output objects) are written by hand: this test checks that
//! each exists, parses and carries the right `$id`, and the `saccade` crate's
//! tests validate real outputs against them.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::path::PathBuf;

use saccade_core::explain::{ExplainBlindKey, ExplainPack};
use saccade_core::report::Report;
use saccade_core::view::{BlindKey, Decisions};

const BASE: &str = "https://github.com/Tavrin/saccade/schemas";

fn generated<T: schemars::JsonSchema>(file: &str) -> String {
    let schema = schemars::schema_for!(T);
    let mut value = serde_json::to_value(&schema).expect("schema serializes");
    let obj = value.as_object_mut().expect("schema is an object");
    // `$id` right after `$schema`, as in the hand-written files.
    let mut ordered = serde_json::Map::new();
    if let Some(s) = obj.remove("$schema") {
        ordered.insert("$schema".into(), s);
    }
    ordered.insert("$id".into(), format!("{BASE}/{file}").into());
    ordered.extend(std::mem::take(obj));
    let mut text = serde_json::to_string_pretty(&ordered).expect("schema serializes");
    text.push('\n');
    text
}

#[test]
fn committed_schemas_match_the_rust_types() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let update = std::env::var_os("UPDATE_SCHEMAS").is_some();
    let all = [
        (
            "saccade-judge-votes.v1.schema.json",
            generated::<saccade_core::judge_vote::VoteRun>("saccade-judge-votes.v1.schema.json"),
        ),
        (
            "saccade-bisect.v1.schema.json",
            generated::<saccade_core::bisect::BisectResult>("saccade-bisect.v1.schema.json"),
        ),
        (
            "saccade-inbox-item.v1.schema.json",
            generated::<saccade_core::inbox::Item>("saccade-inbox-item.v1.schema.json"),
        ),
        (
            "saccade-ask-result.v1.schema.json",
            generated::<saccade_core::inbox::AskResult>("saccade-ask-result.v1.schema.json"),
        ),
        (
            "saccade-sequence.v1.schema.json",
            generated::<saccade_core::sequence::SequenceReport>("saccade-sequence.v1.schema.json"),
        ),
        (
            "saccade-rank.v1.schema.json",
            generated::<saccade_core::rank::RankReport>("saccade-rank.v1.schema.json"),
        ),
        (
            "saccade-report.v1.schema.json",
            generated::<Report>("saccade-report.v1.schema.json"),
        ),
        (
            "saccade-decisions.v1.schema.json",
            generated::<Decisions>("saccade-decisions.v1.schema.json"),
        ),
        (
            "saccade-blind-key.v1.schema.json",
            generated::<BlindKey>("saccade-blind-key.v1.schema.json"),
        ),
        (
            "saccade-explain.v1.schema.json",
            generated::<ExplainPack>("saccade-explain.v1.schema.json"),
        ),
        (
            "saccade-explain-blind-key.v1.schema.json",
            generated::<ExplainBlindKey>("saccade-explain-blind-key.v1.schema.json"),
        ),
    ];
    for (file, text) in all {
        let path = dir.join(file);
        if update {
            std::fs::create_dir_all(&dir).expect("mkdir schemas");
            std::fs::write(&path, &text).expect("write schema");
            continue;
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run with UPDATE_SCHEMAS=1", path.display()));
        assert!(
            committed == text,
            "{file} drifted from the Rust types; run `UPDATE_SCHEMAS=1 cargo test -p saccade-core --test schemas`"
        );
    }

    // Every schema id the tools emit has a file with a matching `$id`.
    for name in [
        "judge",
        "calibration",
        "judge-selftest",
        "judge-votes",
        "judge-vote-api",
        "bisect",
        "inbox-item",
        "ask-result",
        "sequence",
        "rank",
        "report",
        "decisions",
        "blind-key",
        "explain",
        "explain-blind-key",
        "view-summary",
        "summary",
        "approve",
        "result",
        "explain-result",
        "decision-request",
        "decide-result",
        "error",
    ] {
        let file = format!("saccade-{name}.v1.schema.json");
        let text =
            std::fs::read_to_string(dir.join(&file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let v: serde_json::Value = serde_json::from_str(&text).expect("schema parses");
        assert_eq!(
            v["$id"].as_str(),
            Some(format!("{BASE}/{file}").as_str()),
            "{file}: wrong $id"
        );
    }
}
