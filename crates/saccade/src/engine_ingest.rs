//! Documented renderer test layouts mapped into saccade image pairs.
use crate::agent::CliError;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) enum Format {
    Blender(PathBuf),
    Bevy(PathBuf, PathBuf),
    Unity(PathBuf),
    Unreal(PathBuf),
}

struct Pair {
    key: String,
    expected: Option<PathBuf>,
    actual: Option<PathBuf>,
    diff: Option<PathBuf>,
    metadata: Value,
}

fn images(root: &Path) -> Result<BTreeMap<String, PathBuf>, CliError> {
    fn visit(
        root: &Path,
        here: &Path,
        found: &mut BTreeMap<String, PathBuf>,
    ) -> Result<(), CliError> {
        for entry in std::fs::read_dir(here).map_err(|e| CliError::io(e.to_string()))? {
            let entry = entry.map_err(|e| CliError::io(e.to_string()))?;
            let kind = entry.file_type().map_err(|e| CliError::io(e.to_string()))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                visit(root, &entry.path(), found)?;
            } else if kind.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                let file = entry.path();
                let relative = file
                    .strip_prefix(root)
                    .map_err(|e| CliError::io(e.to_string()))?;
                found.insert(relative.to_string_lossy().replace('\\', "/"), file);
            }
        }
        Ok(())
    }
    if !root.is_dir() {
        return Err(CliError::usage(format!(
            "missing image directory: {}",
            root.display()
        )));
    }
    let mut found = BTreeMap::new();
    visit(root, root, &mut found)?;
    Ok(found)
}

fn pair_dirs(reference: &Path, capture: &Path, prefix: &str) -> Result<Vec<Pair>, CliError> {
    let expected = images(reference)?;
    let actual = images(capture)?;
    let keys: BTreeSet<_> = expected.keys().chain(actual.keys()).cloned().collect();
    Ok(keys
        .into_iter()
        .map(|key| Pair {
            expected: expected.get(&key).cloned(),
            actual: actual.get(&key).cloned(),
            diff: None,
            metadata: json!({"ingest_format":prefix}),
            key,
        })
        .collect())
}

fn select(format: Format) -> Result<(String, Vec<Pair>), CliError> {
    match format {
        Format::Blender(root) => {
            // Blender's render_report.py, test_get_images():
            // https://raw.githubusercontent.com/blender/blender/main/tests/python/modules/render_report.py
            let all = images(&root)?;
            let mut pairs = Vec::new();
            for (key, actual) in &all {
                if key.contains("/ref/")
                    || key.contains("/diff/")
                    || key.starts_with("ref/")
                    || key.starts_with("diff/")
                {
                    continue;
                }
                let Some((category, name)) = key.rsplit_once('/') else {
                    continue;
                };
                let expected = all.get(&format!("{category}/ref/{name}")).cloned();
                let diff = all
                    .get(&format!(
                        "{category}/diff/{}.diff_color.png",
                        name.trim_end_matches(".png")
                    ))
                    .cloned();
                pairs.push(Pair {
                    key: key.clone(),
                    expected,
                    actual: Some(actual.clone()),
                    diff,
                    metadata: json!({"ingest_format":"blender","category":category}),
                });
            }
            for (key, expected) in &all {
                if let Some((category, name)) = key.split_once("/ref/") {
                    let actual_key = format!("{category}/{name}");
                    if !all.contains_key(&actual_key) {
                        pairs.push(Pair {
                            key: actual_key,
                            expected: Some(expected.clone()),
                            actual: None,
                            diff: None,
                            metadata: json!({"ingest_format":"blender","category":category}),
                        });
                    }
                }
            }
            Ok(("blender".into(), pairs))
        }
        Format::Bevy(reference, capture) => {
            // Bevy's screenshot example writes ./screenshot-N.png:
            // https://bevy.org/examples/window/screenshot/
            let pairs = pair_dirs(&reference, &capture, "bevy")?
                .into_iter()
                .filter(|p| {
                    Path::new(&p.key).file_name().is_some_and(|n| {
                        let n = n.to_string_lossy();
                        n.starts_with("screenshot-")
                            && n.ends_with(".png")
                            && !n[11..n.len() - 4].is_empty()
                            && n[11..n.len() - 4].chars().all(|c| c.is_ascii_digit())
                    })
                })
                .collect();
            Ok(("bevy".into(), pairs))
        }
        Format::Unity(assets) => {
            // Unity's Graphics Test Framework documentation defines Assets/
            // ReferenceImages and Assets/ActualImages with ColorSpace/Platform/GraphicsAPI:
            // https://github.com/Unity-Technologies/com.unity.testframework.graphics/blob/master/Documentation~/com.unity.testframework.graphics.md
            let mut pairs = pair_dirs(
                &assets.join("ReferenceImages"),
                &assets.join("ActualImages"),
                "unity",
            )?;
            let references = images(&assets.join("ReferenceImages"))?;
            let mut fallback_used = BTreeSet::new();
            for pair in &mut pairs {
                if pair.expected.is_none() && pair.actual.is_some() {
                    let path = Path::new(&pair.key);
                    if let Some(parent) = path.parent().and_then(Path::parent) {
                        let fallback = parent
                            .join(path.file_name().unwrap_or_default())
                            .to_string_lossy()
                            .replace('\\', "/");
                        pair.expected = references.get(&fallback).cloned();
                        if pair.expected.is_some() {
                            fallback_used.insert(fallback);
                        }
                    }
                }
            }
            pairs.retain(|p| p.actual.is_some() || !fallback_used.contains(&p.key));
            Ok(("unity".into(), pairs))
        }
        Format::Unreal(results) => {
            // FImageComparisonResult exposes portable approved/incoming/diff paths:
            // https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Developer/ScreenShotComparisonTools/FImageComparisonResult
            let raw = std::fs::read(&results).map_err(|e| CliError::io(e.to_string()))?;
            let value: Value = serde_json::from_slice(&raw)?;
            let records = value.as_array().cloned().unwrap_or_else(|| vec![value]);
            let mut pairs = Vec::new();
            let parent = results.parent().unwrap_or(Path::new("."));
            for (index, record) in records.iter().enumerate() {
                let path = |name: &str| {
                    record
                        .get(name)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(|s| parent.join(s))
                };
                let expected = path("ReportApprovedFilePath");
                let actual = path("ReportIncomingFilePath").or_else(|| path("IncomingFilePath"));
                if expected.is_none() && actual.is_none() {
                    continue;
                }
                pairs.push(Pair { key:format!("{index:04}.png"), expected, actual, diff:path("ReportComparisonFilePath"), metadata:json!({"ingest_format":"unreal","screenshot_path":record.get("ScreenshotPath").and_then(Value::as_str).unwrap_or(""),"platform":record.get("SourcePlatform").and_then(Value::as_str).unwrap_or(""),"rhi":record.get("SourceRHI").and_then(Value::as_str).unwrap_or("")}) });
            }
            Ok(("unreal".into(), pairs))
        }
    }
}

pub(crate) fn run(
    format: Format,
    out: &Path,
    json_output: bool,
    absolute: bool,
) -> Result<u8, CliError> {
    if out.exists() || std::fs::symlink_metadata(out).is_ok() {
        return Err(CliError::usage("engine ingest output must be absent"));
    }
    let (name, pairs) = select(format)?;
    if pairs.is_empty() {
        return Err(CliError::usage("no documented image pairs were found"));
    }
    let base = out.join("baseline");
    let cap = out.join("capture");
    let diffs = out.join("source-diffs");
    for dir in [&base, &cap, &diffs] {
        std::fs::create_dir_all(dir).map_err(|e| CliError::io(e.to_string()))?;
    }
    let mut mapping = Vec::new();
    for pair in pairs {
        for (source, dir) in [
            (&pair.expected, &base),
            (&pair.actual, &cap),
            (&pair.diff, &diffs),
        ] {
            if let Some(source) = source {
                let source =
                    std::fs::canonicalize(source).map_err(|e| CliError::io(e.to_string()))?;
                image::image_dimensions(&source)
                    .map_err(|e| CliError::usage(format!("invalid {name} image: {e}")))?;
                let destination = dir.join(&pair.key);
                std::fs::create_dir_all(destination.parent().unwrap_or(dir))
                    .map_err(|e| CliError::io(e.to_string()))?;
                std::fs::copy(&source, destination).map_err(|e| CliError::io(e.to_string()))?;
            }
        }
        let relative = Path::new(&pair.key);
        let sidecar_name = format!(
            "{}.saccade-meta.json",
            relative.file_stem().unwrap_or_default().to_string_lossy()
        );
        for dir in [&base, &cap] {
            let sidecar = dir.join(relative).with_file_name(&sidecar_name);
            if dir.join(relative).is_file() {
                crate::local_cmd::write_value(&sidecar, &pair.metadata)?;
            }
        }
        mapping.push(json!({"entry":pair.key,"expected":pair.expected.map(|p|p.to_string_lossy().into_owned()),"actual":pair.actual.map(|p|p.to_string_lossy().into_owned()),"diff":pair.diff.map(|p|p.to_string_lossy().into_owned())}));
    }
    crate::local_cmd::write_value(
        &out.join("ingest-mapping.json"),
        &json!({"schema":"saccade-engine-ingest.v1","format":name,"entries":mapping}),
    )?;
    let config = saccade_core::config::RunConfig {
        fail_on_new: true,
        record_absolute_paths: absolute,
        ..Default::default()
    };
    let report_dir = out.join("report");
    let report = saccade_core::run::run(&base, &cap, &report_dir, &config)?;
    crate::emit_run(&report, &report_dir, json_output, absolute)?;
    Ok(u8::from(report.is_regression()))
}
