//! The JSON Schemas under `schemas/` are generated from the Rust types. This
//! test fails when a committed schema drifts from them; regenerate with
//! `UPDATE_SCHEMAS=1 cargo test -p flipdiff-core --test schemas`.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::path::PathBuf;

use flipdiff_core::explain::ExplainPack;
use flipdiff_core::report::Report;
use flipdiff_core::view::Decisions;

fn generated<T: schemars::JsonSchema>() -> String {
    let schema = schemars::schema_for!(T);
    let mut text = serde_json::to_string_pretty(&schema).expect("schema serializes");
    text.push('\n');
    text
}

#[test]
fn committed_schemas_match_the_rust_types() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let update = std::env::var_os("UPDATE_SCHEMAS").is_some();
    let all = [
        ("flipdiff-report.v1.schema.json", generated::<Report>()),
        (
            "flipdiff-decisions.v1.schema.json",
            generated::<Decisions>(),
        ),
        ("explain.v1.schema.json", generated::<ExplainPack>()),
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
            "{file} drifted from the Rust types; run `UPDATE_SCHEMAS=1 cargo test -p flipdiff-core --test schemas`"
        );
    }
}
