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

const BASE: &str = "https://github.com/Tavrin/saccade/crates/saccade-core/schemas";

fn generated<T: schemars::JsonSchema>(file: &str) -> String {
    let id = file.trim_end_matches(".schema.json");
    let target = saccade_core::report_links::linked_schema(id);
    let target_file = format!("{target}.schema.json");
    let schema = schemars::schema_for!(T);
    let mut value = serde_json::to_value(&schema).expect("schema serializes");

    if file == "saccade-perf.v2.schema.json" {
        value["$defs"]["CapturePerf"]["properties"]["schema"]["const"] = "saccade-perf.v2".into();
        value["$defs"]["CapturePerf"]["properties"]["kind"]["const"] = "measurement".into();
        value["$defs"]["CapturePerf"]["required"]
            .as_array_mut()
            .expect("required fields")
            .push("kind".into());
        value["$defs"]["PerformanceNoiseRecord"]["properties"]["schema"]["const"] =
            "saccade-perf.v2".into();
        value["$defs"]["PerformanceNoiseRecord"]["properties"]["kind"]["const"] =
            "performance_noise".into();
        value["$defs"]["PerformanceNoiseRecord"]["properties"]["unit"]["const"] = "ms".into();
    }
    if file == "saccade-onset.v1.schema.json" {
        value["properties"]["schema"]["const"] = "saccade-onset.v1".into();
        value["properties"]["operation"]["const"] = "onset".into();
    }
    let obj = value.as_object_mut().expect("schema is an object");
    // `$id` right after `$schema`, as in the hand-written files.
    let mut ordered = serde_json::Map::new();
    if let Some(s) = obj.remove("$schema") {
        ordered.insert("$schema".into(), s);
    }
    ordered.insert("$id".into(), format!("{BASE}/{target_file}").into());
    ordered.extend(std::mem::take(obj));
    // Optional C2PA enables serde_json/preserve_order. Schema bytes must remain
    // independent of that feature while retaining every semantic field.
    let mut canonical = serde_json::Value::Object(ordered);
    saccade_core::report_links::extend_schema(&mut canonical, target);
    canonical.sort_all_objects();
    let mut text = serde_json::to_string_pretty(&canonical).expect("schema serializes");
    text.push('\n');
    text
}

#[test]
fn committed_schemas_match_the_rust_types() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    let update = std::env::var_os("UPDATE_SCHEMAS").is_some();
    let all = [
        (
            "saccade-replay-recipe.v1.schema.json",
            generated::<saccade_core::replay::Recipe>("saccade-replay-recipe.v1.schema.json"),
        ),
        (
            "saccade-replay-pack.v1.schema.json",
            generated::<saccade_core::replay::Pack>("saccade-replay-pack.v1.schema.json"),
        ),
        (
            "saccade-replay-result.v1.schema.json",
            generated::<saccade_core::replay::Receipt>("saccade-replay-result.v1.schema.json"),
        ),
        (
            "saccade-tofu.v1.schema.json",
            generated::<saccade_core::text_quality::TofuReport>("saccade-tofu.v1.schema.json"),
        ),
        (
            "saccade-text-legibility.v1.schema.json",
            generated::<saccade_core::text_quality::LegibilityReport>(
                "saccade-text-legibility.v1.schema.json",
            ),
        ),
        (
            "saccade-asset-views.v1.schema.json",
            generated::<saccade_core::asset_views::Manifest>("saccade-asset-views.v1.schema.json"),
        ),
        (
            "saccade-asset-view-report.v1.schema.json",
            generated::<saccade_core::asset_views::Report>(
                "saccade-asset-view-report.v1.schema.json",
            ),
        ),
        (
            "saccade-motion-review.v1.schema.json",
            generated::<saccade_core::dense_motion::Report>("saccade-motion-review.v1.schema.json"),
        ),
        (
            "saccade-motion-vectors.v1.schema.json",
            generated::<saccade_core::dense_motion::Sidecar>(
                "saccade-motion-vectors.v1.schema.json",
            ),
        ),
        (
            "saccade-vector-buffer.v1.schema.json",
            generated::<saccade_core::dense_motion::Buffer>("saccade-vector-buffer.v1.schema.json"),
        ),
        (
            "saccade-perf-plan.v1.schema.json",
            generated::<saccade_core::paired_stats::Plan>("saccade-perf-plan.v1.schema.json"),
        ),
        (
            "saccade-perf-pairs.v1.schema.json",
            generated::<saccade_core::paired_stats::Samples>("saccade-perf-pairs.v1.schema.json"),
        ),
        (
            "saccade-tesseract.v1.schema.json",
            generated::<saccade_core::ui_review::OcrContract>("saccade-tesseract.v1.schema.json"),
        ),
        (
            "saccade-ui-source.v1.schema.json",
            generated::<saccade_core::ui_review::Source>("saccade-ui-source.v1.schema.json"),
        ),
        (
            "saccade-ui-review.v1.schema.json",
            generated::<saccade_core::ui_review::Report>("saccade-ui-review.v1.schema.json"),
        ),
        (
            "saccade-brand-source.v1.schema.json",
            generated::<saccade_core::brand::Evidence>("saccade-brand-source.v1.schema.json"),
        ),
        (
            "saccade-brand-review.v1.schema.json",
            generated::<saccade_core::brand::Report>("saccade-brand-review.v1.schema.json"),
        ),
        (
            "saccade-renderdoc-extract.v1.schema.json",
            generated::<saccade_core::renderdoc::Capture>(
                "saccade-renderdoc-extract.v1.schema.json",
            ),
        ),
        (
            "saccade-renderdoc-localization.v1.schema.json",
            generated::<saccade_core::renderdoc::Localization>(
                "saccade-renderdoc-localization.v1.schema.json",
            ),
        ),
        (
            "saccade-region-models.v1.schema.json",
            generated::<saccade_core::semantic::ModelManifest>(
                "saccade-region-models.v1.schema.json",
            ),
        ),
        (
            "saccade-grounded.v1.schema.json",
            generated::<saccade_core::grounded::Explanation>("saccade-grounded.v1.schema.json"),
        ),
        (
            "saccade-frozen-region.v1.schema.json",
            generated::<saccade_core::localized::FrozenRegion>(
                "saccade-frozen-region.v1.schema.json",
            ),
        ),
        (
            "saccade-localized.v1.schema.json",
            generated::<saccade_core::localized::Measurement>("saccade-localized.v1.schema.json"),
        ),
        (
            "saccade-dom-regions.v1.schema.json",
            generated::<saccade_core::localized::DomMetadata>("saccade-dom-regions.v1.schema.json"),
        ),
        (
            "saccade-inventory.v1.schema.json",
            generated::<saccade_core::inventory::Manifest>("saccade-inventory.v1.schema.json"),
        ),
        (
            "saccade-inventory-report.v1.schema.json",
            generated::<saccade_core::inventory::Inventory>(
                "saccade-inventory-report.v1.schema.json",
            ),
        ),
        #[cfg(feature = "compression")]
        (
            "saccade-quality-sweep.v1.schema.json",
            generated::<saccade_core::quality::Manifest>("saccade-quality-sweep.v1.schema.json"),
        ),
        #[cfg(feature = "compression")]
        (
            "saccade-quality-report.v1.schema.json",
            generated::<saccade_core::quality::Sweep>("saccade-quality-report.v1.schema.json"),
        ),
        (
            "saccade-onset.v1.schema.json",
            generated::<saccade_core::onset::Document>("saccade-onset.v1.schema.json"),
        ),
        #[cfg(feature = "geometry")]
        (
            "saccade-geometry.v1.schema.json",
            generated::<saccade_core::geometry::Document>("saccade-geometry.v1.schema.json"),
        ),
        (
            "saccade-evidence.v1.schema.json",
            generated::<saccade_core::evidence::Document>("saccade-evidence.v1.schema.json"),
        ),
        (
            "saccade-result.v2.schema.json",
            generated::<saccade_core::evidence::action::ResultEnvelope>(
                "saccade-result.v2.schema.json",
            ),
        ),
        (
            "saccade-labels.v2.schema.json",
            generated::<saccade_core::evidence::human::Labels>("saccade-labels.v2.schema.json"),
        ),
        (
            "saccade-labels.v1.schema.json",
            generated::<saccade_core::labels::Labels>("saccade-labels.v1.schema.json"),
        ),
        (
            "saccade-perf.v1.schema.json",
            generated::<saccade_core::perf::LegacyCapturePerf>("saccade-perf.v1.schema.json"),
        ),
        (
            "saccade-perf.v2.schema.json",
            generated::<saccade_core::perf::PerfDocument>("saccade-perf.v2.schema.json"),
        ),
        (
            "saccade-perf-diff.v1.schema.json",
            generated::<saccade_core::perf::PerfDiff>("saccade-perf-diff.v1.schema.json"),
        ),
        (
            "saccade-gpu-clock.v1.schema.json",
            generated::<saccade_core::gpu_clock::GpuClock>("saccade-gpu-clock.v1.schema.json"),
        ),
        (
            "saccade-ablate.v1.schema.json",
            generated::<saccade_core::ablate::Ablation>("saccade-ablate.v1.schema.json"),
        ),
        (
            "saccade-noise.v1.schema.json",
            generated::<saccade_core::ergonomics::NoiseReport>("saccade-noise.v1.schema.json"),
        ),
        (
            "saccade-entries.v1.schema.json",
            generated::<saccade_core::ergonomics::EntriesPage>("saccade-entries.v1.schema.json"),
        ),
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
        let target =
            saccade_core::report_links::linked_schema(file.trim_end_matches(".schema.json"));
        let path = dir.join(format!("{target}.schema.json"));
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
        "onset",
        "review",
        "labels",
        "judge-bench",
        "noise",
        "entries",
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
        "manifest",
        "link",
        "region-export",
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

#[test]
#[cfg(feature = "compression")]
fn quality_report_links_are_optional_and_both_shapes_remain_readable() {
    let legacy = serde_json::json!({
        "schema": "saccade-quality-report.v1",
        "manifest": {
            "schema": "saccade-quality-sweep.v1",
            "original": {"path": "original.png", "sha256": "0".repeat(64)},
            "reference": {"path": "reference.png", "sha256": "0".repeat(64)},
            "minimum_score": 90.0,
            "maximum_bytes": 1000,
            "viewing_conditions": "synthetic compatibility fixture",
            "candidates": []
        },
        "manifest_sha256": "0".repeat(64),
        "original_bytes": 1000,
        "metric": "fixture",
        "candidates": [],
        "selected_candidate": null,
        "lowest_quality_candidate": null,
        "coverage": "incomplete",
        "human_visual_review": "pending"
    });
    let linked = saccade_core::report_links::decorate(&legacy).expect("decorate quality report");
    assert!(
        linked["report_id"]
            .as_str()
            .expect("report ID")
            .starts_with("sha256:")
    );
    assert!(linked["source_refs"].is_array());
    for value in [legacy.clone(), linked] {
        let report: saccade_core::quality::Sweep =
            serde_json::from_value(value).expect("read linked or unlinked report");
        assert_eq!(
            serde_json::to_value(report).expect("serialize report"),
            legacy
        );
    }
    let schema: serde_json::Value = serde_json::from_str(
        &generated::<saccade_core::quality::Sweep>("saccade-quality-report.v1.schema.json"),
    )
    .expect("quality schema");
    for field in ["report_id", "source_refs"] {
        assert!(schema["properties"].get(field).is_some());
        assert!(
            !schema["required"]
                .as_array()
                .expect("required fields")
                .iter()
                .any(|v| v == field)
        );
    }
}

#[test]
fn linked_contracts_have_distinct_versions_and_legacy_readers_remain_strict() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
    for (old, new) in saccade_core::report_links::SCHEMA_MIGRATIONS {
        let legacy: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{old}.schema.json"))).expect("legacy schema"),
        )
        .expect("legacy JSON");
        assert!(
            legacy["properties"].get("report_id").is_none(),
            "{old}: legacy strict contract changed"
        );
        if matches!(
            *old,
            "saccade-report.v1"
                | "saccade-result.v2"
                | "saccade-grounded.v1"
                | "saccade-localized.v1"
                | "saccade-onset.v1"
                | "saccade-ui-review.v1"
        ) {
            continue;
        }
        let mut linked = legacy.clone();
        linked["$id"] = format!("{BASE}/{new}.schema.json").into();
        saccade_core::report_links::extend_schema(&mut linked, new);
        linked.sort_all_objects();
        let text = format!(
            "{}\n",
            serde_json::to_string_pretty(&linked).expect("linked JSON")
        );
        let path = dir.join(format!("{new}.schema.json"));
        if std::env::var_os("UPDATE_SCHEMAS").is_some() {
            std::fs::write(&path, &text).expect("linked schema");
        }
        assert_eq!(
            std::fs::read_to_string(path).expect("committed linked schema"),
            text,
            "{new}"
        );
        assert_eq!(linked["properties"]["schema"]["const"], *new);
        assert_ne!(old, new);
    }
}
