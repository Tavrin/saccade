//! Bounded, producer-neutral telemetry mapping.
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
/// Producer paths and transformations for native GPU clock evidence.
pub struct ClockMap {
    /// Native destination fields mapped from source paths.
    pub fields: BTreeMap<String, Source>,
    /// Source sample-window array and its per-item mapping.
    pub windows: Windows,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
/// Repeated sample windows in the producer record.
pub struct Windows {
    /// Dotted source path, with numeric indices for arrays.
    pub path: String,
    /// Native destination fields mapped from source paths.
    pub fields: BTreeMap<String, Source>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
/// A bounded source selection, with an optional explicit transformation.
pub struct Source {
    /// Dotted source path, with numeric indices for arrays.
    pub path: String,
    #[serde(default)]
    /// Alternative source paths, tried in order.
    pub fallback: Vec<String>,
    /// Output value used only when no source candidate is available.
    pub default: Option<Value>,
    #[serde(default)]
    /// Transformation applied to the selected source.
    pub transform: Transform,
    /// Per-array-item path, required by the sum transformation.
    pub item_path: Option<String>,
    /// Text before a nonzero mask value; defaults to a neutral throttle label.
    pub prefix: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Declarative transformations; no producer-specific code executes.
pub enum Transform {
    #[default]
    /// Retain the selected JSON value unchanged.
    Value,
    /// Select the first string candidate, skipping non-string values.
    FirstString,
    /// Count source array items.
    Count,
    /// Sum unsigned integer values selected within source array items.
    Sum,
    /// Map zero to no throttling and nonzero to one attributed reason.
    Mask,
}
impl ClockMap {
    /// Read and validate a TOML/JSON mapping of at most 1 MiB.
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = crate::evidence_quality::read(path, 1 << 20).map_err(|e| e.to_string())?;
        let map: Self = if path.extension().is_some_and(|e| e == "json") {
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
        } else {
            toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        };
        map.validate()?;
        Ok(map)
    }
    fn validate(&self) -> Result<(), String> {
        if self.fields.len() > 16 || self.windows.fields.len() > 32 {
            return Err("GPU clock map exceeds field limit".into());
        }
        validate_path(&self.windows.path)?;
        for key in self.fields.keys() {
            if !matches!(key.as_str(), "device_id" | "power_state") {
                return Err(format!("GPU clock map: unsupported destination {key:?}"));
            }
        }
        for key in self.windows.fields.keys() {
            if !matches!(
                key.as_str(),
                "name"
                    | "core_mhz"
                    | "core_mhz.min"
                    | "core_mhz.median"
                    | "core_mhz.max"
                    | "memory_mhz"
                    | "memory_mhz.min"
                    | "memory_mhz.median"
                    | "memory_mhz.max"
                    | "sample_count"
                    | "expected_frames"
                    | "observed_frames"
                    | "query_failures"
                    | "throttle_reasons"
                    | "stabilized"
            ) {
                return Err(format!("GPU clock map: unsupported destination {key:?}"));
            }
        }
        for source in self.fields.values().chain(self.windows.fields.values()) {
            validate_path(&source.path)?;
            if source.fallback.len() > 16 {
                return Err("GPU clock map exceeds fallback limit".into());
            }
            for path in &source.fallback {
                validate_path(path)?;
            }
            if let Some(path) = &source.item_path {
                validate_path(path)?;
            }
            if matches!(source.transform, Transform::Sum) != source.item_path.is_some() {
                return Err(
                    "GPU clock map: sum requires item_path; other transforms forbid it".into(),
                );
            }
            if !matches!(source.transform, Transform::Mask) && source.prefix.is_some() {
                return Err("GPU clock map: prefix requires mask".into());
            }
        }
        for fields in [&self.fields, &self.windows.fields] {
            for key in fields.keys() {
                if fields
                    .keys()
                    .any(|other| other.starts_with(&format!("{key}.")))
                {
                    return Err(format!("GPU clock map: conflicting destination {key:?}"));
                }
            }
        }
        Ok(())
    }
    /// Adapt one producer record without inventing missing measured facts.
    pub fn apply(&self, record: &Value) -> Result<super::GpuClock, String> {
        self.validate()?;
        let mut result = mapped(&self.fields, record)?;
        result.insert("schema".into(), "saccade-gpu-clock.v1".into());
        let windows = crate::arms::lookup(record, &self.windows.path)
            .ok_or_else(|| format!("GPU clock map: {} is absent", self.windows.path))?;
        let windows = windows
            .as_array()
            .ok_or("GPU clock map: windows must be an array")?;
        let windows = windows
            .iter()
            .map(|w| mapped(&self.windows.fields, w).map(Value::Object))
            .collect::<Result<Vec<_>, _>>()?;
        result.insert("windows".into(), windows.into());
        serde_json::from_value(Value::Object(result)).map_err(|e| format!("GPU clock map: {e}"))
    }
}
fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 1024 || path.contains(['*', '?', '[', '{']) {
        return Err(format!(
            "GPU clock map: invalid source path {path:?}; use dotted paths with explicit numeric array indices, no wildcards"
        ));
    }
    Ok(())
}
fn mapped(fields: &BTreeMap<String, Source>, record: &Value) -> Result<Map<String, Value>, String> {
    let mut result = Map::new();
    for (dest, source) in fields {
        let input = std::iter::once(&source.path)
            .chain(&source.fallback)
            .filter_map(|path| crate::arms::lookup(record, path))
            .find(|v| !matches!(source.transform, Transform::FirstString) || v.is_string());
        let value = match input {
            None => source
                .default
                .clone()
                .ok_or_else(|| format!("GPU clock map: {} is absent for {dest}", source.path))?,
            Some(input) => match source.transform {
                Transform::Value | Transform::FirstString => input,
                Transform::Count => Value::from(
                    input
                        .as_array()
                        .ok_or("GPU clock map: count requires array")?
                        .len() as u64,
                ),
                Transform::Sum => {
                    let items = input
                        .as_array()
                        .ok_or("GPU clock map: sum requires array")?;
                    let path = source
                        .item_path
                        .as_deref()
                        .ok_or("GPU clock map: sum requires item_path")?;
                    let mut sum = 0u64;
                    for item in items {
                        let value = crate::arms::lookup(item, path)
                            .and_then(|v| v.as_u64())
                            .ok_or("GPU clock map: sum item must be an unsigned integer")?;
                        sum = sum
                            .checked_add(value)
                            .ok_or("GPU clock map: sum overflow")?;
                    }
                    Value::from(sum)
                }
                Transform::Mask => {
                    let mask = input
                        .as_u64()
                        .ok_or("GPU clock map: mask must be an unsigned integer")?;
                    if mask == 0 {
                        Value::Array(Vec::new())
                    } else {
                        Value::Array(vec![Value::String(format!(
                            "{}{mask}",
                            source.prefix.as_deref().unwrap_or("Throttle mask ")
                        ))])
                    }
                }
            },
        };
        if let Some((parent, child)) = dest.split_once('.') {
            let object = result
                .entry(parent)
                .or_insert_with(|| Value::Object(Map::new()));
            object
                .as_object_mut()
                .ok_or("GPU clock map: conflicting destinations")?
                .insert(child.into(), value);
        } else {
            result.insert(dest.clone(), value);
        }
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn example() -> ClockMap {
        toml::from_str(include_str!(
            "../../tests/fixtures/package/examples/gpu-clock/map.toml"
        ))
        .unwrap()
    }
    fn record() -> Value {
        json!({"device":{"uuid":"GPU-1","name":"fallback"},"sample_windows":[{"name":"frame","sm_mhz":{"min":1790,"p50":1800,"max":1810},"memory_mhz":{"min":6000,"p50":6000,"max":6000},"sample_count":32,"expected_frames":2,"frames":[{"query_failures":1},{"query_failures":2}],"throttle_reasons":0,"warm_to_boost":{"met":true}}]})
    }
    #[test]
    fn example_preserves_sampled_telemetry() {
        let expected: super::super::GpuClock = serde_json::from_value(json!({"schema":"saccade-gpu-clock.v1","device_id":"GPU-1","power_state":null,"windows":[{"name":"frame","core_mhz":{"min":1790,"median":1800,"max":1810},"memory_mhz":{"min":6000,"median":6000,"max":6000},"sample_count":32,"expected_frames":2,"observed_frames":2,"query_failures":3,"throttle_reasons":[],"stabilized":true}]})).unwrap();
        assert_eq!(example().apply(&record()).unwrap(), expected);
        let mut record = record();
        record["device"].as_object_mut().unwrap().remove("uuid");
        record["sample_windows"][0]["throttle_reasons"] = json!(4);
        record["sample_windows"][0]
            .as_object_mut()
            .unwrap()
            .remove("warm_to_boost");
        let mapped = example().apply(&record).unwrap();
        assert_eq!(mapped.device_id, "fallback");
        assert_eq!(mapped.windows[0].throttle_reasons, ["Throttle mask 4"]);
        assert!(!mapped.windows[0].stabilized);
        assert!(
            mapped
                .qualification_reasons()
                .iter()
                .any(|r| r.contains("power state"))
        );
    }
    #[test]
    fn unrelated_job_records_and_flat_array_paths_work_in_json_and_toml() {
        let window = json!({"name":"processing","core_mhz":{"min":1000,"median":1000,"max":1000},"sample_count":1,"expected_frames":1,"observed_frames":1,"query_failures":0,"throttle_reasons":[],"stabilized":true});
        let data = json!({"jobs":[{"device":"compute-1","power":"ac-performance","measurements":[window]}],"jobs.0.device":"flat-identity"});
        let fields = [
            "name",
            "core_mhz",
            "sample_count",
            "expected_frames",
            "observed_frames",
            "query_failures",
            "throttle_reasons",
            "stabilized",
        ]
        .into_iter()
        .map(|key| (key.into(), json!({"path":key})))
        .collect::<Map<_, _>>();
        let value = json!({"fields":{"device_id":{"path":"jobs.0.device"},"power_state":{"path":"jobs.0.power"}},"windows":{"path":"jobs.0.measurements","fields":fields}});
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("map.json");
        std::fs::write(&path, value.to_string()).unwrap();
        let clock = ClockMap::read(&path).unwrap().apply(&data).unwrap();
        assert_eq!(clock.device_id, "flat-identity");
        assert!(clock.qualification_reasons().is_empty());
        let toml_path = temp.path().join("map.toml");
        std::fs::write(
            &toml_path,
            include_str!("../../tests/fixtures/package/examples/gpu-clock/map.toml"),
        )
        .unwrap();
        assert_eq!(
            ClockMap::read(&toml_path)
                .unwrap()
                .apply(&record())
                .unwrap(),
            example().apply(&record()).unwrap()
        );
    }
    #[test]
    fn missing_invalid_wildcard_conflicting_and_overflowing_inputs_fail() {
        let mut map = example();
        let mut data = record();
        data["sample_windows"][0]["frames"][0]["query_failures"] = json!(u64::MAX);
        assert!(map.apply(&data).unwrap_err().contains("overflow"));
        data["sample_windows"][0]["frames"][0]["query_failures"] = json!(-1);
        assert!(map.apply(&data).is_err());
        data["sample_windows"][0]
            .as_object_mut()
            .unwrap()
            .remove("sample_count");
        assert!(map.apply(&data).is_err());
        map.fields.get_mut("device_id").unwrap().path = "devices.*.uuid".into();
        assert!(map.apply(&record()).unwrap_err().contains("no wildcards"));
        let mut map = example();
        map.windows
            .fields
            .insert("core_mhz".into(), map.windows.fields["name"].clone());
        assert!(map.apply(&record()).unwrap_err().contains("conflicting"));
        let mut data = record();
        data["sample_windows"][0]["warm_to_boost"]["met"] = json!("true");
        assert!(example().apply(&data).is_err());
    }
}
