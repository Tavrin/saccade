//! Type-derived wave 7 schemas under the existing core schemas directory.
use serde_json::Value;
fn schema<T: schemars::JsonSchema>(id: &str) -> Value {
    let mut v = schemars::schema_for!(T).to_value();
    v["$id"] =
        format!("https://github.com/Tavrin/saccade/crates/saccade-core/schemas/{id}.schema.json")
            .into();
    v["properties"]["schema"]["const"] = id.into();
    v
}
/// Derive all standalone persisted wave 7 schemas, with exact discriminators.
pub fn documents() -> Vec<(&'static str, Value)> {
    vec![
        (
            super::models::REGISTRY_SCHEMA,
            schema::<super::models::Registry>(super::models::REGISTRY_SCHEMA),
        ),
        (
            super::vision::LOCATE_SCHEMA,
            schema::<super::vision::LocateReport>(super::vision::LOCATE_SCHEMA),
        ),
        (
            super::quality::QUALITY_SCHEMA,
            schema::<super::quality::QualityReport>(super::quality::QUALITY_SCHEMA),
        ),
        (
            super::watermark::WATERMARK_SCHEMA,
            schema::<super::watermark::WatermarkReport>(super::watermark::WATERMARK_SCHEMA),
        ),
        (
            super::faces::FACES_SCHEMA,
            schema::<super::faces::FaceReport>(super::faces::FACES_SCHEMA),
        ),
        (
            super::faces::CROP_SCHEMA,
            schema::<super::faces::CropReport>(super::faces::CROP_SCHEMA),
        ),
        (
            super::observation::OBSERVATION_SCHEMA,
            schema::<super::observation::ObservationReport>(super::observation::OBSERVATION_SCHEMA),
        ),
    ]
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn committed_wave7_schemas_match_rust_types() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas");
        for (id, value) in super::documents() {
            let path = dir.join(format!("{id}.schema.json"));
            let text = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
            if std::env::var_os("UPDATE_WAVE7_SCHEMAS").is_some() {
                std::fs::write(&path, &text).unwrap();
            }
            assert_eq!(std::fs::read_to_string(path).unwrap(), text, "{id}");
        }
    }
}
