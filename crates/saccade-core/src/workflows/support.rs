//! Shared artifact and contract plumbing.
use super::CommandError as CliError;
use crate::evidence::{
    Artifact, Document,
    canonical::{self, Digest},
    case::*,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub(crate) fn parse_contract<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    schema: &str,
) -> Result<T, CliError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(actual) = value.get("schema").and_then(|v| v.as_str())
        && crate::report_links::original_schema(actual) != schema
        && !(schema == crate::wave7::watermark::WATERMARK_SCHEMA
            && crate::report_links::original_schema(actual) == "saccade-watermark.v1")
        && !(schema == "saccade-report.v1" && actual == "flipdiff-report.v1")
    {
        let prefix = schema
            .rsplit_once('v')
            .map(|(prefix, _)| format!("{prefix}v"))
            .unwrap_or_else(|| schema.to_owned());
        let supported = schema
            .rsplit_once('v')
            .and_then(|(_, v)| v.parse::<u32>().ok())
            .unwrap_or(1);
        if actual
            .strip_prefix(prefix.as_str())
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v > supported)
        {
            return Err(CliError::new(
                "version_skew",
                format!("written by {actual}; installed saccade supports up to {schema}, upgrade"),
            ));
        }
        return Err(CliError::usage(format!(
            "expected {schema}, found {actual}"
        )));
    }
    reject_newer_nested_schemas(&value)?;
    enum ParseFailure {
        UnknownField(String),
        Malformed(String),
    }
    fn strict<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ParseFailure> {
        let mut parser = serde_json::Deserializer::from_slice(bytes);
        let mut ignored = None;
        let parsed = serde_ignored::deserialize(&mut parser, |path| {
            if ignored.is_none() {
                ignored = Some(path.to_string());
            }
        })
        .map_err(|e| {
            let message = e.to_string();
            if message.starts_with("unknown field `") {
                ParseFailure::UnknownField(message)
            } else {
                ParseFailure::Malformed(message)
            }
        })?;
        if let Some(path) = ignored {
            return Err(ParseFailure::UnknownField(format!("unknown field {path}")));
        }
        Ok(parsed)
    }
    match strict(bytes) {
        Ok(parsed) => Ok(parsed),
        Err(mut first) => {
            let legacy = crate::report_links::legacy_view(&value);
            if legacy != value {
                // Only known linkage is projected away, and duplicate keys still
                // reject. Original retained bytes remain the artifact-hash source.
                let _: serde_json::Value = crate::evidence::canonical::decode(bytes)?;
                let projected = serde_json::to_vec(&legacy)?;
                match strict(&projected) {
                    Ok(parsed) => return Ok(parsed),
                    Err(error) => first = error,
                }
            }
            match first {
                ParseFailure::UnknownField(first) => Err(CliError::new(
                    "version_skew",
                    format!(
                        "written by a newer producer; installed saccade supports {schema}, upgrade: {first}"
                    ),
                )),
                ParseFailure::Malformed(first) => Err(CliError::io(format!("JSON error: {first}"))),
            }
        }
    }
}
pub(crate) fn reject_newer_nested_schemas(value: &serde_json::Value) -> Result<(), CliError> {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(actual) = fields.get("schema").and_then(|v| v.as_str()) {
                for prefix in [
                    // wave7
                    "saccade-model-registry.v",
                    "saccade-model-status.v",
                    "saccade-locate.v",
                    "saccade-vision-observation.v",
                    "saccade-learned-quality.v",
                    "saccade-watermark.v",
                    "saccade-faces.v",
                    "saccade-crop-check.v",
                    "saccade-provider-mapping.v",
                    "saccade-report.v",
                    "saccade-perf-diff.v",
                    "saccade-noise.v",
                    "saccade-history.v",
                    "saccade-onset.v",
                    "saccade-inventory.v",
                    "saccade-inventory-report.v",
                    "saccade-localized.v",
                    "saccade-frozen-region.v",
                    "saccade-dom-regions.v",
                    "saccade-grounded.v",
                    "saccade-quality-sweep.v",
                    "saccade-quality-report.v",
                    "saccade-region-models.v",
                    "saccade-renderdoc-extract.v",
                    "saccade-renderdoc-localization.v",
                    "saccade-brand-source.v",
                    "saccade-brand-review.v",
                    "saccade-ui-source.v",
                    "saccade-tesseract.v",
                    "saccade-ui-review.v",
                    "saccade-perf-plan.v",
                    "saccade-perf-pairs.v",
                    "saccade-vector-buffer.v",
                    "saccade-motion-vectors.v",
                    "saccade-motion-review.v",
                    "saccade-asset-views.v",
                    "saccade-asset-view-report.v",
                ] {
                    let supported = if prefix == "saccade-watermark.v" {
                        3
                    } else {
                        1
                    };
                    if crate::report_links::original_schema(actual)
                        .strip_prefix(prefix)
                        .and_then(|v| v.parse::<u32>().ok())
                        .is_some_and(|v| v > supported)
                    {
                        return Err(CliError::new(
                            "version_skew",
                            format!(
                                "written by {actual}; installed saccade supports up to {prefix}{supported}, upgrade"
                            ),
                        ));
                    }
                }
            }
            for child in fields.values() {
                reject_newer_nested_schemas(child)?;
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                reject_newer_nested_schemas(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn read_value(path: &Path) -> Result<Value, CliError> {
    Ok(canonical::decode(
        &crate::root_policy::io::read(path).map_err(|e| CliError::io(e.to_string()))?,
    )?)
}
pub(crate) fn write_value(path: &Path, value: &Value) -> Result<(), CliError> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::new("unsafe_path", "refusing output symlink"));
    }
    crate::report_links::write(path, value)?;
    Ok(())
}
pub(crate) fn base_result(operation: &str) -> Value {
    json!({"schema":"saccade-result.v2", "operation":operation,"execution":"complete","measurement":"unknown","validity":"unknown","validity_reasons":[],"review":"pending","artifact":null,"counts":{},"entries":[],"next_actions":[],"limits":[],"errors":[],"page":{"omitted":0,"next_cursor":null}})
}
pub(crate) fn reference(path: &Path) -> Result<Value, CliError> {
    Ok(serde_json::to_value(ArtifactRef::from_file(
        path,
        &std::env::current_dir()
            .map_err(|e| CliError::io(e.to_string()))?
            .join("result.json"),
        false,
    )?)?)
}
pub(crate) fn bounded(mut value: Value, bytes: usize) -> Result<Value, CliError> {
    while serde_json::to_vec(&value)?.len() > bytes {
        if let Some(entries) = value["entries"].as_array_mut()
            && !entries.is_empty()
        {
            entries.pop();
            if let Some(failing) = value.get_mut("failing").and_then(Value::as_array_mut) {
                failing.pop();
            }
            let omitted = value["page"]["omitted"].as_u64().unwrap_or(0) + 1;
            value["page"]["omitted"] = json!(omitted);
        } else {
            return Err(CliError::new(
                "output_budget",
                "invariants exceed the text budget; inspect the full artifact",
            ));
        }
    }
    Ok(value)
}
/// Report input using the established report/evidence contract.
pub fn report_input(
    report: &crate::Report,
    report_file: &Path,
    name: &str,
    baseline: bool,
) -> Option<PathBuf> {
    let root = if baseline {
        report.baseline_dir.as_ref()
    } else {
        report.capture_dir.as_ref()
    }?;
    let root = crate::paths::resolve(root, report_file);
    Some(if root.is_file() {
        root
    } else {
        root.join(name)
    })
}
/// Case from report using the established report/evidence contract.
pub fn case_from_report(
    report: &crate::Report,
    report_file: &Path,
) -> Result<EvidenceCase, CliError> {
    let document = report_file.with_file_name("evidence.json");
    let entries = report
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    let measurement = Measurement::from_report(
        report_file,
        &document,
        entries.clone(),
        "native+flip.v1".into(),
        "saccade-config.v1".into(),
    )?;
    let mut inputs = Vec::new();
    for entry in &report.entries {
        for (role, path, hash) in [
            ("baseline", &entry.paths.baseline, &entry.baseline_sha256),
            ("capture", &entry.paths.capture, &entry.capture_sha256),
        ] {
            if let (Some(_path), Some(hash)) = (path, hash) {
                let input_path = report_input(report, report_file, &entry.name, role == "baseline")
                    .ok_or_else(|| {
                        CliError::new("invalid_evidence", "measurement has no source directory")
                    })?;
                let root = if role == "baseline" {
                    report.baseline_dir.as_deref()
                } else {
                    report.capture_dir.as_deref()
                }
                .map(|p| crate::paths::resolve(p, report_file))
                .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new(".")).to_owned());
                let root = if root.is_file() {
                    root.parent().unwrap_or(Path::new(".")).to_owned()
                } else {
                    root
                };
                let mut sidecars = Vec::new();
                let mut directories = Vec::new();
                let mut current = input_path.parent();
                while let Some(dir) = current {
                    if !dir.starts_with(&root) {
                        break;
                    }
                    directories.push(dir.to_owned());
                    if dir == root {
                        break;
                    }
                    current = dir.parent();
                }
                directories.reverse();
                for dir in directories {
                    let file = dir.join(&report.config.meta.name);
                    if file.is_file() {
                        sidecars.push(ArtifactRef {
                            path: lexical_record(
                                &file,
                                document.parent().unwrap_or(Path::new(".")),
                            ),
                            sha256: Digest::of_bytes(
                                &crate::root_policy::io::read(&file)
                                    .map_err(|e| CliError::io(e.to_string()))?,
                            ),
                        });
                    }
                }
                if let Some(stem) = input_path.file_stem().and_then(|s| s.to_str()) {
                    let file =
                        input_path.with_file_name(format!("{stem}.{}", report.config.meta.name));
                    if file.is_file() {
                        sidecars.push(ArtifactRef {
                            path: lexical_record(
                                &file,
                                document.parent().unwrap_or(Path::new(".")),
                            ),
                            sha256: Digest::of_bytes(
                                &crate::root_policy::io::read(&file)
                                    .map_err(|e| CliError::io(e.to_string()))?,
                            ),
                        });
                    }
                }
                inputs.push(Input {
                    id: format!("{role}:{}", entry.name),
                    content: ArtifactRef {
                        path: lexical_record(
                            &input_path,
                            document.parent().unwrap_or(Path::new(".")),
                        ),
                        sha256: Digest::parse(format!("sha256:{hash}"))?,
                    },
                    sidecars,
                    native_samples: Availability::missing(
                        "Native interpretation is recorded in the authoritative measurement.",
                    ),
                    capture: Availability::missing("No typed capture context was supplied."),
                    build: Availability::missing("No build identity was supplied."),
                    provenance: Provenance {
                        source_roots: crate::paths::source_paths(&root)
                            .map_err(|e| CliError::io(e.to_string()))?,
                        ..Default::default()
                    },
                });
            }
        }
    }
    let valid = report.capture_validity();
    let validity = Validity {
        status: match valid.status {
            crate::meta::Validity::Valid => ValidityStatus::Valid,
            crate::meta::Validity::Invalid => ValidityStatus::Invalid,
            _ => ValidityStatus::Unknown,
        },
        reasons: valid.reasons,
    };
    let source_roots = inputs
        .iter()
        .flat_map(|i| i.provenance.source_roots.clone())
        .collect();
    let mut case = EvidenceCase {
        case_id: Digest::of_bytes(b""),
        inputs,
        measurement,
        scope: Scope {
            entries,
            exclusions: report.config.ignore.clone(),
        },
        effective_config: serde_json::from_value(serde_json::to_value(&report.config)?)?,
        calibration: Availability::missing("No calibration input was supplied."),
        validity,
        facts: Vec::new(),
        intent: Availability::missing("No declared intent was supplied."),
        requests: Vec::new(),
        proposals: Vec::new(),
        human_decisions: Vec::new(),
        next_actions: Vec::new(),
        limits: vec!["Measurement does not confer human approval.".into()],
        provenance: Provenance {
            source_roots,
            ..Default::default()
        },
    };
    for entry in &report.entries {
        case.facts.push(Fact {
            id: format!("native-equality:{}", entry.name),
            name: "native decoded-sample equality".into(),
            units: "boolean".into(),
            scope: Scope {
                entries: vec![entry.name.clone()],
                exclusions: Vec::new(),
            },
            source: FactSource::Measured,
            artifact: case.measurement.report.clone(),
            source_identity: case.measurement.semantic_sha256.clone(),
            value: entry.bit_identical.map_or_else(
                || Availability::missing("No complete decoded pair was measured."),
                |v| Availability::Available {
                    value: FactValue::Boolean(v),
                },
            ),
            depends_on_model_observation: false,
            observation_refs: Vec::new(),
        });
    }
    case.refresh_id()?;
    Ok(case)
}
/// Case for result using the established report/evidence contract.
pub fn case_for_result(
    report: &crate::Report,
    report_file: &Path,
) -> Result<EvidenceCase, CliError> {
    let file = report_file.with_file_name("evidence.json");
    if file.is_file() {
        let doc = Document::read(&file)?;
        if let Artifact::Case(case) = doc.artifact {
            if case.measurement.semantic_sha256 != Measurement::report_identity(report)? {
                return Err(CliError::new("stale_evidence", "case and report differ"));
            }
            return Ok(*case);
        }
    }
    case_from_report(report, report_file)
}
/// Lexical record using the established report/evidence contract.
pub fn lexical_record(path: &Path, base: &Path) -> String {
    let source = path.components().collect::<Vec<_>>();
    let root = base.components().collect::<Vec<_>>();
    let shared = source.iter().zip(&root).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return crate::paths::portable(path);
    }
    let mut parts = vec!["..".to_owned(); root.len() - shared];
    parts.extend(
        source[shared..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}
/// Persist case using the established report/evidence contract.
pub fn persist_case(
    report: &crate::Report,
    report_file: &Path,
    args: &super::IntentOptions,
) -> Result<(), CliError> {
    if report.entries.is_empty() {
        return Ok(());
    }
    let mut case = case_from_report(report, report_file)?;
    if case.inputs.is_empty() {
        return Ok(());
    }
    let source = report_file.with_file_name("evidence.json");
    let dir = report_file.parent().unwrap_or(Path::new("."));
    if let Some(bundled) = crate::render::bundle::measured_case(report, dir)? {
        for (index, input) in case.inputs.iter_mut().enumerate() {
            let copy = bundled
                .inputs
                .iter()
                .find(|i| i.id == input.id)
                .ok_or_else(|| CliError::new("invalid_evidence", "missing portable input"))?;
            input.content = copy.content.clone();
            for (sidecar_index, sidecar) in input.sidecars.iter_mut().enumerate() {
                sidecar.verify(&source)?;
                let bytes =
                    crate::root_policy::io::read(crate::paths::resolve(&sidecar.path, &source))
                        .map_err(|e| CliError::io(e.to_string()))?;
                let target = dir.join(format!("assets/input-{index}-sidecar-{sidecar_index}.json"));
                std::fs::write(&target, bytes).map_err(|e| CliError::io(e.to_string()))?;
                *sidecar = ArtifactRef::from_file(&target, &source, false)?;
            }
        }
    }
    if let Some(file) = &args.intent_file {
        let value = read_value(file)?;
        let mut intent: Intent =
            if value.get("schema").and_then(Value::as_str) == Some(crate::intent::SCHEMA) {
                let visual: crate::intent::VisualIntent = serde_json::from_value(value)?;
                visual.validate(file)?;
                Intent {
                    id: "visual-intent".into(),
                    objective: visual.objective,
                    assurance: IntentAssurance::Structured,
                    expected_changes: Vec::new(),
                    invariants: vec!["no change elsewhere".into()],
                    criteria: Vec::new(),
                    source: None,
                    mask_sources: Vec::new(),
                    provenance: Provenance::default(),
                }
            } else {
                serde_json::from_value(value)?
            };
        intent.assurance = IntentAssurance::Structured;
        let target = dir.join("assets/intent.json");
        std::fs::copy(file, &target).map_err(|e| CliError::io(e.to_string()))?;
        if let Ok(visual) = serde_json::from_value::<crate::intent::VisualIntent>(read_value(file)?)
        {
            visual.validate(file)?;
            for change in &visual.changes {
                if let Some(mask) = &change.mask {
                    let source_mask = file.parent().unwrap_or(Path::new(".")).join(mask);
                    let bundled_mask = target.parent().unwrap_or(dir).join(mask);
                    if let Some(parent) = bundled_mask.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| CliError::io(e.to_string()))?;
                    }
                    std::fs::copy(&source_mask, &bundled_mask)
                        .map_err(|e| CliError::io(e.to_string()))?;
                    intent.mask_sources.push(ArtifactRef::from_file(
                        &bundled_mask,
                        &source,
                        false,
                    )?);
                }
            }
        }
        intent.source = Some(ArtifactRef::from_file(&target, &source, false)?);
        case.intent = Availability::Available { value: intent };
    } else if let Some(text) = &args.intent {
        case.intent = Availability::Available {
            value: Intent {
                id: "declared-intent".into(),
                objective: text.clone(),
                assurance: IntentAssurance::Text,
                expected_changes: Vec::new(),
                invariants: Vec::new(),
                criteria: Vec::new(),
                source: None,
                mask_sources: Vec::new(),
                provenance: Provenance::default(),
            },
        };
    }
    if let Some(file) = &args.changes_file {
        let changes: Vec<DeclaredChange> = serde_json::from_value(read_value(file)?)?;
        if let Availability::Available { value: intent } = &mut case.intent {
            intent.expected_changes = changes;
        } else {
            return Err(CliError::usage(
                "--changes-file requires --intent or --intent-file",
            ));
        }
    }
    case.refresh_id()?;
    case.validate()?;
    write_value(
        &source,
        &serde_json::to_value(Document::new(Artifact::Case(Box::new(case))))?,
    )?;
    crate::render::render_html(report, dir)?;
    Ok(())
}
/// Visual intent using the established report/evidence contract.
pub fn visual_intent(
    args: &super::IntentOptions,
) -> Result<Option<(crate::intent::VisualIntent, PathBuf)>, CliError> {
    let Some(path) = &args.intent_file else {
        return Ok(None);
    };
    let value = read_value(path)?;
    if value.get("schema").and_then(Value::as_str) != Some(crate::intent::SCHEMA) {
        return Ok(None);
    }
    let intent: crate::intent::VisualIntent = serde_json::from_value(value)?;
    intent.validate(path)?;
    Ok(Some((intent, path.clone())))
}
/// Verify visual intent using the established report/evidence contract.
pub fn verify_visual_intent(
    report: &crate::Report,
    out: &Path,
    intent: Option<&(crate::intent::VisualIntent, PathBuf)>,
) -> Result<bool, CliError> {
    let result_file = out.join(crate::intent::RESULT_FILE);
    let Some((declaration, path)) = intent else {
        if result_file.exists() {
            std::fs::remove_file(&result_file).map_err(|e| CliError::io(e.to_string()))?;
        }
        return Ok(false);
    };
    let verification = crate::intent::verify(declaration, path, report);
    write_value(&result_file, &serde_json::to_value(&verification)?)?;
    let summary = format!(
        "<section aria-label=\"Intent verification\"><h2>Intent verification</h2><p>Matched: {}; unexpected: {}; missing: {}; unmeasurable: {}. <a href=\"{}\">Full deterministic findings</a>.</p></section>",
        verification.matched.len(),
        verification.unexpected.len(),
        verification.missing.len(),
        verification.unmeasurable.len(),
        crate::intent::RESULT_FILE
    );
    let html_path = out.join("index.html");
    let html = std::fs::read_to_string(&html_path).map_err(|e| CliError::io(e.to_string()))?;
    std::fs::write(
        &html_path,
        html.replacen("<main>", &format!("<main>{summary}"), 1),
    )
    .map_err(|e| CliError::io(e.to_string()))?;
    Ok(!verification.unexpected.is_empty()
        || !verification.missing.is_empty()
        || !verification.unmeasurable.is_empty())
}
pub(crate) fn check_no_symlinks(baseline_dir: &Path, rel: &Path) -> Result<(), CliError> {
    let mut cur = baseline_dir.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match std::fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CliError::new(
                    "unsafe_path",
                    format!("refusing to write through symlink {}", cur.display()),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => return Err(CliError::io(format!("inspecting {}: {e}", cur.display()))),
        }
    }
    Ok(())
}
