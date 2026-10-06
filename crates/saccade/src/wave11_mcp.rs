//! Root-contained mirrors of timing analysis, settling and external indexes.
use crate::agent::CliError;
use saccade_core::root_policy::RootPolicy;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Args {
    #[serde(rename = "timing_ab")]
    Timing {
        input: PathBuf,
        out: PathBuf,
        #[serde(default = "session_format")]
        format: String,
        band_pct: Option<f64>,
        #[serde(default)]
        source_refs: Vec<String>,
    },
    #[serde(rename = "settle")]
    Settle {
        frames: PathBuf,
        out: PathBuf,
        reference: Option<PathBuf>,
        change_frame: Option<usize>,
        event: Option<PathBuf>,
        #[serde(default = "fps")]
        fps: f64,
        #[serde(default = "tile")]
        tile_size: u32,
        #[serde(default = "threshold")]
        threshold: f64,
        #[serde(default = "three")]
        consecutive: usize,
        #[serde(default = "three")]
        final_frames: usize,
        #[serde(default)]
        source_refs: Vec<String>,
    },
    #[serde(rename = "index_export")]
    Export {
        index: PathBuf,
        out: Option<PathBuf>,
        #[serde(default = "jsonl")]
        format: String,
    },
}
fn session_format() -> String {
    "session".into()
}
fn jsonl() -> String {
    "jsonl".into()
}
fn fps() -> f64 {
    30.
}
fn tile() -> u32 {
    32
}
fn threshold() -> f64 {
    0.02
}
fn three() -> usize {
    3
}
pub(crate) fn handles(name: &str, op: &str) -> bool {
    name == "saccade_measure" && matches!(op, "timing_ab" | "settle" | "index_export")
}
fn summary(value: &Value, file: &Path) -> Result<Value, CliError> {
    let refs = value["source_refs"].as_array().cloned().unwrap_or_default();
    let reasons = value["reasons"].as_array().cloned().unwrap_or_default();
    let mut compact = value.clone();
    compact["source_refs"] = json!(
        refs.iter()
            .take(3)
            .filter_map(Value::as_str)
            .map(|s| crate::local_cmd::short(s, 200))
            .collect::<Vec<_>>()
    );
    compact["reasons"] = json!(
        reasons
            .iter()
            .take(4)
            .filter_map(Value::as_str)
            .map(|s| crate::local_cmd::short(s, 200))
            .collect::<Vec<_>>()
    );
    let value = &compact;
    let mut result =
        crate::local_cmd::base_result(value["schema"].as_str().unwrap_or("experiment"));
    result["schema"] = saccade_core::report_links::linked_schema("saccade-result.v2").into();
    result["report_id"] = value["report_id"].clone();
    result["source_refs"] = value["source_refs"].clone();
    result["verdict"] = value["verdict"].clone();
    result["artifact"] = crate::local_cmd::reference(file)?;
    result["data"] = json!({"settle_frame":value["settle_frame"],"lag_frames":value["lag_frames"],"effect_pct":value["effect_pct"],"interval_pct":value["interval_pct"],"diagnostic_only":value["diagnostic_only"],"reasons":value["reasons"],"omitted":{"source_refs":refs.len().saturating_sub(3),"reasons":reasons.len().saturating_sub(4)}});
    crate::local_cmd::bounded(result, 4096)
}
pub(crate) fn call(
    policy: &RootPolicy,
    args: &serde_json::Map<String, Value>,
) -> Result<Value, CliError> {
    match serde_json::from_value(Value::Object(args.clone()))? {
        Args::Timing {
            input,
            out,
            format,
            band_pct,
            source_refs,
        } => {
            let input = policy.read(&input)?;
            let out = policy.write(&out)?;
            let _context = saccade_core::report_links::scope(
                source_refs,
                Some(out.join("reports/index.jsonl")),
            )?;
            let root = input.parent().unwrap_or(Path::new("."));
            if format != "csv" {
                let v: Value = serde_json::from_slice(&saccade_core::evidence_quality::read(
                    &input,
                    4 << 20,
                )?)?;
                if let Some(pairs) = v["pairs"].as_array() {
                    for p in pairs {
                        for arm in ["a", "b"] {
                            if let Some(f) = p[arm]["file"].as_str() {
                                policy.read(&root.join(f))?;
                            }
                        }
                    }
                }
                if let Some(pairs) = v["captures"].as_array() {
                    for p in pairs {
                        if let Some(pair) = p.as_array() {
                            for path in pair {
                                let s = path.as_str().ok_or_else(|| {
                                    CliError::usage("capture path must be a string")
                                })?;
                                policy.read(&root.join(s))?;
                            }
                        }
                    }
                }
            }
            let (mut session, sources) = saccade_core::timing::load(&input, &format)?;
            if let Some(b) = band_pct {
                session.band_pct = b;
            }
            let report = saccade_core::timing::analyze(session, sources, root)?;
            crate::general_cmd::prepare_out(&out, &[&input])?;
            let file = out.join(format!("{}.json", saccade_core::timing::REPORT_SCHEMA));
            let value = saccade_core::report_links::write(&file, &report)?;
            summary(&value, &file)
        }
        Args::Export { index, out, format } => {
            if !matches!(format.as_str(), "json" | "jsonl") {
                return Err(CliError::usage("format must be json or jsonl"));
            }
            let index = policy.read(&index)?;
            let rows = saccade_core::report_links::export(&index)?;
            if let Some(out) = out {
                let out = policy.write(&out)?;
                let text = match format.as_str() {
                    "json" => serde_json::to_string_pretty(&rows)?,
                    "jsonl" => rows
                        .iter()
                        .map(serde_json::to_string)
                        .collect::<Result<Vec<_>, _>>()?
                        .join("\n"),
                    _ => return Err(CliError::usage("format must be json or jsonl")),
                };
                crate::schema_cmd::write_new(&out, format!("{text}\n").as_bytes())?;
                let mut result = crate::local_cmd::base_result("index_export");
                result["counts"] = json!({"rows":rows.len()});
                result["artifact"] = crate::local_cmd::reference(&out)?;
                Ok(result)
            } else {
                let count = rows.len();
                let mut preview = rows.into_iter().take(4).collect::<Vec<_>>();
                loop {
                    let mut result = crate::local_cmd::base_result("index_export");
                    result["data"] = json!({"rows":preview,"omitted":count-preview.len()});
                    result["counts"] = json!({"rows":count});
                    result["artifact"] = crate::local_cmd::reference(&index)?;
                    if serde_json::to_vec(&result)?.len() <= 4096 {
                        return Ok(result);
                    }
                    if preview.pop().is_none() {
                        return Err(CliError::usage(
                            "index artifact path exceeds MCP response budget",
                        ));
                    }
                }
            }
        }
        Args::Settle {
            frames,
            out,
            reference,
            change_frame,
            event,
            fps,
            tile_size,
            threshold,
            consecutive,
            final_frames,
            source_refs,
        } => {
            #[cfg(feature = "graphics")]
            {
                let frames = policy.read(&frames)?;
                let reference = reference.map(|p| policy.read(&p)).transpose()?;
                let event = event.map(|p| policy.read(&p)).transpose()?;
                let out = policy.write(&out)?;
                let _context = saccade_core::report_links::scope(
                    source_refs,
                    Some(out.join("reports/index.jsonl")),
                )?;
                let change = if let Some(i) = change_frame {
                    i
                } else {
                    let event = event
                        .as_ref()
                        .ok_or_else(|| CliError::usage("declare change_frame or event"))?;
                    let v: Value = serde_json::from_slice(&saccade_core::evidence_quality::read(
                        event,
                        1 << 20,
                    )?)?;
                    v["change_frame"]
                        .as_u64()
                        .and_then(|i| usize::try_from(i).ok())
                        .ok_or_else(|| CliError::usage("integer change_frame required"))?
                };
                let mut sources = std::collections::BTreeMap::new();
                for dir in std::iter::once(&frames).chain(reference.iter()) {
                    for file in saccade_core::settling::frame_paths(dir)? {
                        let file = policy.read(&file)?;
                        sources.insert(
                            file.display().to_string(),
                            saccade_core::run::sha256_file(&file)?,
                        );
                    }
                }
                if let Some(e) = &event {
                    sources.insert(e.display().to_string(), saccade_core::run::sha256_file(e)?);
                }
                let observations = crate::wave11_cmd::frames(&frames)?;
                let refs = reference
                    .as_deref()
                    .map(crate::wave11_cmd::frames)
                    .transpose()?;
                let report = saccade_core::settling::analyze(
                    &observations,
                    refs.as_deref(),
                    saccade_core::settling::Policy {
                        change_frame: change,
                        fps,
                        tile_size,
                        threshold,
                        consecutive,
                        final_frames,
                    },
                )?;
                let mut inputs = vec![frames.as_path()];
                if let Some(r) = &reference {
                    inputs.push(r);
                }
                if let Some(e) = &event {
                    inputs.push(e);
                }
                crate::general_cmd::prepare_out(&out, &inputs)?;
                let file = out.join(format!("{}.json", saccade_core::settling::SCHEMA));
                let mut value = serde_json::to_value(&report)?;
                value["sources"] = json!(sources);
                let value = saccade_core::report_links::write(&file, &value)?;
                std::fs::write(out.join("index.html"), report.html())
                    .map_err(|e| CliError::io(e.to_string()))?;
                summary(&value, &file)
            }
            #[cfg(not(feature = "graphics"))]
            {
                let _ = (
                    frames,
                    out,
                    reference,
                    change_frame,
                    event,
                    fps,
                    tile_size,
                    threshold,
                    consecutive,
                    final_frames,
                    source_refs,
                );
                Err(saccade_core::Error::FeatureUnavailable {
                    feature: "graphics",
                }
                .into())
            }
        }
    }
}
pub(crate) fn schemas() -> Vec<Value> {
    let mut result = Vec::new();
    for (op, properties, required) in [
        (
            "timing_ab",
            json!({"input":{"type":"string"},"out":{"type":"string"},"format":{"enum":["session","csv"]},"band_pct":{"type":"number"},"source_refs":{"type":"array","maxItems":128,"items":{"type":"string"}}}),
            vec!["operation", "input", "out"],
        ),
        (
            "settle",
            json!({"frames":{"type":"string"},"out":{"type":"string"},"reference":{"type":"string"},"event":{"type":"string"},"change_frame":{"type":"integer","minimum":1},"fps":{"type":"number"},"tile_size":{"type":"integer"},"threshold":{"type":"number"},"consecutive":{"type":"integer"},"final_frames":{"type":"integer"},"source_refs":{"type":"array","maxItems":128,"items":{"type":"string"}}}),
            vec!["operation", "frames", "out"],
        ),
        (
            "index_export",
            json!({"index":{"type":"string"},"out":{"type":"string"},"format":{"enum":["jsonl","json"]}}),
            vec!["operation", "index"],
        ),
    ] {
        let mut p = properties;
        p["operation"] = json!({"const":op});
        result.push(json!({"type":"object","additionalProperties":false,"properties":p,"required":required}));
    }
    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn timing_mirror_is_bounded_and_rejects_external_reference_egress() {
        let input = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let policy =
            RootPolicy::new(&[input.path().into()], Some(output.path()), false, &[]).unwrap();
        let manifest = input.path().join("session.json");
        let mut pairs = Vec::new();
        for aa in [false, true] {
            for i in 0..6 {
                pairs.push(json!({"id":format!("{aa}-{i}"),"block":format!("{aa}-{i}"),"order":if i%2==0{"ab"}else{"ba"},"a":[10.],"b":[if aa{10.}else{11.}],"aa":aa}));
            }
        }
        let mut v = json!({"schema":saccade_core::timing::SESSION_SCHEMA,"session":"specimen-run","band_pct":2,"max_pairs":6,"confidence":0.95,"seed":1,"resamples":1024,"pairs":pairs});
        std::fs::write(&manifest, serde_json::to_vec(&v).unwrap()).unwrap();
        let args = json!({"operation":"timing_ab","input":manifest,"out":output.path().join("report"),"source_refs":["urn:specimen:4"]});
        let result = call(&policy, args.as_object().unwrap()).unwrap();
        let shape: Value =
            serde_json::from_str(saccade_core::schema_catalog::get("saccade-result.v4").unwrap())
                .unwrap();
        jsonschema::validator_for(&shape)
            .unwrap()
            .validate(&result)
            .unwrap();
        let legacy: Value =
            serde_json::from_str(saccade_core::schema_catalog::get("saccade-result.v2").unwrap())
                .unwrap();
        assert!(
            !jsonschema::validator_for(&legacy)
                .unwrap()
                .is_valid(&result)
        );
        serde_json::from_value::<saccade_core::evidence::action::ResultEnvelope>(result.clone())
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(result["verdict"], "slower");
        assert!(result["data"]["diagnostic_only"].as_bool().unwrap());
        assert!(serde_json::to_vec(&result).unwrap().len() <= 4096);
        assert_eq!(result["source_refs"][0], "urn:specimen:4");
        let secret = outside.path().join("measure.json");
        std::fs::write(&secret, b"10").unwrap();
        v["pairs"][0]["a"] = json!({"file":secret,"format":"json","path":"latency"});
        std::fs::write(&manifest, serde_json::to_vec(&v).unwrap()).unwrap();
        let mut args = args;
        args["out"] = json!(output.path().join("refused"));
        assert!(call(&policy, args.as_object().unwrap()).is_err());
        assert!(!output.path().join("refused").exists());
        let index = output.path().join("report/reports/index.jsonl");
        let exported=call(&policy,json!({"operation":"index_export","index":index,"out":output.path().join("export.json"),"format":"json"}).as_object().unwrap()).unwrap();
        assert_eq!(exported["counts"]["rows"], 1);
    }
}
