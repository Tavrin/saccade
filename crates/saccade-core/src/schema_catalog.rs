//! Source-independent discovery of the JSON Schemas shipped with this build.
/// All shipped schema IDs and their exact JSON bytes, sorted by ID.
pub const DOCUMENTS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/schema_catalog.rs"));
/// Retrieve a schema by its discriminator, without a source checkout or filesystem lookup.
pub fn get(id: &str) -> crate::Result<&'static str> {
    DOCUMENTS
        .binary_search_by_key(&id, |(id, _)| *id)
        .map(|i| DOCUMENTS[i].1)
        .map_err(|_| {
            crate::Error::Config(format!("unknown schema ID {id:?}; use saccade schema list"))
        })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn all_code_schema_ids_are_embedded() {
        for file in
            walkdir::WalkDir::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.path().extension().is_some_and(|s| s == "rs")
                        && !e.path().components().any(|c| c.as_os_str() == "tests")
                })
        {
            let text = std::fs::read_to_string(file.path()).unwrap();
            for literal in text
                .split("#[cfg(test)]")
                .next()
                .unwrap()
                .split('"')
                .skip(1)
                .step_by(2)
            {
                if literal.starts_with("saccade-")
                    && literal.rsplit_once(".v").is_some_and(|(_, v)| {
                        !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
                    })
                {
                    assert!(
                        super::get(literal).is_ok(),
                        "{}: {literal}",
                        file.path().display()
                    );
                }
            }
        }
        for (id, text) in super::DOCUMENTS {
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            let uri = value["$id"].as_str().unwrap();
            assert!(
                uri == *id || uri.ends_with(&format!("/{id}.schema.json")),
                "{id}: {uri}"
            );
        }
    }
}
