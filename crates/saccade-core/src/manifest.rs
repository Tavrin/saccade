//! Output discovery: one manifest per output directory, stable links to reports,
//! and loud failures when a link no longer resolves to the bytes it recorded.
//!
//! The manifest is built on the report identity the rest of the toolkit already
//! writes (`report_id`, `reports/index.jsonl`, see [`crate::report_links`]); it adds
//! no second identity scheme and no second index. Nothing here is retained or
//! written unless a caller asks for it, and nothing here grants approval: the
//! `approval` block is separate from every verdict, and "last good" is a different
//! role from "approved anchor".
use crate::{Error, Result, report_links};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

/// Schema of the per-directory manifest.
pub const MANIFEST_SCHEMA: &str = "saccade-manifest.v1";
/// Schema of a stable link to one report.
pub const LINK_SCHEMA: &str = "saccade-link.v1";
/// File name of the manifest inside an output directory.
pub const MANIFEST_FILE: &str = "saccade-manifest.json";
/// The shared report index, relative to an output directory.
pub const INDEX_REL: &str = "reports/index.jsonl";
/// Link failure: the recorded file moved or was removed.
pub const CODE_LINK_MISSING: &str = "link_missing";
/// Link failure: the file exists but its bytes changed since the link was made.
pub const CODE_STALE_LINK: &str = "stale_link";

const MAX_FILES: usize = 20_000;
const MAX_DEPTH: usize = 6;
const MAX_JSON_BYTES: u64 = 64 << 20;

/// Optional, explicitly declared references recorded next to (never inferred from)
/// verdicts.
#[derive(Debug, Default, Clone, Copy)]
pub struct Anchors<'a> {
    /// A baseline a human approved. Recorded only when declared.
    pub approved: Option<&'a Path>,
    /// The most recent passing run, as resolved by the history store. Not approval.
    pub last_good: Option<&'a Path>,
}

fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = std::fs::File::open(crate::paths::native(path))
        .map_err(crate::run::io_err(format!("reading {}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(crate::run::io_err(format!("reading {}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total += n as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

fn read_json(path: &Path) -> Option<Value> {
    let file = std::fs::File::open(crate::paths::native(path)).ok()?;
    let mut data = Vec::new();
    file.take(MAX_JSON_BYTES + 1).read_to_end(&mut data).ok()?;
    if data.len() as u64 > MAX_JSON_BYTES {
        return None;
    }
    serde_json::from_slice(&data).ok()
}

fn schema_of(value: &Value) -> Option<&str> {
    value.get("schema").and_then(Value::as_str)
}

/// Decide what a path is: a report directory, a JSON document, or a compact API
/// or CLI response envelope. Reads at most 64 MiB and never follows a symlink.
pub fn classify(path: &Path) -> Result<Value> {
    let meta = std::fs::symlink_metadata(crate::paths::native(path))
        .map_err(crate::run::io_err(format!("inspecting {}", path.display())))?;
    if meta.file_type().is_symlink() {
        return Err(Error::Config("refusing to classify a symlink".into()));
    }
    if meta.is_dir() {
        let manifest = path.join(MANIFEST_FILE).is_file();
        let indexed = path.join(INDEX_REL).is_file();
        let page = path.join("index.html").is_file()
            && path.join(crate::report::REPORT_FILE_NAME).is_file();
        let kind = if manifest || indexed || page {
            "report_directory"
        } else {
            "unrecognized_directory"
        };
        return Ok(json!({"kind":kind,"manifest":manifest,"report_index":indexed,"page":page}));
    }
    let Some(value) = read_json(path) else {
        return Ok(json!({"kind":"unrecognized_file"}));
    };
    let schema = schema_of(&value).map(str::to_owned);
    let kind = match schema.as_deref() {
        None => "json_document",
        Some(s) if s.starts_with("saccade-manifest.") => "manifest",
        Some(s) if s.starts_with("saccade-link.") => "link",
        Some(s)
            if s.starts_with("saccade-api-")
                || s.starts_with("saccade-result.")
                || s.starts_with("saccade-error.") =>
        {
            "api_response"
        }
        Some(_) => "json_document",
    };
    Ok(json!({
        "kind":kind,
        "schema":schema,
        "report_id":value.get("report_id").cloned().unwrap_or(Value::Null),
    }))
}

struct File {
    rel: String,
    sha256: String,
    bytes: u64,
    role: &'static str,
    media_type: &'static str,
    schema: Option<String>,
    report: Option<(String, Value)>,
}

fn media_and_role(rel: &str) -> (&'static str, &'static str) {
    let ext = rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "json" => ("application/json", "document"),
        "jsonl" => (
            "application/x-ndjson",
            if rel == INDEX_REL {
                "index"
            } else {
                "document"
            },
        ),
        "html" => ("text/html", "page"),
        "csv" => ("text/csv", "table"),
        "png" => ("image/png", "image"),
        "jpg" | "jpeg" => ("image/jpeg", "image"),
        "webp" => ("image/webp", "image"),
        "avif" => ("image/avif", "image"),
        "exr" => ("image/x-exr", "image"),
        "md" => ("text/markdown", "document"),
        _ => ("application/octet-stream", "other"),
    }
}

fn scan(dir: &Path) -> Result<(Vec<File>, usize)> {
    let mut files = Vec::new();
    let mut skipped_symlinks = 0;
    for entry in walkdir::WalkDir::new(crate::paths::native(dir))
        .follow_links(false)
        .max_depth(MAX_DEPTH)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|e| Error::Config(format!("walking output directory: {e}")))?;
        if entry.file_type().is_symlink() {
            skipped_symlinks += 1;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry
            .path()
            .strip_prefix(crate::paths::native(dir).as_ref())
        else {
            continue;
        };
        let rel = crate::paths::portable(relative);
        // The batch sentinel is coordination state, not an output artifact.
        // Reading it through another handle while batch holds its exclusive
        // byte-range lock fails on Windows. Never hash or reopen it here.
        if rel == MANIFEST_FILE || rel == ".batch-lock" {
            continue;
        }
        if files.len() >= MAX_FILES {
            return Err(Error::Config(format!(
                "output directory holds more than {MAX_FILES} files; point the manifest at a report directory"
            )));
        }
        let (sha256, bytes) = hash_file(entry.path())?;
        let (media_type, mut role) = media_and_role(&rel);
        let mut schema = if rel == "rows.jsonl" && dir.join("batch-run.json").is_file() {
            Some("saccade-batch-row.v1".to_owned())
        } else {
            None
        };
        let mut report = None;
        if media_type == "application/json"
            && let Some(value) = read_json(entry.path())
        {
            schema = schema_of(&value).map(str::to_owned);
            if let (Some(s), Some(id)) = (
                schema.as_deref(),
                value.get("report_id").and_then(Value::as_str),
            ) && report_links::is_report_schema(s)
            {
                role = "report";
                report = Some((id.to_owned(), value));
            }
        }
        files.push(File {
            rel,
            sha256,
            bytes,
            role,
            media_type,
            schema,
            report,
        });
    }
    Ok((files, skipped_symlinks))
}

fn anchor_ref(path: &Path, manifest: &Path) -> Result<Value> {
    if !path.is_file() {
        return Err(Error::Config(format!(
            "anchor {} is not a file",
            path.display()
        )));
    }
    let (sha256, _) = hash_file(path)?;
    let resolved = crate::paths::canonicalize(path)
        .map_err(crate::run::io_err(format!("resolving {}", path.display())))?;
    Ok(json!({
        "path": crate::paths::record(&resolved, manifest.parent().unwrap_or(Path::new(".")), false),
        "sha256": sha256,
    }))
}

/// Build the manifest of `dir` without writing it. Identical files are listed once
/// (`also_at` names the copies) and reports are keyed by `report_id`, so duplicates
/// of either never inflate the counts.
pub fn build(dir: &Path, anchors: Anchors<'_>) -> Result<Value> {
    if !dir.is_dir() {
        return Err(Error::Config(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let (files, skipped_symlinks) = scan(dir)?;
    let mut by_hash: BTreeMap<&str, Vec<&File>> = BTreeMap::new();
    for file in &files {
        by_hash.entry(&file.sha256).or_default().push(file);
    }
    let mut artifacts: Vec<Value> = by_hash
        .values()
        .map(|group| {
            let first = group[0];
            let mut value = json!({
                "path": first.rel,
                "sha256": first.sha256,
                "bytes": first.bytes,
                "role": first.role,
                "media_type": first.media_type,
                "also_at": group[1..].iter().map(|f| f.rel.clone()).collect::<Vec<_>>(),
            });
            if let Some(schema) = &first.schema {
                value["schema"] = json!(schema);
            }
            if let Some((id, _)) = &first.report {
                value["report_id"] = json!(id);
            }
            value
        })
        .collect();
    artifacts.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));

    #[derive(Default)]
    struct Acc {
        schema: Value,
        verdict: Value,
        refs: Value,
        local: Vec<(String, String)>,
        rows: usize,
        row_path: Option<String>,
    }
    let mut reports: BTreeMap<String, Acc> = BTreeMap::new();
    for file in &files {
        if let Some((id, value)) = &file.report {
            let acc = reports.entry(id.clone()).or_default();
            acc.schema = value["schema"].clone();
            acc.verdict = report_links::verdict_class(value);
            acc.refs = value
                .get("source_refs")
                .cloned()
                .unwrap_or_else(|| json!([]));
            acc.local.push((file.rel.clone(), file.sha256.clone()));
        }
    }
    let mut indexed_rows = 0;
    let index = dir.join(INDEX_REL);
    if index.is_file() {
        for row in report_links::export(&index)? {
            let Some(id) = row.get("report_id").and_then(Value::as_str) else {
                continue;
            };
            indexed_rows += 1;
            let acc = reports.entry(id.to_owned()).or_default();
            acc.rows += 1;
            if acc.local.is_empty() {
                acc.schema = row["report_schema"].clone();
                acc.verdict = row["verdict_class"].clone();
                acc.refs = row.get("source_refs").cloned().unwrap_or_else(|| json!([]));
                acc.row_path = row["report_path"].as_str().map(str::to_owned);
            }
        }
    }
    let duplicate_rows: usize = reports.values().map(|a| a.rows.saturating_sub(1)).sum();
    let report_rows: Vec<Value> = reports
        .into_iter()
        .map(|(id, acc)| {
            let local = acc.local.first();
            json!({
                "report_id": id,
                "schema": acc.schema,
                "verdict_class": acc.verdict,
                "source_refs": acc.refs,
                "local": local.is_some(),
                "path": local.map(|l| l.0.clone()).or(acc.row_path),
                "sha256": local.map(|l| l.1.clone()),
                "also_at": acc.local.iter().skip(1).map(|l| l.0.clone()).collect::<Vec<_>>(),
                "indexed_rows": acc.rows,
            })
        })
        .collect();

    let manifest_path = dir.join(MANIFEST_FILE);
    let approved = anchors
        .approved
        .map(|p| anchor_ref(p, &manifest_path))
        .transpose()?;
    let last_good = anchors
        .last_good
        .map(|p| anchor_ref(p, &manifest_path))
        .transpose()?;
    Ok(json!({
        "schema": MANIFEST_SCHEMA,
        "kind": "report_directory",
        "completion": "recorded",
        "approval": {
            "state": if approved.is_some() { "anchor_declared" } else { "none" },
            "approved_anchor": approved,
            "last_good": last_good,
            "note": "Completion and passing verdicts are never approval. Only a declared approved_anchor records one, and last_good is a different role.",
        },
        "counts": {
            "files": files.len(),
            "unique_artifacts": artifacts.len(),
            "duplicate_files": files.len() - artifacts.len(),
            "reports": report_rows.len(),
            "indexed_rows": indexed_rows,
            "duplicate_rows": duplicate_rows,
            "skipped_symlinks": skipped_symlinks,
        },
        "artifacts": artifacts,
        "reports": report_rows,
    }))
}

/// Replace `path` atomically. An existing file is overwritten only when it is a
/// document of `schema`; anything else is refused and left alone.
pub fn write_owned(path: &Path, schema: &str, value: &Value) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(Error::NotEmptyOutDir(format!(
                    "{} exists and is not a saccade file; refusing to overwrite it",
                    path.display()
                )));
            }
            if read_json(path).as_ref().and_then(schema_of) != Some(schema) {
                return Err(Error::NotEmptyOutDir(format!(
                    "{} exists and is not a {schema} document; refusing to overwrite it",
                    path.display()
                )));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(Error::Io {
                context: format!("inspecting {}", path.display()),
                source: e,
            });
        }
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(crate::run::io_err(format!("writing {}", path.display())))?;
    std::io::Write::write_all(&mut temp, &serde_json::to_vec_pretty(value)?)
        .map_err(crate::run::io_err(format!("writing {}", path.display())))?;
    temp.persist(path)
        .map_err(|e| Error::Config(format!("writing {}: {}", path.display(), e.error)))?;
    Ok(())
}

/// Build the manifest of `dir` and write it as [`MANIFEST_FILE`]. An existing
/// manifest is regenerated; any other file of that name is refused.
pub fn write(dir: &Path, anchors: Anchors<'_>) -> Result<(PathBuf, Value)> {
    let value = build(dir, anchors)?;
    let path = dir.join(MANIFEST_FILE);
    write_owned(&path, MANIFEST_SCHEMA, &value)?;
    Ok((path, value))
}

fn finding(code: &'static str, path: &str, message: String) -> Value {
    json!({"code":code,"path":path,"message":message})
}

fn check(document: &Path, recorded: &str, sha256: &str, findings: &mut Vec<Value>) {
    let resolved = crate::paths::resolve(recorded, document);
    if !resolved.is_file() {
        findings.push(finding(
            CODE_LINK_MISSING,
            recorded,
            format!(
                "{recorded} no longer exists at the recorded location; it was moved or removed"
            ),
        ));
        return;
    }
    match hash_file(&resolved) {
        Ok((now, _)) if now == sha256 => {}
        Ok(_) => findings.push(finding(
            CODE_STALE_LINK,
            recorded,
            format!(
                "{recorded} changed since it was recorded; the link no longer names the same bytes"
            ),
        )),
        Err(e) => findings.push(finding(CODE_LINK_MISSING, recorded, e.to_string())),
    }
}

/// Re-hash everything a manifest or link names and return every failure with its
/// stable code (`link_missing`, `stale_link`). An empty list means every recorded
/// artifact is where it was and has the same bytes. Files added since are not
/// failures. `target` is a report directory, a manifest or a link file.
pub fn verify(target: &Path) -> Result<Vec<Value>> {
    let file = if target.is_dir() {
        target.join(MANIFEST_FILE)
    } else {
        target.to_path_buf()
    };
    let value = read_json(&file).ok_or_else(|| {
        Error::Config(format!(
            "{} is not readable saccade JSON; build a manifest with `saccade manifest build DIR`",
            file.display()
        ))
    })?;
    let mut findings = Vec::new();
    match schema_of(&value) {
        Some(MANIFEST_SCHEMA | crate::coverage::MANIFEST_SCHEMA) => {
            if schema_of(&value) == Some(crate::coverage::MANIFEST_SCHEMA) {
                let declaration = crate::coverage::Declaration {
                    schema: crate::coverage::CASES_SCHEMA.into(),
                    axes: serde_json::from_value(value["axes"].clone())?,
                    cases: serde_json::from_value(value["cases"].clone())?,
                };
                declaration.validate()?;
                for case in &declaration.cases {
                    for reference in [
                        &case.baseline,
                        &case.capture,
                        &case.approved_anchor,
                        &case.last_good,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        check(&file, &reference.path, &reference.sha256, &mut findings);
                    }
                }
            }
            for artifact in value["artifacts"].as_array().into_iter().flatten() {
                let (Some(path), Some(sha)) =
                    (artifact["path"].as_str(), artifact["sha256"].as_str())
                else {
                    return Err(Error::Config(
                        "manifest artifact lacks path or sha256".into(),
                    ));
                };
                check(&file, path, sha, &mut findings);
                for copy in artifact["also_at"].as_array().into_iter().flatten() {
                    if let Some(copy) = copy.as_str() {
                        check(&file, copy, sha, &mut findings);
                    }
                }
            }
            for role in ["approved_anchor", "last_good"] {
                let anchor = &value["approval"][role];
                if let (Some(path), Some(sha)) =
                    (anchor["path"].as_str(), anchor["sha256"].as_str())
                {
                    check(&file, path, sha, &mut findings);
                }
            }
        }
        Some(LINK_SCHEMA) => {
            let (Some(path), Some(sha)) = (
                value["report"]["path"].as_str(),
                value["report"]["sha256"].as_str(),
            ) else {
                return Err(Error::Config("link lacks report path or sha256".into()));
            };
            check(&file, path, sha, &mut findings);
            if findings.iter().any(|f| f["code"] == CODE_LINK_MISSING)
                && let (Some(id), Some(manifest)) =
                    (value["report_id"].as_str(), value["manifest"].as_str())
                && let Some(moved) = read_json(&crate::paths::resolve(manifest, &file))
                && let Some(row) = moved["reports"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|r| r["report_id"] == id)
            {
                findings.push(finding(
                    CODE_LINK_MISSING,
                    path,
                    format!("the manifest still lists this report at {}", row["path"]),
                ));
            }
        }
        other => {
            return Err(Error::Config(format!(
                "expected a {MANIFEST_SCHEMA} or {LINK_SCHEMA} document, found {other:?}"
            )));
        }
    }
    Ok(findings)
}

/// Write a [`LINK_SCHEMA`] document at `out` that names one local report of the
/// directory's manifest by `report_id`, with its content hash. The manifest must
/// exist (`build` it first) so a link never points at an unindexed guess.
pub fn link(dir: &Path, report_id: &str, out: &Path) -> Result<Value> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let manifest = read_json(&manifest_path).ok_or_else(|| {
        Error::Config(format!(
            "{} has no readable manifest; run `saccade manifest build` first",
            dir.display()
        ))
    })?;
    let row = manifest["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| r["report_id"] == report_id)
        .ok_or_else(|| Error::Config(format!("no report with id {report_id} in the manifest")))?;
    let (Some(relative), true) = (row["path"].as_str(), row["local"] == true) else {
        return Err(Error::Config(format!(
            "report {report_id} is only indexed; its file is not in {}",
            dir.display()
        )));
    };
    let resolved = crate::paths::canonicalize(dir.join(relative))
        .map_err(crate::run::io_err(format!("resolving {relative}")))?;
    let base = out.parent().unwrap_or(Path::new("."));
    let (sha256, _) = hash_file(&resolved)?;
    let document = json!({
        "schema": LINK_SCHEMA,
        "report_id": report_id,
        "report": {"path": crate::paths::record(&resolved, base, false), "sha256": sha256},
        "manifest": crate::paths::record(&manifest_path, base, false),
    });
    write_owned(out, LINK_SCHEMA, &document)?;
    Ok(document)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn report_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let _scope = report_links::scope(vec![], None).unwrap();
        let value = json!({"schema":"saccade-timing-ab.v1","verdict":"pass","n":1});
        report_links::write(&tmp.path().join("a.json"), &value).unwrap();
        // Same bytes under another name: one artifact, one report.
        std::fs::copy(tmp.path().join("a.json"), tmp.path().join("copy.json")).unwrap();
        tmp
    }

    #[test]
    fn duplicates_are_listed_once_and_passing_is_not_approval() {
        let tmp = report_dir();
        let m = build(tmp.path(), Anchors::default()).unwrap();
        assert_eq!(m["counts"]["files"], 3); // a.json, copy.json, reports/index.jsonl
        assert_eq!(m["counts"]["unique_artifacts"], 2);
        assert_eq!(m["counts"]["reports"], 1);
        assert_eq!(m["reports"][0]["verdict_class"], "pass");
        assert_eq!(m["reports"][0]["indexed_rows"], 1);
        assert_eq!(m["approval"]["state"], "none");
        assert!(m["approval"]["approved_anchor"].is_null());
        assert!(m["approval"]["last_good"].is_null());
        // A last-good reference is a different role and still not approval.
        let good = tmp.path().join("a.json");
        let m = build(
            tmp.path(),
            Anchors {
                approved: None,
                last_good: Some(&good),
            },
        )
        .unwrap();
        assert_eq!(m["approval"]["state"], "none");
        assert!(m["approval"]["last_good"]["sha256"].is_string());
    }

    #[test]
    fn moved_and_changed_artifacts_fail_with_stable_codes_and_other_files_are_not_overwritten() {
        let tmp = report_dir();
        write(tmp.path(), Anchors::default()).unwrap();
        assert!(verify(tmp.path()).unwrap().is_empty());
        std::fs::write(tmp.path().join("a.json"), b"{}").unwrap();
        std::fs::remove_file(tmp.path().join("copy.json")).unwrap();
        let codes: Vec<_> = verify(tmp.path())
            .unwrap()
            .iter()
            .map(|f| f["code"].as_str().unwrap().to_owned())
            .collect();
        assert!(codes.contains(&CODE_STALE_LINK.to_owned()));
        assert!(codes.contains(&CODE_LINK_MISSING.to_owned()));
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join(MANIFEST_FILE), b"not ours").unwrap();
        assert!(write(other.path(), Anchors::default()).is_err());
        assert_eq!(
            std::fs::read(other.path().join(MANIFEST_FILE)).unwrap(),
            b"not ours"
        );
    }

    #[test]
    fn a_link_resolves_one_report_then_goes_stale() {
        let tmp = report_dir();
        let (_, m) = write(tmp.path(), Anchors::default()).unwrap();
        let id = m["reports"][0]["report_id"].as_str().unwrap().to_owned();
        let out = tmp.path().join("link.json");
        link(tmp.path(), &id, &out).unwrap();
        assert!(verify(&out).unwrap().is_empty());
        assert!(link(tmp.path(), "sha256:nope", &out).is_err());
        std::fs::write(tmp.path().join("a.json"), b"{}").unwrap();
        assert_eq!(verify(&out).unwrap()[0]["code"], CODE_STALE_LINK);
    }

    #[test]
    fn directories_json_documents_and_responses_are_told_apart() {
        let tmp = report_dir();
        assert_eq!(classify(tmp.path()).unwrap()["kind"], "report_directory");
        assert_eq!(
            classify(&tmp.path().join("a.json")).unwrap()["kind"],
            "json_document"
        );
        let api = tmp.path().join("api.json");
        std::fs::write(&api, br#"{"schema":"saccade-result.v2"}"#).unwrap();
        assert_eq!(classify(&api).unwrap()["kind"], "api_response");
    }
}
