//! User-owned mappings of retained historical evidence, never inferred plans.
use crate::capture::{self, Code, Record, Report};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// One canonical field and the decision used to obtain it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FieldEvidence {
    /// Canonical destination, including array index where applicable.
    pub field: String,
    /// mapped, defaulted, unavailable, or hashed_at_check_time.
    pub state: String,
    /// Source file and path, without producer data values.
    pub source: Option<String>,
    /// Exact encoded-byte digest computed during this check, never acquisition time.
    pub sha256: Option<String>,
}
/// A missing historical contract requirement, distinct from contradictory evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Unavailable {
    /// Stable requirement code.
    pub code: String,
    /// Canonical field that cannot be established.
    pub field: String,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct Source {
    path: String,
    file: Option<String>,
    default: Option<Value>,
    values: Option<BTreeMap<String, String>>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct Rows {
    source: Source,
    fields: BTreeMap<String, Source>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct Mapping {
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-capture-legacy-map.v1")))]
    schema: String,
    record: String,
    hash_policy: HashPolicy,
    absent: Absent,
    fields: BTreeMap<String, Source>,
    expected: Rows,
    acquisitions: Rows,
    retry_records: Option<Source>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum HashPolicy {
    Recorded,
    ComputeNow,
    Unavailable,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum Absent {
    Unavailable,
}
fn relative(s: &str) -> bool {
    crate::paths::safe_relative_name(s)
}
fn tokens(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.split('.')
            .all(|p| !p.is_empty() && !p.contains(['*', '[', ']', '/', '\\']))
}
fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    if let Some(value) = v.get(path) {
        return Some(value);
    }
    let mut current = v;
    for token in path.split('.') {
        current = if let Some(array) = current.as_array() {
            array.get(token.parse::<usize>().ok()?)?
        } else {
            current.get(token)?
        };
    }
    Some(current)
}
fn put(v: &mut Value, path: &str, value: Value) {
    let mut current = v;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current[part] = value;
            return;
        }
        if current.get(part).is_none() {
            current[part] = json!({});
        }
        current = &mut current[part];
    }
}
fn unavailable(report: &mut Report, field: &str, code: &str) {
    if !report
        .unavailable
        .iter()
        .any(|u| u.field == field && u.code == code)
    {
        report.unavailable.push(Unavailable {
            code: code.into(),
            field: field.into(),
        });
    }
    report.conformant = false;
}
fn validate_mapping(mapping: &Mapping) -> Result<(), ()> {
    let validate = |fields: &BTreeMap<String, Source>, allowed: &[&str]| -> Result<(), ()> {
        if fields.len() > 128 {
            return Err(());
        }
        for (dest, source) in fields {
            if !tokens(dest)
                || !tokens(&source.path)
                || source.file.as_ref().is_some_and(|f| !relative(f))
                || !allowed
                    .iter()
                    .any(|a| dest == a || dest.starts_with(&format!("{a}.")))
                || fields
                    .keys()
                    .any(|other| other != dest && dest.starts_with(&format!("{other}.")))
                || (source.values.is_some()
                    && !matches!(dest.as_str(), "producer" | "completion" | "status"))
                || source
                    .values
                    .as_ref()
                    .is_some_and(|v| v.is_empty() || v.len() > 32)
                || (source.default.is_some()
                    && !matches!(dest.as_str(), "producer" | "error" | "details"))
            {
                return Err(());
            }
        }
        Ok(())
    };
    validate(&mapping.fields, &["producer", "run_id", "completion"])?;
    validate(
        &mapping.expected.fields,
        &["id", "settings", "fingerprint", "clock_domain"],
    )?;
    validate(
        &mapping.acquisitions.fields,
        &[
            "id",
            "status",
            "error",
            "image",
            "settings",
            "fingerprint",
            "clock",
            "details",
        ],
    )?;
    for source in [&mapping.expected.source, &mapping.acquisitions.source]
        .into_iter()
        .chain(mapping.retry_records.iter())
    {
        if !tokens(&source.path)
            || source.file.as_ref().is_some_and(|f| !relative(f))
            || source.default.is_some()
            || source.values.is_some()
        {
            return Err(());
        }
    }
    Ok(())
}
struct Reader<'a> {
    root: &'a Path,
    documents: BTreeMap<String, Value>,
    projected_bytes: usize,
}
impl Reader<'_> {
    fn document(&mut self, name: &str) -> Result<Option<Value>, ()> {
        if !relative(name) {
            return Err(());
        }
        if let Some(v) = self.documents.get(name) {
            return Ok(Some(v.clone()));
        }
        if self.documents.len() >= 32 {
            return Err(());
        }
        let v = match capture::bounded_read(&self.root.join(name), capture::RECORD_LIMIT) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| ())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Null,
            Err(_) => return Err(()),
        };
        self.documents.insert(name.into(), v.clone());
        Ok((!v.is_null()).then_some(v))
    }
    fn source(
        &mut self,
        source: &Source,
        context: &Value,
        dest: &str,
        report: &mut Report,
    ) -> Result<Option<Value>, ()> {
        if !tokens(&source.path) {
            return Err(());
        }
        let document = if let Some(file) = &source.file {
            self.document(file)?
        } else {
            Some(context.clone())
        };
        let found = document
            .as_ref()
            .and_then(|v| lookup(v, &source.path))
            .filter(|v| !v.is_null())
            .cloned();
        let found = if let (Some(value), Some(values)) = (found.as_ref(), source.values.as_ref()) {
            let key = match value {
                Value::String(s) => s.clone(),
                Value::Bool(_) | Value::Number(_) => value.to_string(),
                _ => return Err(()),
            };
            Some(Value::String(values.get(&key).ok_or(())?.clone()))
        } else {
            found
        };
        // Only non-evidentiary defaults are allowed. Plans, statuses, clocks and identities must be retained.
        if source.default.is_some() && !matches!(dest, "producer" | "error" | "details") {
            return Err(());
        }
        let (value, state) = if found.is_some() {
            (found, "mapped")
        } else if source.default.is_some() {
            (source.default.clone(), "defaulted")
        } else {
            (None, "unavailable")
        };
        self.projected_bytes +=
            dest.len() + source.path.len() + source.file.as_ref().map_or(0, String::len) + 64;
        if let Some(v) = &value {
            self.projected_bytes += serde_json::to_vec(v).map_err(|_| ())?.len();
        }
        // Charge field metadata as well as selected arrays and projected values.
        if self.projected_bytes > capture::RECORD_LIMIT as usize {
            return Err(());
        }
        report.fields.push(FieldEvidence {
            field: dest.into(),
            state: state.into(),
            sha256: None,
            source: Some(format!(
                "{}:{}",
                source.file.as_deref().unwrap_or("record"),
                source.path
            )),
        });
        Ok(value)
    }
    fn fields(
        &mut self,
        fields: &BTreeMap<String, Source>,
        context: &Value,
        prefix: &str,
        report: &mut Report,
    ) -> Result<Value, ()> {
        if fields.len() > 128 {
            return Err(());
        }
        let mut value = json!({});
        for (dest, source) in fields {
            if !tokens(dest)
                || fields
                    .keys()
                    .any(|other| other != dest && dest.starts_with(&format!("{other}.")))
            {
                return Err(());
            }
            let before = report.fields.len();
            if let Some(v) = self.source(source, context, dest, report)? {
                put(&mut value, dest, v);
            }
            report.fields[before].field = format!("{prefix}{dest}");
        }
        Ok(value)
    }
    fn rows(
        &mut self,
        rows: &Rows,
        context: &Value,
        dest: &str,
        report: &mut Report,
    ) -> Result<Vec<Value>, ()> {
        let Some(value) = self.source(&rows.source, context, dest, report)? else {
            unavailable(report, dest, &format!("{dest}_unavailable"));
            return Ok(Vec::new());
        };
        let array = value.as_array().ok_or(())?;
        if array.len() > capture::ITEM_LIMIT {
            return Err(());
        }
        array
            .iter()
            .enumerate()
            .map(|(i, row)| self.fields(&rows.fields, row, &format!("{dest}.{i}."), report))
            .collect()
    }
}
fn identity_schema(row: &mut Value, prefix: &str, report: &mut Report) {
    // A contract discriminator is formatting, not retained acquisition evidence.
    if let Some(fp) = row.get_mut("fingerprint").and_then(Value::as_object_mut)
        && !fp.contains_key("schema")
    {
        fp.insert("schema".into(), crate::arms::FINGERPRINT_SCHEMA.into());
        report.fields.push(FieldEvidence {
            field: format!("{prefix}fingerprint.schema"),
            state: "defaulted".into(),
            source: None,
            sha256: None,
        });
    }
}
fn requirements(value: &Value, names: &[(&str, &str)], prefix: &str, report: &mut Report) -> bool {
    let mut available = true;
    for (field, code) in names {
        if lookup(value, field).is_none_or(Value::is_null) {
            let field = format!("{prefix}{field}");
            unavailable(report, &field, code);
            if !report.fields.iter().any(|f| f.field == field) {
                report.fields.push(FieldEvidence {
                    field,
                    state: "unavailable".into(),
                    source: None,
                    sha256: None,
                });
            }
            available = false;
        }
    }
    available
}
/// Adapt one directory without writing records or images. Every outcome is labelled.
/// Invalid maps and unsafe files fail closed; missing source data stays unavailable.
pub fn conform_legacy(root: &Path, map: &Path) -> Report {
    let mut report = Report::new();
    report.provenance = "adapted_legacy".into();
    if adapt(root, map, &mut report).is_err() {
        report.fail(Code::InvalidRecord, None);
    }
    report
}
fn adapt(root: &Path, map_path: &Path, report: &mut Report) -> Result<(), ()> {
    let bytes = capture::bounded_read(map_path, capture::RECORD_LIMIT).map_err(|_| ())?;
    let mapping: Mapping = if map_path.extension().is_some_and(|e| e == "toml") {
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| ())?).map_err(|_| ())?
    } else {
        serde_json::from_slice(&bytes).map_err(|_| ())?
    };
    if mapping.schema != "saccade-capture-legacy-map.v1" || !relative(&mapping.record) {
        return Err(());
    }
    validate_mapping(&mapping)?;
    let _ = mapping.absent;
    let mut reader = Reader {
        root,
        documents: BTreeMap::new(),
        projected_bytes: 0,
    };
    let Some(document) = reader.document(&mapping.record)? else {
        report.fail(Code::RecordUnavailable, None);
        return Ok(());
    };
    let mut record = reader.fields(&mapping.fields, &document, "", report)?;
    if mapping
        .fields
        .keys()
        .any(|k| !matches!(k.as_str(), "producer" | "run_id" | "completion"))
    {
        return Err(());
    }
    let mut expected = reader.rows(&mapping.expected, &document, "expected", report)?;
    let mut acquired = reader.rows(&mapping.acquisitions, &document, "acquisitions", report)?;
    report.expected = expected.len();
    report.acquired = acquired.len();
    if let Some(source) = &mapping.retry_records {
        if let Some(v) = reader.source(source, &document, "retry_records", report)? {
            report.retry_records = v
                .as_array()
                .filter(|v| v.len() <= capture::ITEM_LIMIT)
                .ok_or(())?
                .clone();
        } else {
            unavailable(report, "retry_records", "retry_records_unavailable");
        }
    }
    let mut complete_plan = requirements(
        &record,
        &[
            ("producer", "producer_unavailable"),
            ("run_id", "run_id_unavailable"),
            ("completion", "completion_unavailable"),
        ],
        "",
        report,
    );
    let identity_fields = [
        "fingerprint.schema",
        "fingerprint.producer.binary",
        "fingerprint.producer.build",
        "fingerprint.inputs.identity",
        "fingerprint.run.mode",
        "fingerprint.run.env",
        "fingerprint.run.readiness",
        "fingerprint.run.session",
    ];
    for (i, plan) in expected.iter_mut().enumerate() {
        for field in ["settings", "fingerprint"] {
            if plan
                .get(field)
                .is_some_and(|v| !v.is_null() && !v.is_object())
            {
                return Err(());
            }
        }
        identity_schema(plan, &format!("expected.{i}."), report);
        let names: Vec<_> = identity_fields
            .iter()
            .map(|f| (*f, "planned_identity_unavailable"))
            .collect();
        complete_plan &= requirements(plan, &names, &format!("expected.{i}."), report);
        complete_plan &= requirements(
            plan,
            &[
                ("id", "planned_slot_unavailable"),
                ("settings", "planned_settings_unavailable"),
                ("fingerprint", "planned_identity_unavailable"),
                ("clock_domain", "planned_clock_unavailable"),
            ],
            &format!("expected.{i}."),
            report,
        );
    }
    if record.get("completion").and_then(Value::as_str) == Some("partial") {
        report.fail(Code::PartialRun, None);
    }
    for (i, actual) in acquired.iter_mut().enumerate() {
        for field in ["settings", "fingerprint", "clock", "image", "details"] {
            if actual
                .get(field)
                .is_some_and(|v| !v.is_null() && !v.is_object())
            {
                return Err(());
            }
        }
        let id = actual.get("id").and_then(Value::as_str);
        match actual.get("status").and_then(Value::as_str) {
            Some("failed") => report.fail(Code::AcquisitionFailed, id),
            Some("skipped") => report.fail(Code::PartialRun, id),
            Some("captured") => {
                if actual.get("error").is_some_and(|v| !v.is_null()) {
                    report.fail(Code::AcquisitionFailed, id);
                }
            }
            Some(_) => return Err(()),
            None => {}
        }
        let prefix = format!("acquisitions.{i}.");
        identity_schema(actual, &prefix, report);
        requirements(
            actual,
            &[
                ("id", "observed_slot_unavailable"),
                ("status", "acquisition_status_unavailable"),
            ],
            &prefix,
            report,
        );
        if actual.get("status").and_then(Value::as_str) == Some("captured") {
            requirements(
                actual,
                &[
                    ("settings", "observed_settings_unavailable"),
                    ("fingerprint", "observed_identity_unavailable"),
                    ("clock", "observed_clock_unavailable"),
                    ("image.path", "image_path_unavailable"),
                ],
                &prefix,
                report,
            );
            let names: Vec<_> = identity_fields
                .iter()
                .map(|f| (*f, "observed_identity_unavailable"))
                .collect();
            requirements(actual, &names, &prefix, report);
            requirements(
                actual,
                &[
                    ("clock.domain", "observed_clock_unavailable"),
                    ("clock.run_id", "observed_clock_unavailable"),
                    ("clock.start_ns", "observed_clock_unavailable"),
                    ("clock.end_ns", "observed_clock_unavailable"),
                ],
                &prefix,
                report,
            );
            if report
                .unavailable
                .iter()
                .any(|u| u.code == "observed_clock_unavailable" && u.field.starts_with(&prefix))
            {
                actual["clock"] = Value::Null;
            }
            if let Some(image) = actual.get("image")
                && let Some(path) = image.get("path").and_then(Value::as_str)
            {
                let id = actual.get("id").and_then(Value::as_str);
                if !relative(path) {
                    report.fail(Code::UnsafeImagePath, id);
                } else if matches!(mapping.hash_policy, HashPolicy::Recorded)
                    && image.get("sha256").is_some_and(|v| !v.is_null())
                {
                    let image: capture::Image =
                        serde_json::from_value(image.clone()).map_err(|_| ())?;
                    if let Some(code) = capture::image_code(root, &image) {
                        report.fail(code, id);
                    }
                } else if matches!(
                    mapping.hash_policy,
                    HashPolicy::Unavailable | HashPolicy::Recorded
                ) && let Err(e) =
                    capture::bounded_read(&root.join(path), capture::IMAGE_LIMIT)
                {
                    report.fail(
                        if e.kind() == std::io::ErrorKind::NotFound {
                            Code::MissingImage
                        } else {
                            Code::ImageUnavailable
                        },
                        id,
                    );
                }
            }
            match mapping.hash_policy {
                HashPolicy::Recorded => {
                    requirements(
                        actual,
                        &[("image.sha256", "acquisition_hash_unavailable")],
                        &prefix,
                        report,
                    );
                }
                HashPolicy::ComputeNow => {
                    unavailable(
                        report,
                        &format!("{prefix}image.sha256"),
                        "acquisition_hash_unavailable",
                    );
                    if let Some(path) = lookup(actual, "image.path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                    {
                        if !relative(&path) {
                            report.fail(
                                Code::UnsafeImagePath,
                                actual.get("id").and_then(Value::as_str),
                            );
                        } else {
                            match capture::bounded_read(&root.join(&path), capture::IMAGE_LIMIT) {
                                Ok(bytes) => {
                                    actual["image"]["sha256"] =
                                        format!("{:x}", Sha256::digest(bytes)).into();
                                    report.fields.push(FieldEvidence {
                                        field: format!("{prefix}image.sha256"),
                                        state: "hashed_at_check_time".into(),
                                        source: Some(path),
                                        sha256: actual["image"]["sha256"]
                                            .as_str()
                                            .map(str::to_owned),
                                    });
                                }
                                Err(e) => report.fail(
                                    if e.kind() == std::io::ErrorKind::NotFound {
                                        Code::MissingImage
                                    } else {
                                        Code::ImageUnavailable
                                    },
                                    actual.get("id").and_then(Value::as_str),
                                ),
                            }
                        }
                    }
                }
                HashPolicy::Unavailable => {
                    unavailable(
                        report,
                        &format!("{prefix}image.sha256"),
                        "acquisition_hash_unavailable",
                    );
                }
            }
        }
        // Null optional observations do not invent evidence; details grants no authority.
        for field in ["error", "image", "settings", "fingerprint", "clock"] {
            if actual.get(field).is_none() {
                actual[field] = Value::Null;
                report.fields.push(FieldEvidence {
                    field: format!("{prefix}{field}"),
                    state: "unavailable".into(),
                    source: None,
                    sha256: None,
                });
            }
        }
        if actual.get("details").is_none() {
            actual["details"] = json!({});
            report.fields.push(FieldEvidence {
                field: format!("{prefix}details"),
                state: "defaulted".into(),
                source: None,
                sha256: None,
            });
        }
        if matches!(mapping.hash_policy, HashPolicy::Unavailable)
            && actual.get("status").and_then(Value::as_str) == Some("captured")
        {
            actual["image"] = Value::Null;
        }
        if actual.get("image").is_some_and(|v| {
            !v.is_null()
                && (v.get("sha256").is_none_or(Value::is_null)
                    || v.get("path").is_none_or(Value::is_null))
        }) {
            actual["image"] = Value::Null;
        }
    }
    if !complete_plan
        || report.unavailable.iter().any(|u| {
            matches!(
                u.code.as_str(),
                "expected_unavailable"
                    | "acquisitions_unavailable"
                    | "planned_slot_unavailable"
                    | "acquisition_status_unavailable"
                    | "observed_slot_unavailable"
            )
        })
    {
        return Ok(());
    }
    record["schema"] = capture::RECORD_SCHEMA.into();
    record["expected"] = Value::Array(std::mem::take(&mut expected));
    record["acquisitions"] = Value::Array(acquired);
    let record: Record = serde_json::from_value(record).map_err(|_| ())?;
    let checked = capture::conform(&record, root);
    for finding in checked.findings {
        let missing = match finding.code {
            Code::ClockMismatch => "observed_clock_unavailable",
            Code::MissingIdentity => "observed_identity_unavailable",
            Code::SettingsMismatch => "observed_settings_unavailable",
            Code::MissingImage => "acquisition_hash_unavailable",
            _ => "",
        };
        let index = finding
            .id
            .as_ref()
            .and_then(|id| record.acquisitions.iter().position(|a| &a.id == id));
        if !missing.is_empty()
            && index.is_some_and(|i| {
                report.unavailable.iter().any(|u| {
                    (u.code == missing
                        || (finding.code == Code::MissingImage
                            && u.code == "image_path_unavailable"))
                        && u.field.starts_with(&format!("acquisitions.{i}."))
                })
            })
        {
            continue;
        }
        report.fail(finding.code, finding.id.as_deref());
    }
    Ok(())
}
/// Deterministic archive rows, limited to 4096 visited entries, 256 directories,
/// and depth 16. Symlink entries are rejected, never followed; no writes occur.
pub fn archive(root: &Path, map: &Path) -> Vec<(PathBuf, Report)> {
    match archive_inner(root, map) {
        Ok(rows) => rows,
        Err(reason) => {
            let mut report = Report::new();
            report.provenance = "adapted_legacy".into();
            report.fail(Code::InvalidRecord, None);
            report.fields.push(FieldEvidence {
                field: "archive".into(),
                state: "unavailable".into(),
                source: Some(reason),
                sha256: None,
            });
            vec![(root.to_owned(), report)]
        }
    }
}
fn archive_inner(root: &Path, map: &Path) -> Result<Vec<(PathBuf, Report)>, String> {
    // Validate map once before traversal, including its primary record filename.
    let bytes = capture::bounded_read(map, capture::RECORD_LIMIT).map_err(|_| "map unavailable")?;
    let mapping: Mapping = if map.extension().is_some_and(|e| e == "toml") {
        toml::from_str(std::str::from_utf8(&bytes).map_err(|_| "invalid map")?)
            .map_err(|_| "invalid map")?
    } else {
        serde_json::from_slice(&bytes).map_err(|_| "invalid map")?
    };
    if mapping.schema != "saccade-capture-legacy-map.v1" || !relative(&mapping.record) {
        return Err("invalid map".into());
    }
    validate_mapping(&mapping).map_err(|_| "invalid map")?;
    let mut rows = Vec::new();
    let dir = capture::safe_directory(root).map_err(|_| "archive unavailable or unsafe")?;
    let mut count = 0;
    visit(&dir, root, 0, &mapping.record, map, &mut count, &mut rows)?;
    if rows.is_empty() {
        rows.push((root.to_owned(), conform_legacy(root, map)));
    }
    Ok(rows)
}
fn visit(
    dir: &cap_std::fs::Dir,
    path: &Path,
    depth: usize,
    record: &str,
    map: &Path,
    count: &mut usize,
    rows: &mut Vec<(PathBuf, Report)>,
) -> Result<(), String> {
    use cap_fs_ext::DirExt;
    if depth > 16 {
        return Err("archive depth limit exceeded".into());
    }
    if dir.symlink_metadata(record).is_ok() {
        if rows.len() >= 256 {
            return Err("archive record limit exceeded".into());
        }
        rows.push((path.to_owned(), conform_legacy(path, map)));
    }
    let mut entries = Vec::new();
    for entry in dir.entries().map_err(|_| "archive unavailable")? {
        *count += 1;
        if *count > 4096 {
            return Err("archive entry limit exceeded".into());
        }
        entries.push(entry.map_err(|_| "archive unavailable")?);
    }
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let kind = entry.file_type().map_err(|_| "archive unavailable")?;
        if kind.is_symlink() {
            return Err("archive symlink refused".into());
        }
        if kind.is_dir() {
            let child = dir
                .open_dir_nofollow(entry.file_name())
                .map_err(|_| "archive unavailable or unsafe")?;
            visit(
                &child,
                &path.join(entry.file_name()),
                depth + 1,
                record,
                map,
                count,
                rows,
            )?;
        }
    }
    Ok(())
}
