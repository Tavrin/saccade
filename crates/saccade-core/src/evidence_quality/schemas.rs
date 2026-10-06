//! Type-derived schemas for persisted rendering evidence.
use serde_json::Value;
fn schema<T: schemars::JsonSchema>(id: &str) -> Value {
    let mut v = schemars::schema_for!(T).to_value();
    v["$id"] =
        format!("https://github.com/Tavrin/saccade/crates/saccade-core/schemas/{id}.schema.json")
            .into();
    v["properties"]["schema"]["const"] = id.into();
    v.sort_all_objects();
    v
}
/// Standalone evidence and input schema documents with exact discriminators.
pub fn documents() -> Vec<(&'static str, Value)> {
    vec![
        (
            crate::arms::FINGERPRINT_SCHEMA,
            schema::<crate::arms::Fingerprint>(crate::arms::FINGERPRINT_SCHEMA),
        ),
        (
            crate::arms::RESULT_SCHEMA,
            schema::<crate::arms::Check>(crate::arms::RESULT_SCHEMA),
        ),
        (
            super::effect::SCHEMA,
            schema::<super::effect::EffectResult>(super::effect::SCHEMA),
        ),
        (
            super::spatial::SCHEMA,
            schema::<super::spatial::SpatialReport>(super::spatial::SCHEMA),
        ),
        (
            super::layers::SCHEMA,
            schema::<super::layers::Manifest>(super::layers::SCHEMA),
        ),
        (
            super::temporal::SCHEMA,
            schema::<super::temporal::Report>(super::temporal::SCHEMA),
        ),
        (
            super::reference::SCHEMA,
            schema::<super::reference::Report>(super::reference::SCHEMA),
        ),
        (
            super::trial::PLAN_SCHEMA,
            schema::<super::trial::Plan>(super::trial::PLAN_SCHEMA),
        ),
        (
            super::trial::FROZEN_SCHEMA,
            schema::<super::trial::Frozen>(super::trial::FROZEN_SCHEMA),
        ),
        (
            super::trial::RECEIPT_SCHEMA,
            schema::<super::trial::Receipt>(super::trial::RECEIPT_SCHEMA),
        ),
    ]
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    fn inherited<T: schemars::JsonSchema>(file: &str) -> String {
        let mut v = schemars::schema_for!(T).to_value();
        if file == "saccade-perf.v2.schema.json" {
            v["$defs"]["CapturePerf"]["properties"]["schema"]["const"] = "saccade-perf.v2".into();
            v["$defs"]["CapturePerf"]["properties"]["kind"]["const"] = "measurement".into();
            v["$defs"]["CapturePerf"]["required"]
                .as_array_mut()
                .expect("required")
                .push("kind".into());
            v["$defs"]["PerformanceNoiseRecord"]["properties"]["schema"]["const"] =
                "saccade-perf.v2".into();
            v["$defs"]["PerformanceNoiseRecord"]["properties"]["kind"]["const"] =
                "performance_noise".into();
            v["$defs"]["PerformanceNoiseRecord"]["properties"]["unit"]["const"] = "ms".into();
        }
        v["$id"] =
            format!("https://github.com/Tavrin/saccade/crates/saccade-core/schemas/{file}").into();
        v.sort_all_objects();
        format!("{}\n", serde_json::to_string_pretty(&v).expect("schema"))
    }
    #[test]
    fn persisted_evidence_and_additive_contract_schemas_match() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
        let mut docs: Vec<_> = super::documents()
            .into_iter()
            .map(|(id, v)| {
                (
                    format!("{id}.schema.json"),
                    format!("{}\n", serde_json::to_string_pretty(&v).expect("schema")),
                )
            })
            .collect();
        // Existing contract schemas are derived identically to tests/schemas.rs.
        docs.extend([
            (
                "saccade-report.v1.schema.json".into(),
                inherited::<crate::report::Report>("saccade-report.v1.schema.json"),
            ),
            (
                "saccade-entries.v1.schema.json".into(),
                inherited::<crate::ergonomics::EntriesPage>("saccade-entries.v1.schema.json"),
            ),
            (
                "saccade-perf.v2.schema.json".into(),
                inherited::<crate::perf::PerfDocument>("saccade-perf.v2.schema.json"),
            ),
            (
                "saccade-perf-diff.v1.schema.json".into(),
                inherited::<crate::perf::PerfDiff>("saccade-perf-diff.v1.schema.json"),
            ),
        ]);
        #[cfg(feature = "graphics")]
        docs.extend([
            (
                "saccade-ablate.v1.schema.json".into(),
                inherited::<crate::ablate::Ablation>("saccade-ablate.v1.schema.json"),
            ),
            (
                "saccade-sequence.v1.schema.json".into(),
                inherited::<crate::sequence::SequenceReport>("saccade-sequence.v1.schema.json"),
            ),
        ]);
        for (file, text) in docs {
            let p = dir.join(&file);
            if std::env::var_os("UPDATE_WAVE9_SCHEMAS").is_some() {
                std::fs::write(&p, &text).expect("write schema");
            }
            assert_eq!(
                std::fs::read_to_string(&p).expect("read schema"),
                text,
                "{file}"
            );
        }
    }
}
