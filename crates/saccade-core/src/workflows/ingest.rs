//! Generic screenshot ingest: named vendors are adapters over the same report engine.
use super::CommandError as CliError;
pub use super::engine_ingest::{Format, run_engine};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    entries: Vec<Snapshot>,
    #[serde(default)]
    inventory: Option<crate::inventory::Manifest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    test_id: String,
    #[serde(default)]
    case_id: Option<String>,
    #[serde(default)]
    dom_regions: Option<crate::localized::DomMetadata>,
    #[serde(default)]
    ui_sources: Option<[crate::ui_review::Source; 2]>,
    project: String,
    browser: String,
    viewport: Option<[u32; 2]>,
    expected: String,
    actual: String,
    diff: Option<String>,
}

/// Resolve a regular screenshot inside its declared input root.
pub fn contained_source(root: &Path, relative: &Path) -> Result<PathBuf, CliError> {
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

/// Measured ingest artifacts, preserved coverage failures and deferred console warnings.
pub struct IngestResult {
    /// Full comparison report.
    pub report: crate::Report,
    /// Output report directory.
    pub report_dir: PathBuf,
    /// CLI-compatible diagnostics, never printed by the library.
    pub warnings: Vec<String>,
    /// Declared capture coverage is incomplete.
    pub incomplete: bool,
    /// Optional capture coverage state.
    pub coverage: Option<String>,
}
/// Intake a bounded, content-declared screenshot reporter manifest and write its report.
pub fn run_playwright(
    manifest: &Path,
    out: &Path,
    threshold: Option<f64>,
    absolute: bool,
    policy: &super::signed_approval::Policy,
) -> Result<IngestResult, CliError> {
    if std::fs::symlink_metadata(out).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::usage("ingest --out must not be a symlink"));
    }
    let raw = std::fs::read(manifest).map_err(|e| CliError::io(e.to_string()))?;
    let mut source_manifest: Manifest = super::parse_contract(&raw, "saccade-playwright.v1")?;
    if source_manifest.schema != "saccade-playwright.v1"
        || (source_manifest.entries.is_empty() && source_manifest.inventory.is_none())
    {
        return Err(CliError::usage(
            "expected nonempty saccade-playwright.v1 manifest",
        ));
    }
    if out.exists()
        && std::fs::read_dir(out)
            .map_err(|e| CliError::io(e.to_string()))?
            .next()
            .is_some()
    {
        return Err(CliError::usage("ingest --out must be empty or absent"));
    }
    let mut seen = BTreeSet::new();
    let mut validated = Vec::new();
    for (index, entry) in source_manifest.entries.iter().enumerate() {
        if entry.test_id.is_empty() || !seen.insert((entry.test_id.clone(), entry.project.clone()))
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
            source(manifest, &entry.expected)?,
            source(manifest, &entry.actual)?,
            entry
                .diff
                .as_deref()
                .map(|d| source(manifest, d))
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
            let hash = crate::run::sha256_file(actual)?;
            if inventory.supplied.iter().any(|s| {
                s.entry.as_deref() == Some(&name) && s.capture_sha256.as_deref() != Some(&hash)
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
            let expected_hash = crate::run::sha256_file(expected)?;
            let (w, h) =
                image::image_dimensions(expected).map_err(|e| CliError::usage(e.to_string()))?;
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
            super::support::write_value(
                &dir.join(format!("{index:04}.json")),
                &serde_json::to_value(metadata)?,
            )?;
        }
        if let Some(sources) = &entry.ui_sources {
            let dir = out.join("ui-sources");
            std::fs::create_dir_all(&dir).map_err(|e| CliError::io(e.to_string()))?;
            for (side, (metadata, path)) in sources.iter().zip([expected, actual]).enumerate() {
                let hash = crate::run::sha256_file(path)?;
                let (w, h) =
                    image::image_dimensions(path).map_err(|e| CliError::usage(e.to_string()))?;
                metadata.validate(&hash, [w, h])?;
                super::support::write_value(
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
            super::support::write_value(&dir.join(&sidecar_name), &sidecar)?;
        }
        mapping.push(json!({"entry":name,"test_id":entry.test_id,"case_id":entry.case_id,"dom_regions":entry.dom_regions.as_ref().map(|_|format!("dom-regions/{index:04}.json")),"ui_sources":entry.ui_sources.as_ref().map(|_|[format!("ui-sources/{index:04}-0.json"),format!("ui-sources/{index:04}-1.json")]),"project":entry.project,"browser":entry.browser,"viewport":entry.viewport,"diff":diff.as_ref().map(|_|format!("playwright-diffs/{index:04}.png"))}));
    }
    super::support::write_value(
        &out.join("playwright-mapping.json"),
        &json!({"schema":"saccade-playwright-mapping.v1","entries":mapping}),
    )?;
    let mut config = crate::config::RunConfig {
        record_absolute_paths: absolute,
        ..Default::default()
    };
    if let Some(threshold) = threshold {
        config.default_threshold = threshold;
    }
    let report_dir = out.join("report");
    let report = policy.run(&baseline, &capture, &report_dir, &config, None)?;
    let warnings = super::run_warnings(&config, &report, &baseline);
    let mut coverage = None;
    let mut incomplete = false;
    if let Some(manifest) = &source_manifest.inventory {
        let report_bytes = std::fs::read(report_dir.join(crate::report::REPORT_FILE_NAME))
            .map_err(|e| CliError::io(e.to_string()))?;
        let inventory = inventory_value(manifest, &report, &raw, &report_bytes)?;
        incomplete = inventory.coverage != "complete";
        super::support::write_value(
            &out.join("inventory.json"),
            &serde_json::to_value(&inventory)?,
        )?;
        coverage = Some(inventory.coverage.clone());
    }
    Ok(IngestResult {
        report,
        report_dir,
        warnings,
        incomplete,
        coverage,
    })
}
fn inventory_value(
    manifest: &crate::inventory::Manifest,
    report: &crate::Report,
    manifest_bytes: &[u8],
    report_bytes: &[u8],
) -> Result<crate::inventory::Inventory, CliError> {
    let mut inventory = crate::inventory::reconcile(manifest, report)?;
    inventory.manifest_sha256 = format!("{:x}", Sha256::digest(manifest_bytes));
    inventory.report_sha256 = format!("{:x}", Sha256::digest(report_bytes));
    Ok(inventory)
}
