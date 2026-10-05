//! Supplied camera-view renders joined to static geometry evidence.
//! Manifest and report types remain available without the graphics producer.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

/// Exact source/decoded geometry identity and producer's LOD label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Asset {
    /// Lowercase SHA256 of the source document bytes.
    pub document_sha256: String,
    /// Ordered decoded geometry identity from the geometry comparison.
    pub geometry_sha256: String,
    /// Human-readable LOD/cut identity, never inferred from image similarity.
    pub lod: String,
}
/// Common view matrices. Arrays are row-major and act on column vectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Camera {
    /// Must be row_major_column_vector_right_handed_clip_z_zero_to_one.
    pub convention: String,
    /// Placement of the already decoded world geometry into the render world.
    pub model_to_world: [f64; 16],
    /// Render world to camera transformation.
    pub world_to_view: [f64; 16],
    /// Camera to homogeneous clip coordinates, with declared Z range.
    pub projection: [f64; 16],
}
/// Renderer and environment identity shared by both assets and all views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Context {
    /// Producer name/version, retained alongside hashes.
    pub renderer: String,
    /// Renderer source revision/content identity.
    pub renderer_source_sha256: String,
    /// Exact executable identity asserted by the producer.
    pub renderer_binary_sha256: String,
    /// Render settings, including temporal sample/frame policy.
    pub settings_sha256: String,
    /// Lighting/environment content identity.
    pub lighting_sha256: String,
    /// Background content identity.
    pub background_sha256: String,
    /// Colour pipeline/profile/display transform identity.
    pub color_pipeline_sha256: String,
    /// asset_materials or override_materials.
    pub material_mode: String,
    /// Override material identity; required only in override mode.
    pub override_material_sha256: Option<String>,
    /// Image dimensions; no resizing is allowed.
    pub dimensions: [u32; 2],
}
/// Exact supplied image or fractional silhouette mask bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ImagePin {
    /// Local path inside the camera manifest directory.
    pub path: String,
    /// Lowercase SHA256 of the encoded file.
    pub sha256: String,
}
/// Producer assertion binding one render to camera, context and asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Render {
    /// Supplied render image.
    pub image: ImagePin,
    /// Optional L8 PNG object coverage, 0..255 including partial coverage.
    pub silhouette: Option<ImagePin>,
    /// Canonical camera object hash.
    pub camera_sha256: String,
    /// Canonical render context object hash.
    pub context_sha256: String,
    /// Corresponding asset document identity.
    pub asset_document_sha256: String,
    /// Corresponding decoded geometry identity.
    pub asset_geometry_sha256: String,
    /// Material content identity; may differ in asset_materials mode.
    pub materials_sha256: String,
    /// Ordered content identities of texture inputs, including normal maps.
    pub textures_sha256: Vec<String>,
}
/// One required viewpoint; absent renders remain part of the coverage denominator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct View {
    /// Stable unique view name.
    pub id: String,
    /// Exact common camera for this paired view.
    pub camera: Camera,
    /// Reference receipt, absent when acquisition failed or was omitted.
    pub reference: Option<Render>,
    /// Candidate receipt.
    pub candidate: Option<Render>,
}
/// Frozen finite-view acquisition declaration; does not invoke a renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Manifest {
    /// Contract identity.
    pub schema: String,
    /// Same unit declaration as the geometry report.
    pub unit: String,
    /// Reference/candidate assets, respectively.
    pub assets: [Asset; 2],
    /// Common controlled renderer environment.
    pub context: Context,
    /// Every required distinct camera, in producer-declared order.
    pub views: Vec<View>,
    /// FLIP viewing parameter.
    pub pixels_per_degree: f32,
    /// Per-view raw mean threshold.
    pub maximum_mean_flip: f64,
    /// Fractional absolute silhouette change threshold, if masks are supplied.
    pub maximum_silhouette_change: f64,
}
/// Fractional object coverage evidence, separate from camera-view coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Silhouette {
    /// Mean reference mask coverage over all pixels.
    pub reference_coverage: f64,
    /// Mean candidate mask coverage.
    pub candidate_coverage: f64,
    /// Mean absolute fractional coverage change.
    pub changed_fraction: f64,
    /// Bounds of nonzero mask differences, x/y/width/height.
    pub change_bounds: Option<[u32; 4]>,
}
/// Per-view raw evidence, including missing/rejected views.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Measurement {
    /// Corresponding declared view name.
    pub id: String,
    /// Canonical paired camera identity.
    pub camera_sha256: String,
    /// measured, missing_render, or rejected_render.
    pub state: String,
    /// Input/identity failures; an absent measurement is never a zero score.
    pub issues: Vec<String>,
    /// Unaligned full-frame FLIP, absent if the pair is invalid.
    pub flip: Option<crate::report::Metrics>,
    /// Row-major raw FLIP errors, allowing local defects to be inspected.
    pub error_map: Vec<f32>,
    /// Bounds of raw errors exceeding maximum_mean_flip (not a segmentation).
    pub high_error_bounds: Option<[u32; 4]>,
    /// Optional paired mask evidence.
    pub silhouette: Option<Silhouette>,
    /// Threshold outcome; unknown for unmeasured pairs.
    pub regression: Option<bool>,
}
/// Worst measured view selected by raw mean FLIP, with stable name tie breaking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WorstView {
    /// Stable view name.
    pub id: String,
    /// Raw mean FLIP.
    pub mean: f64,
    /// Raw maximum FLIP in that view.
    pub maximum: f64,
}
/// Finite-view coverage; never an angular or surface coverage bound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Coverage {
    /// All required cameras, including absent/rejected evidence.
    pub declared_views: usize,
    /// Both render receipts supplied, independent of validity.
    pub supplied_pairs: usize,
    /// Successfully measured pairs.
    pub measured_views: usize,
    /// Missing receipt or file pairs.
    pub missing_views: usize,
    /// Other rejected pairs.
    pub rejected_views: usize,
    /// measured_views / declared_views.
    pub fraction: f64,
}
/// Multi-view packet embedded additively in saccade-geometry.v1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Report {
    /// Contract identity.
    pub schema: String,
    /// Complete supplied declaration and receipts.
    pub manifest: Manifest,
    /// Canonical body identity of the manifest.
    pub manifest_sha256: String,
    /// Canonical common renderer/context identity.
    pub context_sha256: String,
    /// All views, including failures.
    pub views: Vec<Measurement>,
    /// Worst measured raw mean view, absent when none were measured.
    pub worst_view: Option<WorstView>,
    /// Explicit acquired/measured coverage.
    pub coverage: Coverage,
    /// True only with complete coverage and no declared-threshold regression.
    pub passed: bool,
    /// Limits of supplied, finite-view render evidence.
    pub limitations: Vec<String>,
}
fn invalid(message: impl std::fmt::Display) -> Error {
    Error::Config(format!("asset views: {message}"))
}
/// Hash a camera or context by the existing canonical JSON contract.
pub fn hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(crate::localized::digest(
        &crate::evidence::canonical::bytes(value).map_err(invalid)?,
    ))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invertible(m: &[f64; 16]) -> bool {
    if m.iter().any(|v| !v.is_finite() || v.abs() > 1e15) {
        return false;
    }
    let mut a = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = m[r * 4 + c];
        }
    }
    for c in 0..4 {
        let p = (c..4)
            .max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))
            .unwrap_or(c);
        if a[p][c].abs() < 1e-15 {
            return false;
        }
        a.swap(c, p);
        let pivot = a[c];
        for row in a.iter_mut().skip(c + 1) {
            let f = row[c] / pivot[c];
            for (value, p) in row.iter_mut().zip(pivot).skip(c) {
                *value -= f * p;
            }
        }
    }
    a.iter().flatten().all(|v| v.is_finite())
}
impl Manifest {
    /// Bind source and decoded mesh identities before considering supplied renders.
    pub fn validate(
        &self,
        documents: &[String; 2],
        geometry: &[String; 2],
        unit: &str,
    ) -> Result<()> {
        let c = &self.context;
        if self.schema != "saccade-asset-views.v1"
            || self.unit != unit
            || unit.trim().is_empty()
            || self.views.is_empty()
            || self.views.len() > 64
            || !self.pixels_per_degree.is_finite()
            || self.pixels_per_degree <= 0.0
            || !self.maximum_mean_flip.is_finite()
            || !(0.0..=1.0).contains(&self.maximum_mean_flip)
            || !self.maximum_silhouette_change.is_finite()
            || !(0.0..=1.0).contains(&self.maximum_silhouette_change)
            || c.renderer.trim().is_empty()
            || c.dimensions.iter().any(|&v| v == 0 || v > 2048)
            || u64::from(c.dimensions[0]) * u64::from(c.dimensions[1]) * self.views.len() as u64
                > 4 * 1024 * 1024
            || [
                &c.renderer_source_sha256,
                &c.renderer_binary_sha256,
                &c.settings_sha256,
                &c.lighting_sha256,
                &c.background_sha256,
                &c.color_pipeline_sha256,
            ]
            .iter()
            .any(|h| !valid_hash(h))
            || !(c.material_mode == "asset_materials" && c.override_material_sha256.is_none()
                || c.material_mode == "override_materials"
                    && c.override_material_sha256
                        .as_deref()
                        .is_some_and(valid_hash))
        {
            return Err(invalid(
                "invalid view plan, thresholds, dimensions or render context",
            ));
        }
        for i in 0..2 {
            let a = &self.assets[i];
            if !valid_hash(&a.document_sha256)
                || !valid_hash(&a.geometry_sha256)
                || a.lod.trim().is_empty()
                || a.document_sha256 != documents[i]
                || a.geometry_sha256 != geometry[i]
            {
                return Err(invalid(
                    "manifest does not bind the compared document/decoded geometry or LOD identity",
                ));
            }
        }
        let mut ids = BTreeSet::new();
        let mut cameras = BTreeSet::new();
        for v in &self.views {
            let camera = &v.camera;
            if v.id.trim().is_empty()
                || !ids.insert(&v.id)
                || camera.convention != "row_major_column_vector_right_handed_clip_z_zero_to_one"
                || !invertible(&camera.model_to_world)
                || !invertible(&camera.world_to_view)
                || !invertible(&camera.projection)
                || camera.model_to_world[12..] != [0.0, 0.0, 0.0, 1.0]
                || camera.world_to_view[12..] != [0.0, 0.0, 0.0, 1.0]
                || !cameras.insert({
                    // Positive homogeneous scale preserves division and clip inequalities.
                    // Keep receipt hashes tied to the original bytes; normalize coverage only.
                    let mut normalized = camera.clone();
                    let scale = camera
                        .projection
                        .iter()
                        .copied()
                        .map(f64::abs)
                        .fold(0.0, f64::max);
                    normalized.projection = camera.projection.map(|v| {
                        let v = v / scale;
                        if v == 0.0 { 0.0 } else { v }
                    });
                    hash(&normalized)?
                })
            {
                return Err(invalid(
                    "view IDs/cameras must be distinct, finite, nonsingular and use the declared matrix convention",
                ));
            }
        }
        Ok(())
    }
}
// Containment is checked even for absent files; symlinks cannot supply outside bytes.
fn contained(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid(
            "image paths must be local without parent components",
        ));
    }
    let root = std::fs::canonicalize(root).map_err(invalid)?;
    let full = root.join(p);
    if let Ok(actual) = std::fs::canonicalize(&full) {
        if !actual.starts_with(&root) {
            return Err(invalid("render path escapes manifest root"));
        }
        return Ok(actual);
    }
    // An absent child under an outside symlink is also an escape.
    let mut ancestor = full.parent();
    while let Some(p) = ancestor {
        if let Ok(actual) = std::fs::canonicalize(p) {
            if !actual.starts_with(&root) {
                return Err(invalid("missing render path escapes manifest root"));
            }
            break;
        }
        ancestor = p.parent();
    }
    Ok(full)
}
/// Measure a declared finite view set. Requires graphics for production only.
pub fn measure(
    manifest: Manifest,
    root: &Path,
    documents: &[String; 2],
    geometry: &[String; 2],
    unit: &str,
) -> Result<Report> {
    manifest.validate(documents, geometry, unit)?;
    // Validate every supplied path before reading any bytes, even on reduced builds.
    for v in &manifest.views {
        for r in [&v.reference, &v.candidate].into_iter().flatten() {
            contained(root, &r.image.path)?;
            if let Some(s) = &r.silhouette {
                contained(root, &s.path)?;
            }
        }
    }
    #[cfg(feature = "graphics")]
    {
        producer::measure(manifest, root)
    }
    #[cfg(not(feature = "graphics"))]
    {
        Err(Error::FeatureUnavailable {
            feature: "graphics",
        })
    }
}
#[cfg(feature = "graphics")]
mod producer;
