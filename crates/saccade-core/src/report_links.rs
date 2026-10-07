//! Content-addressed report cross-links and portable external index rows.
use crate::{Error, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::{Read, Write},
    path::{Path, PathBuf},
};
/// Exported report-index row contract.
pub const INDEX_SCHEMA: &str = "saccade-report-index-row.v1";
thread_local! { static CONTEXT: RefCell<(Vec<String>,Option<PathBuf>)> = const { RefCell::new((Vec::new(),None)) }; }
/// Set invocation-local external source references and index destination.
/// Core callers can leave the default empty context; report IDs are still emitted.
pub fn context(refs: Vec<String>, index: Option<PathBuf>) -> Result<()> {
    if refs.len() > 128
        || refs
            .iter()
            .any(|s| s.is_empty() || s.len() > 2048 || s.chars().any(char::is_control))
    {
        return Err(Error::Config(
            "source refs need 1..2048 printable bytes, at most 128".into(),
        ));
    }
    CONTEXT.with(|c| *c.borrow_mut() = (refs, index));
    Ok(())
}
/// Scoped context that restores prior references when an MCP request completes.
pub struct ContextGuard((Vec<String>, Option<PathBuf>));
impl Drop for ContextGuard {
    fn drop(&mut self) {
        CONTEXT.with(|c| *c.borrow_mut() = self.0.clone());
    }
}
/// Apply a context for one request, restoring it even on error.
pub fn scope(refs: Vec<String>, index: Option<PathBuf>) -> Result<ContextGuard> {
    let previous = CONTEXT.with(|c| c.borrow().clone());
    context(refs, index)?;
    Ok(ContextGuard(previous))
}
/// Whether a schema describes a report rather than acquisition/policy/authority input.
pub fn is_report_schema(id: &str) -> bool {
    ![
        "-source.",
        "-map.",
        "-mapping.",
        "-authority.",
        "-requests.",
        "-decisions.",
        "-request.",
        "-captures.",
        "-clock.",
        "-buffer.",
        "-models.",
        "-registry.",
        "-corpus.",
        "-fixture.",
        "-constructed-",
        "-motion-vectors.",
        "-frozen.",
        "-trial-",
        "-index.",
    ]
    .iter()
    .any(|part| id.contains(part))
        && !id.contains("-plan.")
        && !id.contains("-frozen-")
        && !id.contains("-blind-key.")
        && !id.contains("-manifest.")
        && !id.contains("-link.")
        && !id.contains("-region-export.")
        && !id.contains("-model.")
        && !id.contains("-export-inputs.")
        && !id.contains("-fingerprint.")
        && !id.contains("-dom-regions.")
        && !id.contains("-renderdoc-extract.")
        && !id.contains("-capture-layers.")
        && !id.contains("-timing-session.")
        && !matches!(
            id,
            "saccade-api-analyze.v1"
                | "saccade-api-compare.v1"
                | "saccade-api-search.v1"
                | "saccade-evidence.v1"
                | "saccade-inbox-item.v1"
                | "saccade-ocrs.v1"
                | "saccade-tesseract.v1"
                | "saccade-paddle-ocr.v1"
                | "saccade-visual-intent.v1"
                | "saccade-object-ids.v1"
                | "saccade-config.v1"
                | "saccade-perf.v1"
                | "saccade-perf.v2"
                | "saccade-perf-pairs.v1"
                | "saccade-labels.v1"
                | "saccade-labels.v2"
                | "saccade-cases.v1"
                | "saccade-inventory.v1"
                | "saccade-quality-sweep.v1"
                | "saccade-asset-views.v1"
                | "saccade-sweep.v1"
                | "saccade-assist-batch.v1"
                | "saccade-report-index-row.v1"
        )
}
/// Legacy strict contracts and their linked successors. Legacy schemas remain frozen.
pub const SCHEMA_MIGRATIONS: &[(&str, &str)] = &[
    ("saccade-a11y.v1", "saccade-a11y.v2"),
    ("saccade-api-health.v1", "saccade-api-health.v2"),
    ("saccade-approve.v1", "saccade-approve.v2"),
    ("saccade-assess.v1", "saccade-assess.v2"),
    ("saccade-assist-gates.v1", "saccade-assist-gates.v2"),
    (
        "saccade-assist-mask-audit.v1",
        "saccade-assist-mask-audit.v2",
    ),
    ("saccade-assist-masks.v1", "saccade-assist-masks.v2"),
    (
        "saccade-assist-observations.v1",
        "saccade-assist-observations.v2",
    ),
    (
        "saccade-assist-vision-provider.v1",
        "saccade-assist-vision-provider.v2",
    ),
    ("saccade-assist.v1", "saccade-assist.v2"),
    ("saccade-capabilities.v1", "saccade-capabilities.v2"),
    ("saccade-crop-check.v1", "saccade-crop-check.v2"),
    ("saccade-decide-result.v1", "saccade-decide-result.v2"),
    ("saccade-dedupe.v1", "saccade-dedupe.v2"),
    ("saccade-design-pull.v1", "saccade-design-pull.v2"),
    ("saccade-design-report.v1", "saccade-design-report.v2"),
    ("saccade-document-ocr.v1", "saccade-document-ocr.v2"),
    ("saccade-document-text.v1", "saccade-document-text.v2"),
    ("saccade-documents.v1", "saccade-documents.v2"),
    (
        "saccade-embedding-export-receipt.v1",
        "saccade-embedding-export-receipt.v2",
    ),
    (
        "saccade-embedding-qualification.v1",
        "saccade-embedding-qualification.v2",
    ),
    ("saccade-embedding-query.v1", "saccade-embedding-query.v2"),
    ("saccade-error.v1", "saccade-error.v2"),
    ("saccade-explain-result.v1", "saccade-explain-result.v2"),
    ("saccade-faces.v1", "saccade-faces.v2"),
    ("saccade-general-result.v1", "saccade-general-result.v2"),
    ("saccade-grounded.v1", "saccade-grounded.v2"),
    ("saccade-hash.v1", "saccade-hash.v2"),
    ("saccade-imgtune-audit.v1", "saccade-imgtune-audit.v2"),
    ("saccade-imgtune-search.v1", "saccade-imgtune-search.v2"),
    ("saccade-imgtune.v1", "saccade-imgtune.v2"),
    ("saccade-inspect-image.v1", "saccade-inspect-image.v2"),
    ("saccade-keyframes.v1", "saccade-keyframes.v2"),
    ("saccade-learned-quality.v1", "saccade-learned-quality.v2"),
    ("saccade-localized.v1", "saccade-localized.v2"),
    ("saccade-locate.v1", "saccade-locate.v2"),
    ("saccade-media-compare.v1", "saccade-media-compare.v2"),
    ("saccade-media-error.v1", "saccade-media-error.v2"),
    (
        "saccade-media-index-query.v1",
        "saccade-media-index-query.v2",
    ),
    ("saccade-media-record.v1", "saccade-media-record.v2"),
    ("saccade-model-status.v1", "saccade-model-status.v2"),
    ("saccade-near-duplicate.v1", "saccade-near-duplicate.v2"),
    ("saccade-notification.v1", "saccade-notification.v2"),
    ("saccade-notify-result.v1", "saccade-notify-result.v2"),
    ("saccade-onset.v1", "saccade-onset.v2"),
    ("saccade-perf-validation.v1", "saccade-perf-validation.v2"),
    ("saccade-pipeline-choice.v1", "saccade-pipeline-choice.v2"),
    (
        "saccade-playwright-matcher.v1",
        "saccade-playwright-matcher.v2",
    ),
    ("saccade-question-report.v1", "saccade-question-report.v2"),
    ("saccade-registration.v1", "saccade-registration.v2"),
    ("saccade-report.v1", "saccade-report.v2"),
    ("saccade-result.v1", "saccade-result.v3"),
    ("saccade-result.v2", "saccade-result.v4"),
    ("saccade-runs.v1", "saccade-runs.v2"),
    ("saccade-safety.v1", "saccade-safety.v2"),
    ("saccade-schema-list.v1", "saccade-schema-list.v2"),
    ("saccade-schema-path.v1", "saccade-schema-path.v2"),
    ("saccade-similar.v1", "saccade-similar.v2"),
    ("saccade-summary.v1", "saccade-summary.v2"),
    ("saccade-sweep-report.v1", "saccade-sweep-report.v2"),
    ("saccade-text.v1", "saccade-text.v2"),
    ("saccade-usage.v1", "saccade-usage.v2"),
    ("saccade-view-summary.v1", "saccade-view-summary.v2"),
    (
        "saccade-vision-observation.v1",
        "saccade-vision-observation.v2",
    ),
    ("saccade-watermark.v1", "saccade-watermark.v2"),
    ("saccade-watermark.v3", "saccade-watermark.v4"),
    ("saccade-ui-review.v1", "saccade-ui-review.v2"),
];
/// Schema emitted when links would violate a legacy strict reader.
pub fn linked_schema(id: &str) -> &str {
    SCHEMA_MIGRATIONS
        .iter()
        .find(|(old, _)| *old == id)
        .map_or(id, |(_, new)| *new)
}
/// Historical discriminator of a linked report, for the shared semantic identity.
pub fn original_schema(id: &str) -> &str {
    SCHEMA_MIGRATIONS
        .iter()
        .find(|(_, new)| *new == id)
        .map_or(id, |(old, _)| *old)
}
/// Data-only view for a strict legacy model of a known linked successor.
/// This creates a copy; the authoritative artifact bytes and their hash stay intact.
/// Unknown discriminators and non-report authority fields are preserved.
pub fn legacy_view(value: &Value) -> Value {
    fn project(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                for child in fields.values_mut() {
                    project(child);
                }
            }
            Value::Array(items) => {
                for child in items {
                    project(child);
                }
            }
            _ => {}
        }
        if let Some(id) = value.get("schema").and_then(Value::as_str)
            && original_schema(id) != id
        {
            let original = original_schema(id).to_owned();
            if let Some(object) = value.as_object_mut() {
                object.remove("report_id");
                object.remove("source_refs");
                object.insert("schema".into(), original.into());
            }
        }
    }
    let mut value = value.clone();
    project(&mut value);
    value
}
/// Decode a report's data-only view; callers still validate its schema and bindings.
/// Only declared migration fields are projected away; strict models reject other fields.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let value: Value =
        crate::evidence::canonical::decode(bytes).map_err(|e| Error::Config(e.to_string()))?;
    Ok(serde_json::from_value(legacy_view(&value))?)
}
/// Add optional linkage fields to the top-level report JSON Schema only.
pub fn extend_schema(value: &mut Value, id: &str) {
    if linked_schema(id) != id {
        fn strip(value: &mut Value) {
            match value {
                Value::Object(object) => {
                    if let Some(properties) =
                        object.get_mut("properties").and_then(Value::as_object_mut)
                    {
                        properties.remove("report_id");
                        properties.remove("source_refs");
                    }
                    for value in object.values_mut() {
                        strip(value);
                    }
                }
                Value::Array(values) => values.iter_mut().for_each(strip),
                _ => {}
            }
        }
        strip(value);
        return;
    }
    if original_schema(id) != id {
        value["properties"]["schema"]["const"] = id.into();
        value["properties"]["report_id"] =
            json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"});
        value["properties"]["source_refs"] = json!({"type":"array","maxItems":128,"items":{"type":"string","minLength":1,"maxLength":2048}});
        let required = value["required"].as_array_mut();
        if let Some(required) = required {
            for field in ["report_id", "source_refs"] {
                if !required.iter().any(|v| v == field) {
                    required.push(field.into());
                }
            }
        }
    }
    if is_report_schema(id)
        && linked_schema(id) == id
        && let Some(properties) = value.get_mut("properties").and_then(Value::as_object_mut)
    {
        properties
            .entry("report_id")
            .or_insert_with(|| json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}));
        properties.entry("source_refs").or_insert_with(
            || json!({"type":"array","maxItems":128,"items":{"type":"string","maxLength":2048}}),
        );
    }
}
/// Migrate a linked transport envelope without replacing its artifact's report ID.
pub fn migrate_linked(value: &Value) -> Value {
    let mut value = value.clone();
    if value["report_id"].is_string()
        && let Some(id) = value["schema"].as_str()
    {
        value["schema"] = linked_schema(id).into();
    }
    value
}
/// Hash the canonical report payload excluding linkage and generation timestamp.
/// Input paths, configuration and measured values remain bound.
pub fn decorate(value: &Value) -> Result<Value> {
    if !value.is_object()
        || !value
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(is_report_schema)
    {
        return Ok(value.clone());
    }
    let id = crate::evidence::case::Measurement::report_identity_value(value)
        .map_err(|e| Error::Config(e.to_string()))?;
    let mut result = value.clone();
    let schema = value["schema"].as_str().unwrap_or_default();
    result["schema"] = json!(linked_schema(schema));
    result["report_id"] = json!(id);
    let refs = CONTEXT.with(|c| c.borrow().0.clone());
    if !refs.is_empty() {
        result["source_refs"] = json!(refs);
    } else if result.get("source_refs").is_none() {
        result["source_refs"] = json!([]);
    }
    Ok(result)
}
/// Coarse outcome class of a report, as recorded in index rows and manifests. It
/// describes the measurement, never approval.
pub fn verdict_class(value: &Value) -> Value {
    if let Some(v) = value
        .get("verdict")
        .or_else(|| value.get("result"))
        .and_then(Value::as_str)
    {
        json!(v)
    } else if original_schema(value["schema"].as_str().unwrap_or_default())
        == crate::report::REPORT_SCHEMA
    {
        serde_json::from_value::<crate::Report>(value.clone())
            .map(|r| {
                json!(if r.is_regression() {
                    "fail"
                } else if r
                    .perf_diff
                    .as_ref()
                    .is_some_and(|p| p.comparability != crate::perf::Comparability::Qualified)
                {
                    "diagnostic_performance"
                } else {
                    "pass"
                })
            })
            .unwrap_or_else(|_| json!("recorded"))
    } else if value.get("arms").is_some() {
        json!("ablation_table")
    } else {
        value
            .get("coverage")
            .and_then(Value::as_str)
            .map(|v| json!(v))
            .unwrap_or_else(|| json!("recorded"))
    }
}
/// Indexed row; timestamps describe index insertion, never statistical qualification.
pub fn index(path: &Path, value: &Value) -> Result<()> {
    if value.get("report_id").is_none()
        || !value
            .get("schema")
            .and_then(Value::as_str)
            .is_some_and(is_report_schema)
    {
        return Ok(());
    }
    let target = CONTEXT.with(|c| c.borrow().1.clone()).unwrap_or_else(|| {
        path.parent()
            .unwrap_or(Path::new("."))
            .join("reports/index.jsonl")
    });
    let parent = target.parent().unwrap_or(Path::new("."));
    if std::fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Config(
            "report index directory cannot be a symlink".into(),
        ));
    }
    std::fs::create_dir_all(parent)
        .map_err(crate::run::io_err("creating report index directory".into()))?;
    if std::fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Config("report index cannot be a symlink".into()));
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&target)
        .map_err(crate::run::io_err("opening report index".into()))?;
    file.lock()
        .map_err(crate::run::io_err("locking report index".into()))?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Config(e.to_string()))?
        .as_secs();
    let verdict = verdict_class(value);
    let row = json!({"report_id":value["report_id"],"source_refs":value["source_refs"],"verdict_class":verdict,"timestamp_unix":timestamp,"report_path":crate::paths::portable(&crate::explain::absolute(path)),"schema":INDEX_SCHEMA,"report_schema":value["schema"]});
    let mut writer = &file;
    writer
        .write_all(format!("{}\n", serde_json::to_string(&row)?).as_bytes())
        .map_err(crate::run::io_err("appending report index".into()))?;
    Ok(())
}
/// Write an enriched JSON report and append its external index row.
pub fn write<T: Serialize>(path: &Path, report: &T) -> Result<Value> {
    let value = decorate(&serde_json::to_value(report)?)?;
    std::fs::write(path, serde_json::to_vec_pretty(&value)?)
        .map_err(crate::run::io_err("writing linked report".into()))?;
    index(path, &value)?;
    Ok(value)
}
/// Add report links to JSON output bytes. Non-JSON assets pass through unchanged.
pub fn write_bytes(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    let bytes = bytes.as_ref();
    if path.extension().is_some_and(|e| e == "json")
        && let Ok(value) = serde_json::from_slice::<Value>(bytes)
        && value.get("schema").is_some_and(Value::is_string)
    {
        write(path, &value)?;
    } else {
        std::fs::write(path, bytes).map_err(crate::run::io_err("writing report asset".into()))?;
    }
    Ok(())
}
/// Export a bounded JSONL index as parsed rows, without altering it.
pub fn export(path: &Path) -> Result<Vec<Value>> {
    let mut file =
        std::fs::File::open(path).map_err(crate::run::io_err("opening report index".into()))?;
    file.lock_shared()
        .map_err(crate::run::io_err("locking report index".into()))?;
    let mut text = String::new();
    Read::by_ref(&mut file)
        .take((16 << 20) + 1)
        .read_to_string(&mut text)
        .map_err(crate::run::io_err("reading report index".into()))?;
    if text.len() > 16 << 20 {
        return Err(Error::Config("report index exceeds 16 MiB".into()));
    }
    text.lines()
        .filter(|s| !s.trim().is_empty())
        .map(|s| serde_json::from_str(s).map_err(Error::from))
        .collect()
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn legacy_projection_handles_nested_reports_and_preserves_other_fields() {
        let value = json!({"outer":[{
            "schema":"saccade-crop-check.v2", "report_id":"sha256:crop", "source_refs":[],
            "detection":{"schema":"saccade-faces.v2", "report_id":"sha256:face", "source_refs":[], "future_field":true},
            "input":{"schema":"saccade-brand-source.v1", "report_id":"authority"},
            "newer":{"schema":"saccade-faces.v99", "report_id":"newer"}
        }]});
        let projected = legacy_view(&value);
        let crop = &projected["outer"][0];
        assert_eq!(crop["schema"], "saccade-crop-check.v1");
        assert!(crop.get("report_id").is_none());
        assert!(crop.get("source_refs").is_none());
        assert_eq!(crop["detection"]["schema"], "saccade-faces.v1");
        assert!(crop["detection"].get("report_id").is_none());
        assert_eq!(crop["detection"]["future_field"], true);
        assert_eq!(crop["input"], value["outer"][0]["input"]);
        assert_eq!(crop["newer"], value["outer"][0]["newer"]);
        assert_eq!(value["outer"][0]["schema"], "saccade-crop-check.v2");
    }
    #[test]
    #[cfg(unix)]
    fn implicit_index_refuses_an_escaped_directory_and_index_rows_keep_their_ids() {
        let owned = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), owned.path().join("reports")).unwrap();
        let _scope = scope(vec![], None).unwrap();
        let value = json!({"schema":"saccade-timing-ab.v1","verdict":"equivalent"});
        assert!(write(&owned.path().join("report.json"), &value).is_err());
        assert!(!outside.path().join("index.jsonl").exists());
        let row = json!({"schema":INDEX_SCHEMA,"report_id":"sha256:source-report"});
        assert_eq!(decorate(&row).unwrap(), row);
    }
    #[test]
    fn index_verdict_class_is_a_string_for_structured_results() {
        let tmp = tempfile::tempdir().unwrap();
        let _scope = scope(vec![], Some(tmp.path().join("index.jsonl"))).unwrap();
        let report = json!({"schema":"saccade-timing-ab.v1","result":{"count":2},"coverage":0.5});
        write(&tmp.path().join("report.json"), &report).unwrap();
        let rows = export(&tmp.path().join("index.jsonl")).unwrap();
        assert_eq!(rows[0]["verdict_class"], "recorded");
    }
    #[test]
    fn pair_report_id_is_the_existing_relocatable_measurement_identity() {
        let mut report: crate::Report = serde_json::from_value(json!({
            "schema":"saccade-report.v1","tool_version":"fixture","generated_at_unix":1,
            "baseline_dir":"old/baseline","capture_dir":"old/capture",
            "config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},
            "totals":{"total":0,"pass":0,"fail":0,"error":0,"missing":0,"new":0},"entries":[]
        })).unwrap();
        let id = crate::evidence::case::Measurement::report_identity(&report).unwrap();
        let value = decorate(&serde_json::to_value(&report).unwrap()).unwrap();
        assert_eq!(value["report_id"], id.as_str());
        assert_eq!(value["schema"], "saccade-report.v2");
        let linked: crate::Report = serde_json::from_value(value).unwrap();
        assert_eq!(
            crate::evidence::case::Measurement::report_identity(&linked).unwrap(),
            id
        );
        report.generated_at_unix = 999;
        report.baseline_dir = Some("relocated/baseline".into());
        report.capture_dir = Some("relocated/capture".into());
        assert_eq!(
            decorate(&serde_json::to_value(&report).unwrap()).unwrap()["report_id"],
            id.as_str()
        );
        report.config.default_threshold = 0.02;
        assert_ne!(
            crate::evidence::case::Measurement::report_identity(&report).unwrap(),
            id
        );
    }
    #[test]
    fn unrelated_report_domains_share_stable_content_ids_and_refs() {
        let tmp = tempfile::tempdir().unwrap();
        context(
            vec!["urn:specimen:12".into()],
            Some(tmp.path().join("reports/index.jsonl")),
        )
        .unwrap();
        let v = json!({"schema":"saccade-report.v1","verdict":"pass","observations":[2,3],"generated_at":"one"});
        let a = write(&tmp.path().join("a.json"), &v).unwrap();
        let mut v = v;
        v["generated_at"] = json!("two");
        let b = decorate(&v).unwrap();
        assert_eq!(a["report_id"], b["report_id"]);
        v["observations"][0] = json!(4);
        assert_ne!(a["report_id"], decorate(&v).unwrap()["report_id"]);
        let rows = export(&tmp.path().join("reports/index.jsonl")).unwrap();
        assert_eq!(rows[0]["source_refs"][0], "urn:specimen:12");
        context(vec![], None).unwrap();
    }
}
