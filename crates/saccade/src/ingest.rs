//! CLI adapters for test-runner screenshot files. No test runner is executed.
use crate::agent::CliError;
use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(crate) struct IngestArgs {
    #[command(subcommand)]
    operation: IngestOperation,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod containment_tests {
    use super::*;
    #[test]
    fn manifest_paths_reject_parent_absolute_and_symlink_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("run");
        std::fs::create_dir(&root).unwrap();
        let outside = temp.path().join("outside.png");
        std::fs::write(&outside, b"image").unwrap();
        assert!(contained_source(&root, Path::new("../outside.png")).is_err());
        assert!(contained_source(&root, &outside).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("link.png")).unwrap();
            assert!(contained_source(&root, Path::new("link.png")).is_err());
        }
    }
}

#[derive(Subcommand)]
enum IngestOperation {
    /// Pair Blender render report category/ref images with category renders.
    Blender {
        root: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Pair Bevy screenshot-N.png files from two runs.
    Bevy {
        reference: PathBuf,
        capture: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Pair Unity Graphics Test Framework ReferenceImages and ActualImages.
    Unity {
        assets: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Read Unreal screenshot comparison result paths from JSON.
    Unreal {
        results: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Compare expected and actual Playwright screenshot attachments.
    Playwright {
        /// Manifest written by integrations/playwright/reporter.cjs.
        manifest: PathBuf,
        /// New directory for paired inputs and the comparison report.
        #[arg(long)]
        out: PathBuf,
        /// Print the bounded comparison result.
        #[arg(long)]
        json: bool,
        /// FLIP threshold for the comparison.
        #[arg(long)]
        threshold: Option<f64>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    entries: Vec<Snapshot>,
    #[serde(default)]
    inventory: Option<saccade_core::inventory::Manifest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    test_id: String,
    #[serde(default)]
    case_id: Option<String>,
    #[serde(default)]
    dom_regions: Option<saccade_core::localized::DomMetadata>,
    #[serde(default)]
    ui_sources: Option<[saccade_core::ui_review::Source; 2]>,
    project: String,
    browser: String,
    viewport: Option<[u32; 2]>,
    expected: String,
    actual: String,
    diff: Option<String>,
}

pub(crate) fn contained_source(root: &Path, relative: &Path) -> Result<PathBuf, CliError> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(CliError::usage(format!(
            "ingest path must stay under {}: {}",
            root.display(),
            relative.display()
        )));
    }
    let root = std::fs::canonicalize(root).map_err(|e| CliError::io(e.to_string()))?;
    let path =
        std::fs::canonicalize(root.join(relative)).map_err(|e| CliError::io(e.to_string()))?;
    if !path.starts_with(&root) {
        return Err(CliError::usage(format!(
            "ingest path escapes root {}: {}",
            root.display(),
            relative.display()
        )));
    }
    Ok(path)
}

fn source(manifest: &Path, relative: &str) -> Result<Option<PathBuf>, CliError> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(CliError::usage("unsafe Playwright attachment path"));
    }
    let parent = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let root = std::fs::canonicalize(parent).map_err(|e| CliError::io(e.to_string()))?;
    let target = root.join(relative);
    let mut ancestor = target.as_path();
    loop {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| CliError::usage("unsafe attachment path"))?;
            }
            Err(e) => return Err(CliError::io(e.to_string())),
        }
    }
    let resolved = std::fs::canonicalize(ancestor).map_err(|e| CliError::io(e.to_string()))?;
    if !resolved.starts_with(&root) {
        return Err(CliError::usage("Playwright attachment escapes root"));
    }
    if ancestor != target {
        return Ok(None);
    }
    if !resolved.is_file() {
        return Err(CliError::usage(
            "Playwright attachment is not a regular file",
        ));
    }
    Ok(Some(resolved))
}

fn copy_image(from: &Path, to: &Path, inventory_mode: bool) -> Result<(), CliError> {
    if !inventory_mode {
        image::image_dimensions(from)
            .map_err(|e| CliError::usage(format!("invalid Playwright screenshot: {e}")))?;
    }
    std::fs::copy(from, to).map_err(|e| CliError::io(e.to_string()))?;
    Ok(())
}

pub(crate) fn run(args: IngestArgs, absolute: bool) -> Result<u8, CliError> {
    match args.operation {
        IngestOperation::Blender { root, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Blender(root),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Bevy {
            reference,
            capture,
            out,
            json,
        } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Bevy(reference, capture),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Unity { assets, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Unity(assets),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Unreal { results, out, json } => crate::engine_ingest::run(
            crate::engine_ingest::Format::Unreal(results),
            &out,
            json,
            absolute,
        ),
        IngestOperation::Playwright {
            manifest,
            out,
            json,
            threshold,
        } => {
            if std::fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(CliError::usage("ingest --out must not be a symlink"));
            }
            let raw = std::fs::read(&manifest).map_err(|e| CliError::io(e.to_string()))?;
            let mut source_manifest: Manifest =
                crate::parse_contract(&raw, "saccade-playwright.v1")?;
            if source_manifest.schema != "saccade-playwright.v1"
                || (source_manifest.entries.is_empty() && source_manifest.inventory.is_none())
            {
                return Err(CliError::usage(
                    "expected nonempty saccade-playwright.v1 manifest",
                ));
            }
            if out.exists()
                && std::fs::read_dir(&out)
                    .map_err(|e| CliError::io(e.to_string()))?
                    .next()
                    .is_some()
            {
                return Err(CliError::usage("ingest --out must be empty or absent"));
            }
            let mut seen = BTreeSet::new();
            let mut validated = Vec::new();
            for (index, entry) in source_manifest.entries.iter().enumerate() {
                if entry.test_id.is_empty()
                    || !seen.insert((entry.test_id.clone(), entry.project.clone()))
                {
                    return Err(CliError::usage(
                        "Playwright test IDs must be nonempty and unique per project",
                    ));
                }
                if let Some(inventory) = &source_manifest.inventory {
                    let name = format!("{index:04}.png");
                    let id = entry
                        .case_id
                        .as_deref()
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| CliError::usage("inventoried snapshot case_id missing"))?;
                    if !inventory.expected.iter().any(|e| e.case_id == id)
                        || !inventory
                            .supplied
                            .iter()
                            .any(|s| s.case_id == id && s.entry.as_deref() == Some(&name))
                        || inventory
                            .supplied
                            .iter()
                            .any(|s| s.entry.as_deref() == Some(&name) && s.case_id != id)
                    {
                        return Err(CliError::usage(
                            "snapshot case_id contradicts inventory filename assignment",
                        ));
                    }
                }
                validated.push((
                    source(&manifest, &entry.expected)?,
                    source(&manifest, &entry.actual)?,
                    entry
                        .diff
                        .as_deref()
                        .map(|d| source(&manifest, d))
                        .transpose()?
                        .flatten(),
                ));
            }
            let baseline = out.join("baseline");
            let capture = out.join("capture");
            let diffs = out.join("playwright-diffs");
            for dir in [&baseline, &capture, &diffs] {
                std::fs::create_dir_all(dir).map_err(|e| CliError::io(e.to_string()))?;
            }
            let mut mapping = Vec::new();
            for (index, (entry, (expected, actual, diff))) in source_manifest
                .entries
                .iter()
                .zip(validated.iter())
                .enumerate()
            {
                let name = format!("{index:04}.png");
                let (Some(expected), Some(actual)) = (expected, actual) else {
                    if let Some(inventory) = &mut source_manifest.inventory {
                        for s in &mut inventory.supplied {
                            if s.entry.as_deref() == Some(&name) {
                                s.state = "missing".into();
                            }
                        }
                        continue;
                    }
                    return Err(CliError::usage("Playwright attachment missing"));
                };
                if let Some(inventory) = &source_manifest.inventory {
                    let hash = saccade_core::run::sha256_file(actual)?;
                    if inventory.supplied.iter().any(|s| {
                        s.entry.as_deref() == Some(&name)
                            && s.capture_sha256.as_deref() != Some(&hash)
                    }) {
                        return Err(CliError::usage("snapshot hash contradicts inventory"));
                    }
                }
                copy_image(
                    expected,
                    &baseline.join(&name),
                    source_manifest.inventory.is_some(),
                )?;
                copy_image(
                    actual,
                    &capture.join(&name),
                    source_manifest.inventory.is_some(),
                )?;
                if let Some(diff) = diff {
                    copy_image(
                        diff,
                        &diffs.join(&name),
                        source_manifest.inventory.is_some(),
                    )?;
                }
                if let Some(metadata) = &entry.dom_regions {
                    let expected_hash = saccade_core::run::sha256_file(expected)?;
                    let (w, h) = image::image_dimensions(expected)
                        .map_err(|e| CliError::usage(e.to_string()))?;
                    if metadata.schema != "saccade-dom-regions.v1"
                        || metadata.reference_sha256 != expected_hash
                        || metadata.dimensions != [w, h]
                    {
                        return Err(CliError::usage(
                            "DOM geometry does not match reference screenshot",
                        ));
                    }
                    let dir = out.join("dom-regions");
                    std::fs::create_dir_all(&dir).map_err(|e| CliError::io(e.to_string()))?;
                    crate::local_cmd::write_value(
                        &dir.join(format!("{index:04}.json")),
                        &serde_json::to_value(metadata)?,
                    )?;
                }
                if let Some(sources) = &entry.ui_sources {
                    let dir = out.join("ui-sources");
                    std::fs::create_dir_all(&dir).map_err(|e| CliError::io(e.to_string()))?;
                    for (side, (metadata, path)) in
                        sources.iter().zip([expected, actual]).enumerate()
                    {
                        let hash = saccade_core::run::sha256_file(path)?;
                        let (w, h) = image::image_dimensions(path)
                            .map_err(|e| CliError::usage(e.to_string()))?;
                        metadata.validate(&hash, [w, h])?;
                        crate::local_cmd::write_value(
                            &dir.join(format!("{index:04}-{side}.json")),
                            &serde_json::to_value(metadata)?,
                        )?;
                    }
                }
                let sidecar = json!({
                    "playwright_test_id":entry.test_id,
                    "playwright_project":entry.project,
                    "playwright_browser":entry.browser,
                    "playwright_viewport_width":entry.viewport.map(|v|v[0]),
                    "playwright_viewport_height":entry.viewport.map(|v|v[1])
                });
                let sidecar_name = format!("{index:04}.saccade-meta.json");
                for dir in [&baseline, &capture] {
                    crate::local_cmd::write_value(&dir.join(&sidecar_name), &sidecar)?;
                }
                mapping.push(json!({"entry":name,"test_id":entry.test_id,"case_id":entry.case_id,"dom_regions":entry.dom_regions.as_ref().map(|_|format!("dom-regions/{index:04}.json")),"ui_sources":entry.ui_sources.as_ref().map(|_|[format!("ui-sources/{index:04}-0.json"),format!("ui-sources/{index:04}-1.json")]),"project":entry.project,"browser":entry.browser,"viewport":entry.viewport,"diff":diff.as_ref().map(|_|format!("playwright-diffs/{index:04}.png"))}));
            }
            crate::local_cmd::write_value(
                &out.join("playwright-mapping.json"),
                &json!({"schema":"saccade-playwright-mapping.v1","entries":mapping}),
            )?;
            let mut config = saccade_core::config::RunConfig {
                record_absolute_paths: absolute,
                ..Default::default()
            };
            if let Some(threshold) = threshold {
                config.default_threshold = threshold;
            }
            let report_dir = out.join("report");
            let report = saccade_core::run::run(&baseline, &capture, &report_dir, &config)?;
            let mut incomplete = false;
            if let Some(manifest) = &source_manifest.inventory {
                let report_bytes =
                    std::fs::read(report_dir.join(saccade_core::report::REPORT_FILE_NAME))
                        .map_err(|e| CliError::io(e.to_string()))?;
                let inventory =
                    crate::inventory_cmd::value(manifest, &report, &raw, &report_bytes)?;
                incomplete = inventory.coverage != "complete";
                crate::local_cmd::write_value(
                    &out.join("inventory.json"),
                    &serde_json::to_value(&inventory)?,
                )?;
                if !json {
                    crate::emit(&format!(
                        "capture coverage: {}; see inventory.json\n",
                        inventory.coverage
                    ))?;
                }
            }
            crate::emit_run(&report, &report_dir, json, absolute)?;
            Ok(u8::from(report.is_regression() || incomplete))
        }
    }
}
