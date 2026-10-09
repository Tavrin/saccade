//! Native layer predicates, shared by effect occupancy and comparison scope.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Selection rule evaluated in native units (ID PNGs retain integer values).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    /// Generic ID label glob from the manifest or dump dictionary.
    Labels {
        /// Explicit glob; no vocabulary is built in.
        pattern: String,
    },
    /// Exact native integer labels.
    Ids {
        /// Allowed labels.
        values: Vec<u32>,
    },
    /// Inclusive range in scalar/depth units.
    Range {
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
    /// Scalar greater than the declared threshold.
    Above {
        /// Threshold in native units.
        threshold: f64,
    },
    /// Nonzero scalar mask samples.
    Mask,
}
impl Predicate {
    /// Reject ambiguous or nonfinite predicates.
    pub fn validate(&self) -> Result<()> {
        let valid = match self {
            Self::Labels { pattern } => {
                !pattern.is_empty() && crate::config::compile_glob(pattern).is_ok()
            }
            Self::Ids { values } => !values.is_empty() && values.len() <= 65536,
            Self::Range { min, max } => min.is_finite() && max.is_finite() && min <= max,
            Self::Above { threshold } => threshold.is_finite(),
            Self::Mask => true,
        };
        if !valid {
            return Err(Error::Config("invalid layer predicate".into()));
        }
        Ok(())
    }
    /// Test a native scalar.
    pub fn matches(&self, value: f64) -> bool {
        match self {
            Self::Labels { .. } => false,
            Self::Ids { values } => values.iter().any(|v| f64::from(*v) == value),
            Self::Range { min, max } => value >= *min && value <= *max,
            Self::Above { threshold } => value > *threshold,
            Self::Mask => value != 0.0,
        }
    }
}
/// Parse the stable `NAME=PREDICATE` mask-spec grammar used by CLI and MCP.
///
/// Predicates are `id=U32[,U32...]`, `label=GLOB`, `material=GLOB` (an alias
/// for `label`), `mask`, `above=FINITE` or `range=FINITE,FINITE` (inclusive).
/// Names and patterns are case-sensitive; whitespace is not trimmed. Native IDs
/// are preserved. Invalid, empty and nonfinite predicates return [`Error::Config`].
/// This API parses and validates syntax; evaluating a label requires the supplied
/// layer dictionary and evaluating samples requires native scalar data.
///
/// ```
/// use saccade_core::evidence_quality::layers::{parse_layer_spec, Predicate};
/// let (name, rule) = parse_layer_spec("specimen=id=12,13")?;
/// assert_eq!(name, "specimen");
/// assert_eq!(rule, Predicate::Ids { values: vec![12, 13] });
/// # Ok::<(), saccade_core::Error>(())
/// ```
pub fn parse_layer_spec(spec: &str) -> Result<(String, Predicate)> {
    let (layer, p) = spec
        .split_once('=')
        .ok_or_else(|| Error::Config("mask layer requires NAME=PREDICATE".into()))?;
    if layer.is_empty() {
        return Err(Error::Config("layer name is empty".into()));
    }
    let pred = if let Some(ids) = p.strip_prefix("id=") {
        Predicate::Ids {
            values: ids
                .split(',')
                .map(|v| {
                    v.parse::<u32>()
                        .map_err(|_| Error::Config("ID must be a native unsigned integer".into()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        }
    } else if let Some(pattern) = p
        .strip_prefix("label=")
        .or_else(|| p.strip_prefix("material="))
    {
        Predicate::Labels {
            pattern: pattern.into(),
        }
    } else if p == "mask" {
        Predicate::Mask
    } else if let Some(v) = p.strip_prefix("above=") {
        Predicate::Above {
            threshold: v
                .parse()
                .map_err(|_| Error::Config("above requires a numeric threshold".into()))?,
        }
    } else if let Some(v) = p.strip_prefix("range=") {
        let (min, max) = v
            .split_once(',')
            .ok_or_else(|| Error::Config("range=min,max".into()))?;
        Predicate::Range {
            min: min
                .parse()
                .map_err(|_| Error::Config("invalid range".into()))?,
            max: max
                .parse()
                .map_err(|_| Error::Config("invalid range".into()))?,
        }
    } else {
        return Err(Error::Config(
            "predicate must be id=, label=, material=, mask, above= or range=".into(),
        ));
    };
    pred.validate()?;
    Ok((layer.into(), pred))
}
/// Scalar layer samples without display conversion.
pub fn scalar(path: &Path, dimensions: (u32, u32)) -> Result<Vec<f64>> {
    let img = super::image(path)?;
    if (img.width(), img.height()) != dimensions {
        return Err(Error::Config("layer dimensions differ".into()));
    }
    let values: Vec<f64> = match img {
        image::DynamicImage::ImageLuma8(i) => i.into_raw().into_iter().map(f64::from).collect(),
        image::DynamicImage::ImageLuma16(i) => i.into_raw().into_iter().map(f64::from).collect(),
        image::DynamicImage::ImageRgb32F(i) => i.pixels().map(|p| f64::from(p.0[0])).collect(),
        image::DynamicImage::ImageRgba32F(i) => i.pixels().map(|p| f64::from(p.0[0])).collect(),
        _ => {
            return Err(Error::Config(
                "scalar layer requires L8, L16 or floating R channel".into(),
            ));
        }
    };
    if values.iter().any(|v| !v.is_finite()) {
        return Err(Error::Config("nonfinite layer sample".into()));
    }
    Ok(values)
}

/// Generic image-adjacent capture manifest discriminator.
pub const SCHEMA: &str = "saccade-capture-layers.v1";
/// Native layer interpretation; supplied components have no implicit engine meaning.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Integer labels.
    Id,
    /// Native depth/scalar units.
    Depth,
    /// Nonzero mask.
    Mask,
    /// Arbitrary scalar component.
    Scalar,
    /// RGB colour/radiance component.
    Colour,
}
/// Explicit transfer function for RGB component layers.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColourSpace {
    /// Linear-light numeric RGB.
    #[default]
    Linear,
    /// sRGB samples, decoded to linear light for decomposition.
    Srgb,
}
/// One named layer in a sidecar manifest.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    /// Unique component name.
    pub name: String,
    /// Path relative to the sidecar directory.
    pub image: String,
    /// Native semantic kind.
    pub kind: Kind,
    /// Optional content hash; required by immutable trial binding if used there.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Native scalar/colour scale multiplier.
    #[serde(default = "scale_one")]
    pub scale: f64,
    /// Explicit colour encoding.
    #[serde(default)]
    pub colour_space: ColourSpace,
    /// Whether this is part of an additive RGB decomposition.
    #[serde(default)]
    pub additive: bool,
}
fn scale_one() -> f64 {
    1.0
}
/// Source-bound capture layers; the manifest must describe the supplied final image.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Optional native ID-to-label dictionary for generic label selection.
    #[serde(default)]
    pub labels: std::collections::BTreeMap<u32, String>,
    /// Must equal SCHEMA.
    pub schema: String,
    /// Final-image encoded SHA-256.
    pub image_sha256: String,
    /// Final-image width/height.
    pub dimensions: [u32; 2],
    /// Named ID/depth/mask/scalar/colour layers.
    pub layers: Vec<Layer>,
}
/// Side(s) used to define a comparison scope.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeMode {
    /// Union retains both disappearing and newly appearing occupancy.
    #[default]
    Union,
    /// Both selections must match.
    Intersection,
    /// Freeze the reference footprint.
    Baseline,
    /// Candidate footprint only, explicitly declared.
    Candidate,
}
/// Layer-based inclusion, applied to every deciding statistic.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerScope {
    /// Name in both manifests.
    pub layer: String,
    /// Native predicate.
    pub predicate: Predicate,
    /// How two footprints define inclusion.
    #[serde(default)]
    pub mode: ScopeMode,
}
/// Layer policy for ordinary colour comparison or a BufferSpec.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Generic screen-space dump relative to the capture parent, used if no ID manifest exists.
    pub dump: Option<String>,
    /// Optional sidecar filename in each image parent; otherwise `<filename>.layers.json`.
    #[cfg_attr(
        feature = "schema",
        schemars(
            description = "Optional sidecar filename in each image parent; otherwise <filename>.layers.json."
        )
    )]
    pub manifest: Option<String>,
    /// Optional layer inclusion predicate.
    pub scope: Option<LayerScope>,
    /// Decompose additive RGB layers into final-image delta plus residual.
    pub attribution: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            dump: None,
            manifest: None,
            scope: None,
            attribution: true,
        }
    }
}
impl Policy {
    /// Validate policy shape before input acquisition.
    pub fn validate(&self) -> Result<()> {
        if self
            .manifest
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.contains(['/', '\\']) || p == "." || p == "..")
        {
            return Err(Error::Config(
                "manifest must be a plain sidecar filename".into(),
            ));
        }
        if let Some(scope) = &self.scope {
            if scope.layer.is_empty() {
                return Err(Error::Config("scope layer is empty".into()));
            }
            scope.predicate.validate()?;
        }
        Ok(())
    }
}
/// Retained layer content and manifest identities used for scope/attribution.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Witness {
    /// Actual ID buffer or derived-from-dump source, with confidence.
    #[serde(default)]
    pub provenance: String,
    /// Bundled manifest path, when comparison wrote a portable report.
    #[serde(default)]
    pub manifest_path: Option<String>,
    /// Bundled exact layer bytes, keyed by layer name.
    #[serde(default)]
    pub paths: std::collections::BTreeMap<String, String>,
    /// Manifest encoded content hash.
    pub manifest_sha256: String,
    /// Named layer hashes, computed from retained bytes.
    pub layers: std::collections::BTreeMap<String, String>,
}
/// One additive component's observed difference in linear RGB.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDelta {
    /// Name from the supplied manifests.
    pub name: String,
    /// Signed per-channel mean delta.
    pub mean_signed_rgb: [f64; 3],
    /// Mean absolute component delta (all channels).
    pub mean_absolute: f64,
    /// Projection onto final delta / final squared norm; signed and may exceed one.
    pub projected_share: Option<f64>,
}
/// Algebraic component decomposition, with no causal or physical-model inference.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    /// Final difference in linear RGB.
    pub final_delta: ComponentDelta,
    /// Supplied additive components, ordered by name.
    pub components: Vec<ComponentDelta>,
    /// Final minus summed components, computed per pixel before averaging.
    pub residual: ComponentDelta,
}
/// Layer evidence added to the comparison entry.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerResult {
    /// Exact supplied policy.
    pub policy: Policy,
    /// Source-bound baseline layers.
    pub baseline: Witness,
    /// Source-bound candidate layers.
    pub candidate: Witness,
    /// Included pixel count; empty scopes fail.
    pub included_pixels: u64,
    /// Baseline selected count.
    pub baseline_pixels: u64,
    /// Candidate selected count.
    pub candidate_pixels: u64,
    /// Decomposition if requested and components present.
    pub attribution: Option<Attribution>,
}
/// Loaded, bounded layers and their provenance; no re-reading after validation.
pub struct Loaded {
    names: std::collections::BTreeMap<u32, String>,
    retained: std::collections::BTreeMap<String, Vec<u8>>,
    manifest_bytes: Vec<u8>,
    manifest: Manifest,
    images: std::collections::BTreeMap<String, image::DynamicImage>,
    witness: Witness,
}
/// Load a manifest bound to the final-image content and all its named layers.
pub fn load(path: &Path, policy: &Policy, dimensions: (u32, u32)) -> Result<Loaded> {
    load_inner(path, policy, dimensions, None)
}
/// Bind layers to the pixels already used by the comparison, refusing a changed source.
pub fn load_for_image(path: &Path, policy: &Policy, image: &image::DynamicImage) -> Result<Loaded> {
    load_inner(
        path,
        policy,
        (image.width(), image.height()),
        Some((image, false)),
    )
}
/// Bind native buffer layers to the exact source pixels decoded for measurement.
#[cfg(feature = "graphics")]
pub(crate) fn load_for_buffer(
    path: &Path,
    policy: &Policy,
    image: &image::DynamicImage,
    scalar: bool,
) -> Result<Loaded> {
    load_inner(
        path,
        policy,
        (image.width(), image.height()),
        Some((image, scalar)),
    )
}
fn load_inner(
    path: &Path,
    policy: &Policy,
    dimensions: (u32, u32),
    expected: Option<(&image::DynamicImage, bool)>,
) -> Result<Loaded> {
    policy.validate()?;
    let final_bytes = super::read(path, 128 << 20)?;
    if let Some((expected, scalar)) = expected {
        #[cfg(feature = "graphics")]
        let current = if scalar {
            crate::buffer::decode_r32f_bytes(&final_bytes, path)?
        } else {
            super::decode(&final_bytes, path)?
        };
        #[cfg(not(feature = "graphics"))]
        let current = {
            let _ = scalar;
            super::decode(&final_bytes, path)?
        };
        let matches = if expected.color() == image::ColorType::Rgba8 {
            current.to_rgba8() == expected.to_rgba8()
        } else {
            current.to_rgba32f() == expected.to_rgba32f()
        };
        if !matches {
            return Err(Error::Config(
                "capture-layer final image changed after comparison decode".into(),
            ));
        }
    }
    let root = path.parent().unwrap_or(Path::new("."));
    let name = policy.manifest.clone().unwrap_or_else(|| {
        format!(
            "{}.layers.json",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    if let Some(dump) = &policy.dump
        && !root.join(&name).exists()
    {
        let source = super::relative(root, dump)?;
        let dump_bytes = super::read(&source, 1 << 20)?;
        let (img, names) = super::dump::rasterize(&dump_bytes, dimensions)?;
        let mut encoded = std::io::Cursor::new(Vec::new());
        img.write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|source| Error::Encode {
                path: source_path(root),
                source,
            })?;
        let encoded = encoded.into_inner();
        let layer_name = "derived_instances".to_string();
        let manifest = Manifest {
            labels: Default::default(),
            schema: SCHEMA.into(),
            image_sha256: crate::localized::digest(&final_bytes),
            dimensions: [dimensions.0, dimensions.1],
            layers: vec![Layer {
                name: layer_name.clone(),
                image: "derived-id.png".into(),
                kind: Kind::Id,
                sha256: Some(crate::localized::digest(&encoded)),
                scale: 1.0,
                colour_space: ColourSpace::Linear,
                additive: false,
            }],
        };
        let bytes = serde_json::to_vec(&manifest)?;
        let hash = crate::localized::digest(&encoded);
        return Ok(Loaded {
            names,
            retained: std::collections::BTreeMap::from([
                (layer_name.clone(), encoded),
                ("dump".into(), dump_bytes.clone()),
            ]),
            manifest_bytes: bytes.clone(),
            manifest,
            images: std::collections::BTreeMap::from([(layer_name.clone(), img)]),
            witness: Witness {
                provenance: format!(
                    "derived from dump; lower confidence than real id buffer; dump sha256 {}",
                    crate::localized::digest(&dump_bytes)
                ),
                manifest_path: None,
                paths: Default::default(),
                manifest_sha256: crate::localized::digest(&bytes),
                layers: std::collections::BTreeMap::from([
                    (layer_name, hash),
                    ("dump".into(), crate::localized::digest(&dump_bytes)),
                ]),
            },
        });
    }
    let source = super::relative(root, &name)?;
    let bytes = super::read(&source, 1 << 20)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    if manifest.schema != SCHEMA
        || manifest.dimensions != [dimensions.0, dimensions.1]
        || manifest.layers.len() > 64
        || manifest.image_sha256 != crate::localized::digest(&final_bytes)
    {
        return Err(Error::Config(
            "capture-layer manifest schema/dimensions/image identity mismatch".into(),
        ));
    }
    let mut retained = std::collections::BTreeMap::new();
    let mut allocation = bytes.len() + final_bytes.len();
    let mut hashes = std::collections::BTreeMap::new();
    let mut images = std::collections::BTreeMap::new();
    for layer in &manifest.layers {
        if layer.name.is_empty()
            || hashes.contains_key(&layer.name)
            || !layer.scale.is_finite()
            || layer.scale <= 0.0
            || (layer.additive && layer.kind != Kind::Colour)
        {
            return Err(Error::Config("invalid/duplicate capture layer".into()));
        }
        let file = super::relative(root, &layer.image)?;
        let bytes = super::read(&file, 128 << 20)?;
        let hash = crate::localized::digest(&bytes);
        if layer
            .sha256
            .as_ref()
            .is_some_and(|expected| expected != &hash)
        {
            return Err(Error::Config("capture layer content hash differs".into()));
        }
        // The retained bytes are both hashed and decoded; file replacement cannot split identity.
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(crate::run::io_err("recognizing capture layer".into()))?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(256 << 20);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let img = reader
            .decode()
            .map_err(|source| Error::Decode { path: file, source })?;
        if (img.width(), img.height()) != dimensions {
            return Err(Error::Config("capture layer dimensions differ".into()));
        }
        allocation = allocation
            .saturating_add(bytes.len())
            .saturating_add(img.as_bytes().len());
        if allocation > 512 << 20 {
            return Err(Error::Config(
                "capture layers exceed total allocation limit".into(),
            ));
        }
        retained.insert(layer.name.clone(), bytes);
        hashes.insert(layer.name.clone(), hash);
        images.insert(layer.name.clone(), img);
    }
    Ok(Loaded {
        names: manifest.labels.clone(),
        retained,
        manifest_bytes: bytes.clone(),
        manifest,
        images,
        witness: Witness {
            provenance: "real id buffer / source-bound capture layers".into(),
            manifest_path: None,
            paths: Default::default(),
            manifest_sha256: crate::localized::digest(&bytes),
            layers: hashes,
        },
    })
}
fn definition<'a>(loaded: &'a Loaded, name: &str) -> Result<(&'a Layer, &'a image::DynamicImage)> {
    let layer = loaded
        .manifest
        .layers
        .iter()
        .find(|l| l.name == name)
        .ok_or_else(|| Error::Config(format!("layer {name:?} missing")))?;
    let image = loaded
        .images
        .get(name)
        .ok_or_else(|| Error::Config("layer samples missing".into()))?;
    Ok((layer, image))
}
/// Select native layer values after its declared scale.
pub fn selection(loaded: &Loaded, scope: &LayerScope) -> Result<Vec<bool>> {
    scope.predicate.validate()?;
    let (layer, img) = definition(loaded, &scope.layer)?;
    if matches!(layer.kind, Kind::Colour)
        || (matches!(
            scope.predicate,
            Predicate::Ids { .. } | Predicate::Labels { .. }
        ) && layer.kind != Kind::Id)
    {
        return Err(Error::Config(
            "predicate incompatible with layer kind".into(),
        ));
    }
    let samples: Vec<f64> = match img {
        image::DynamicImage::ImageLuma8(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageLuma16(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageRgb32F(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageRgba32F(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        _ => {
            return Err(Error::Config(
                "scalar layer requires L8/L16 or float R channel".into(),
            ));
        }
    };
    if samples.iter().any(|v| !v.is_finite()) {
        return Err(Error::Config("nonfinite scalar layer".into()));
    }
    let label_glob = if let Predicate::Labels { pattern } = &scope.predicate {
        Some(crate::config::compile_glob(pattern)?)
    } else {
        None
    };
    Ok(samples
        .into_iter()
        .map(|v| {
            if let Some(g) = &label_glob {
                loaded
                    .names
                    .get(&(v as u32))
                    .is_some_and(|name| g.is_match(name))
            } else {
                scope.predicate.matches(v * layer.scale)
            }
        })
        .collect())
}
/// Merge the declared side-specific footprints; empty results are errors.
pub fn scope_pair(b: &Loaded, c: &Loaded, policy: &Policy) -> Result<(Vec<bool>, u64, u64)> {
    let n = (u64::from(b.manifest.dimensions[0]) * u64::from(b.manifest.dimensions[1])) as usize;
    let Some(scope) = &policy.scope else {
        return Ok((vec![true; n], n as u64, n as u64));
    };
    let bs = selection(b, scope)?;
    let cs = selection(c, scope)?;
    let nb = bs.iter().filter(|v| **v).count() as u64;
    let nc = cs.iter().filter(|v| **v).count() as u64;
    let selected: Vec<_> = bs
        .into_iter()
        .zip(cs)
        .map(|(b, c)| match scope.mode {
            ScopeMode::Union => b || c,
            ScopeMode::Intersection => b && c,
            ScopeMode::Baseline => b,
            ScopeMode::Candidate => c,
        })
        .collect();
    if !selected.iter().any(|v| *v) {
        return Err(Error::Config("layer comparison scope is empty".into()));
    }
    Ok((selected, nb, nc))
}
fn colour(img: &image::DynamicImage, space: ColourSpace, scale: f64) -> Result<Vec<[f64; 3]>> {
    let samples: Vec<_> = img
        .to_rgb32f()
        .pixels()
        .map(|p| {
            p.0.map(|v| {
                let v = f64::from(v);
                (if space == ColourSpace::Srgb {
                    crate::color::linear(v)
                } else {
                    v
                }) * scale
            })
        })
        .collect();
    if samples.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::Config("nonfinite colour layer".into()));
    }
    Ok(samples)
}
fn summarize(
    name: String,
    delta: &[[f64; 3]],
    final_delta: &[[f64; 3]],
    selected: &[bool],
) -> ComponentDelta {
    let count = selected.iter().filter(|v| **v).count().max(1) as f64;
    let mut signed = [0.0; 3];
    let mut absolute = 0.0;
    let mut dot = 0.0;
    let mut norm = 0.0;
    for (i, d) in delta.iter().enumerate() {
        if !selected[i] {
            continue;
        }
        for channel in 0..3 {
            signed[channel] += d[channel];
            absolute += d[channel].abs();
            dot += d[channel] * final_delta[i][channel];
            norm += final_delta[i][channel].powi(2);
        }
    }
    ComponentDelta {
        name,
        mean_signed_rgb: signed.map(|v| v / count),
        mean_absolute: absolute / (count * 3.0),
        projected_share: (norm > 1e-20).then(|| dot / norm),
    }
}
/// Attribute final delta to the declared additive RGB layers and an exact residual.
pub fn decompose(
    b: &Loaded,
    c: &Loaded,
    base: &image::DynamicImage,
    candidate: &image::DynamicImage,
    selected: &[bool],
) -> Result<Option<Attribution>> {
    let names: std::collections::BTreeSet<_> = b
        .manifest
        .layers
        .iter()
        .chain(&c.manifest.layers)
        .filter(|l| l.additive)
        .map(|l| l.name.clone())
        .collect();
    if names.is_empty() {
        return Ok(None);
    }
    let space = |i: &image::DynamicImage| {
        if matches!(
            i.color(),
            image::ColorType::Rgb32F | image::ColorType::Rgba32F
        ) {
            ColourSpace::Linear
        } else {
            ColourSpace::Srgb
        }
    };
    let fb = colour(base, space(base), 1.0)?;
    let fc = colour(candidate, space(candidate), 1.0)?;
    let final_delta: Vec<_> = fb
        .iter()
        .zip(fc)
        .map(|(b, c)| std::array::from_fn(|i| c[i] - b[i]))
        .collect();
    let mut residual = final_delta.clone();
    let mut components = Vec::new();
    for name in names {
        let (bl, bi) = definition(b, &name)?;
        let (cl, ci) = definition(c, &name)?;
        if !bl.additive || !cl.additive || bl.kind != Kind::Colour || cl.kind != Kind::Colour {
            return Err(Error::Config(
                "additive component absent or incompatible on one side".into(),
            ));
        }
        let bv = colour(bi, bl.colour_space, bl.scale)?;
        let cv = colour(ci, cl.colour_space, cl.scale)?;
        let delta: Vec<[f64; 3]> = bv
            .iter()
            .zip(cv)
            .map(|(b, c)| std::array::from_fn(|i| c[i] - b[i]))
            .collect();
        for (r, d) in residual.iter_mut().zip(&delta) {
            for i in 0..3 {
                r[i] -= d[i];
            }
        }
        components.push(summarize(name, &delta, &final_delta, selected));
    }
    Ok(Some(Attribution {
        final_delta: summarize("final".into(), &final_delta, &final_delta, selected),
        components,
        residual: summarize("residual".into(), &residual, &final_delta, selected),
    }))
}
/// Construct portable measured layer evidence; native identity stays global.
#[allow(clippy::too_many_arguments)]
pub fn evidence(
    b: &Loaded,
    c: &Loaded,
    policy: &Policy,
    selected: &[bool],
    nb: u64,
    nc: u64,
    base: &image::DynamicImage,
    candidate: &image::DynamicImage,
) -> Result<LayerResult> {
    Ok(LayerResult {
        policy: policy.clone(),
        baseline: b.witness.clone(),
        candidate: c.witness.clone(),
        included_pixels: selected.iter().filter(|v| **v).count() as u64,
        baseline_pixels: nb,
        candidate_pixels: nc,
        attribution: if policy.attribution {
            decompose(b, c, base, candidate, selected)?
        } else {
            None
        },
    })
}

/// Copy exact retained manifest/layer bytes into a portable comparison bundle.
pub fn bundle(loaded: &mut Loaded, out: &Path, entry: &str, side: &str) -> Result<()> {
    let dir = format!("images/{entry}.d/layers");
    std::fs::create_dir_all(out.join(&dir))
        .map_err(crate::run::io_err("creating bundled layers".into()))?;
    let manifest_path = format!("{dir}/{side}-manifest.json");
    std::fs::write(out.join(&manifest_path), &loaded.manifest_bytes)
        .map_err(crate::run::io_err("writing bundled manifest".into()))?;
    loaded.witness.manifest_path = Some(manifest_path);
    if let Some(bytes) = loaded.retained.get("dump") {
        let path = format!("{dir}/{side}-dump.json");
        std::fs::write(out.join(&path), bytes)
            .map_err(crate::run::io_err("writing bundled dump".into()))?;
        loaded.witness.paths.insert("dump".into(), path);
    }
    for (index, layer) in loaded.manifest.layers.iter().enumerate() {
        let extension = Path::new(&layer.image)
            .extension()
            .and_then(|s| s.to_str())
            .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or("bin");
        let path = format!("{dir}/{side}-{index}.{extension}");
        let bytes = loaded
            .retained
            .get(&layer.name)
            .ok_or_else(|| Error::Config("missing retained layer".into()))?;
        std::fs::write(out.join(&path), bytes)
            .map_err(crate::run::io_err("writing bundled layer".into()))?;
        loaded.witness.paths.insert(layer.name.clone(), path);
    }
    Ok(())
}

fn source_path(root: &Path) -> std::path::PathBuf {
    root.join("derived-id.png")
}
/// Retrieve the first named ID layer in manifest order, retaining source confidence.
pub fn id_layer(loaded: &Loaded) -> Result<Option<super::field::IdLayer>> {
    let Some(layer) = loaded.manifest.layers.iter().find(|l| l.kind == Kind::Id) else {
        return Ok(None);
    };
    if layer.scale != 1.0 {
        return Err(Error::Config(
            "ID attribution requires unscaled integer IDs".into(),
        ));
    }
    let (_, img) = definition(loaded, &layer.name)?;
    Ok(Some(super::field::IdLayer {
        values: super::field::ids(img)?,
        names: loaded.names.clone(),
        provenance: format!(
            "{}; layer {}; sha256 {}",
            loaded.witness.provenance,
            layer.name,
            loaded
                .witness
                .layers
                .get(&layer.name)
                .map_or("unknown", String::as_str)
        ),
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path, candidate: bool) -> std::path::PathBuf {
        std::fs::create_dir_all(root).unwrap();
        let path = root.join("frame.png");
        let img = image::RgbImage::from_fn(64, 64, |x, y| {
            let v = if candidate && x >= 32 {
                200
            } else {
                100 + (y % 8) as u8
            };
            image::Rgb([v, v, v])
        });
        img.save(&path).unwrap();
        image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 64, |x, _| {
            image::Luma([if x < 32 { 513 } else { 7 }])
        })
        .save(root.join("ids.png"))
        .unwrap();
        let manifest = Manifest {
            labels: Default::default(),
            schema: SCHEMA.into(),
            image_sha256: crate::localized::digest(&super::super::read(&path, 128 << 20).unwrap()),
            dimensions: [64, 64],
            layers: vec![Layer {
                name: "surface".into(),
                image: "ids.png".into(),
                kind: Kind::Id,
                sha256: None,
                scale: 1.0,
                colour_space: ColourSpace::Linear,
                additive: false,
            }],
        };
        std::fs::write(
            root.join("frame.png.layers.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        path
    }
    #[test]
    fn named_native_id_scope_excludes_other_pixels_but_preserves_global_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let b = fixture(&tmp.path().join("b"), false);
        let c = fixture(&tmp.path().join("c"), true);
        let policy = Policy {
            scope: Some(LayerScope {
                layer: "surface".into(),
                predicate: Predicate::Ids { values: vec![513] },
                mode: ScopeMode::Union,
            }),
            attribution: false,
            ..Default::default()
        };
        let cfg = crate::config::RunConfig {
            layers: Some(policy),
            mask_mode: crate::compare::MaskMode::Neutralize,
            ..Default::default()
        };
        let r = crate::run::run(&b, &c, &tmp.path().join("out"), &cfg).unwrap();
        assert_eq!(r.entries[0].status, crate::Status::Pass);
        assert_eq!(r.entries[0].value, Some(0.0));
        assert_eq!(r.entries[0].bit_identical, Some(false));
        assert_eq!(r.entries[0].layers.as_ref().unwrap().included_pixels, 2048);
        let img = super::super::image(&b).unwrap();
        let loaded = load(&b, &cfg.layers.unwrap(), (64, 64)).unwrap();
        let selection = selection(
            &loaded,
            &LayerScope {
                layer: "surface".into(),
                predicate: Predicate::Ids { values: vec![513] },
                mode: ScopeMode::Baseline,
            },
        )
        .unwrap();
        assert_eq!(selection.iter().filter(|v| **v).count(), 2048);
        assert_eq!(img.width(), 64);
    }
    #[test]
    fn additive_components_have_exact_residual_in_linear_light() {
        let tmp = tempfile::tempdir().unwrap();
        let mut loaded = Vec::new();
        let mut finals = Vec::new();
        for (side, delta) in [("b", 0.0f32), ("c", 0.1f32)] {
            let root = tmp.path().join(side);
            std::fs::create_dir(&root).unwrap();
            let final_path = root.join("frame.exr");
            let final_img = image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
                8,
                8,
                image::Rgb([0.3 + delta; 3]),
            ));
            final_img.save(&final_path).unwrap();
            let mut layers = Vec::new();
            for (name, amount) in [("component-a", delta * 0.5), ("component-b", delta * 0.25)] {
                let file = format!("{name}.exr");
                image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
                    8,
                    8,
                    image::Rgb([0.1 + amount; 3]),
                ))
                .save(root.join(&file))
                .unwrap();
                layers.push(Layer {
                    name: name.into(),
                    image: file,
                    kind: Kind::Colour,
                    sha256: None,
                    scale: 1.0,
                    colour_space: ColourSpace::Linear,
                    additive: true,
                });
            }
            let manifest = Manifest {
                labels: Default::default(),
                schema: SCHEMA.into(),
                image_sha256: crate::localized::digest(
                    &super::super::read(&final_path, 128 << 20).unwrap(),
                ),
                dimensions: [8, 8],
                layers,
            };
            std::fs::write(
                root.join("frame.exr.layers.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            loaded.push(load(&final_path, &Policy::default(), (8, 8)).unwrap());
            finals.push(final_img);
        }
        let a = decompose(&loaded[0], &loaded[1], &finals[0], &finals[1], &[true; 64])
            .unwrap()
            .unwrap();
        assert!((a.residual.mean_signed_rgb[0] - 0.025).abs() < 1e-6);
        assert!(
            (a.components
                .iter()
                .map(|c| c.projected_share.unwrap())
                .sum::<f64>()
                + a.residual.projected_share.unwrap()
                - 1.0)
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn changed_final_identity_and_empty_scope_are_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let path = fixture(tmp.path(), false);
        let p = Policy::default();
        let a = load(&path, &p, (64, 64)).unwrap();
        let scope = Policy {
            scope: Some(LayerScope {
                layer: "surface".into(),
                predicate: Predicate::Ids { values: vec![123] },
                mode: ScopeMode::Union,
            }),
            ..Default::default()
        };
        assert!(scope_pair(&a, &a, &scope).is_err());
        let original = super::super::image(&path).unwrap();
        image::RgbImage::from_pixel(64, 64, image::Rgb([220; 3]))
            .save(&path)
            .unwrap();
        let mp = path.with_file_name("frame.png.layers.json");
        let mut manifest: Manifest =
            serde_json::from_slice(&super::super::read(&mp, 1 << 20).unwrap()).unwrap();
        manifest.image_sha256 =
            crate::localized::digest(&super::super::read(&path, 128 << 20).unwrap());
        std::fs::write(&mp, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(load(&path, &p, (64, 64)).is_ok());
        assert!(load_for_image(&path, &p, &original).is_err());
        std::fs::write(&path, b"changed").unwrap();
        assert!(load(&path, &p, (64, 64)).is_err());
    }
}

#[cfg(all(test, feature = "graphics"))]
mod buffer_tests {
    use super::*;
    #[test]
    fn native_buffer_scope_and_named_effect_share_native_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for (side, candidate) in [("b", false), ("c", true)] {
            let root = tmp.path().join(side);
            std::fs::create_dir(&root).unwrap();
            let final_path = root.join("depth.png");
            image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 64, |x, _| {
                image::Luma([if candidate && x >= 32 { 60000 } else { 10000 }])
            })
            .save(&final_path)
            .unwrap();
            image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 64, |x, _| {
                image::Luma([if x < 32 { 513 } else { 7 }])
            })
            .save(root.join("ids.png"))
            .unwrap();
            let m = Manifest {
                labels: Default::default(),
                schema: SCHEMA.into(),
                image_sha256: crate::localized::digest(
                    &super::super::read(&final_path, 128 << 20).unwrap(),
                ),
                dimensions: [64, 64],
                layers: vec![Layer {
                    name: "surface".into(),
                    image: "ids.png".into(),
                    kind: Kind::Id,
                    sha256: None,
                    scale: 1.0,
                    colour_space: ColourSpace::Linear,
                    additive: false,
                }],
            };
            std::fs::write(
                root.join("depth.png.layers.json"),
                serde_json::to_vec(&m).unwrap(),
            )
            .unwrap();
            paths.push(final_path);
        }
        let cfg = crate::config::RunConfig::from_toml_str(
            r#"
[[buffer]]
glob = "depth.png"
kind = "depth"
threshold = 0.0
[buffer.capture_layers]
attribution = false
[buffer.capture_layers.scope]
layer = "surface"
[buffer.capture_layers.scope.predicate]
kind = "ids"
values = [513]
"#,
        )
        .unwrap();
        let r = crate::run::run(&paths[0], &paths[1], &tmp.path().join("out"), &cfg).unwrap();
        assert_eq!(r.entries[0].value, Some(0.0));
        assert_eq!(r.entries[0].status, crate::Status::Pass);
        assert_eq!(r.entries[0].bit_identical, Some(false));
        let sel = super::super::effect::Selection::NamedLayer {
            name: "surface".into(),
            predicate: Predicate::Ids { values: vec![513] },
            manifest: None,
            dump: None,
        };
        assert_eq!(
            super::super::effect::select(&sel, &paths[0], tmp.path(), (64, 64))
                .unwrap()
                .iter()
                .filter(|v| **v)
                .count(),
            2048
        );
    }
}
