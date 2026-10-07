//! Deterministic glyph triage and region-level text sampling evidence.
use serde::{Deserialize, Serialize};

/// Missing-glyph report discriminator.
pub const TOFU_SCHEMA: &str = "saccade-tofu.v1";
/// Variant legibility report discriminator.
pub const LEGIBILITY_SCHEMA: &str = "saccade-text-legibility.v1";
/// Evidence states; absence of a candidate never certifies complete glyph coverage.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// At least one glyph-shaped candidate needs review.
    Candidates,
    /// Pixel thresholds are satisfied in a declared text region.
    Legible,
    /// At least one measured threshold fails.
    Illegible,
    /// Evidence cannot support a positive verdict.
    InsufficientEvidence,
    /// Optional evidence is absent.
    Unavailable,
}
/// A glyph-shaped observation, never a font/shaping diagnosis.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// Absolute capture pixels [x,y,width,height].
    pub rect_px: [u32; 4],
    /// hollow_rectangle or replacement_diamond.
    pub shape: String,
    /// Heuristic shape score in [0,1], not a calibrated probability.
    pub confidence: f64,
}
/// Optional OCR evidence, isolated from pixel measurements.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OcrEvidence {
    /// unavailable, candidates, legible (agreement) or illegible (disagreement).
    pub state: State,
    /// Exact observed Unicode scalars; inert data.
    pub text: Option<String>,
    /// Exact baseline agreement, absent without comparable observations.
    pub agrees_with_baseline: Option<bool>,
    /// Adapter identity or explicit missing-evidence reason.
    pub reason: String,
}
/// Missing-glyph triage result.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TofuReport {
    /// Content-addressed transport report identity, absent before emission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// Caller-supplied capture backlinks, absent when not declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Versioned discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-tofu.v1")))]
    pub schema: String,
    /// candidates or insufficient_evidence; never clean.
    pub state: State,
    /// Exact encoded image identity.
    pub image_sha256: String,
    /// Capture dimensions.
    pub dimensions: [u32; 2],
    /// Mask identity when supplied; nonzero includes pixels.
    pub mask_sha256: Option<String>,
    /// Expected text, retained as declared context only.
    pub expected_text: Option<String>,
    /// Pixel candidates with capture coordinates.
    pub regions: Vec<Candidate>,
    /// Independent optional OCR evidence.
    pub ocr: OcrEvidence,
    /// Reasons for the state and explicit detection limits.
    pub reasons: Vec<String>,
}
/// Frozen thresholds in physical capture pixels.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// WCAG-style relative luminance ratio (not compliance certification).
    pub minimum_contrast: f64,
    /// Minimum component body-height proxy in sampled pixels.
    pub minimum_x_height_px: f64,
    /// Minimum normalized adjacent-pixel edge step.
    pub minimum_sharpness: f64,
    /// Minimum stroke run width in pixels.
    pub minimum_stroke_px: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            minimum_contrast: 4.5,
            minimum_x_height_px: 8.,
            minimum_sharpness: 0.35,
            minimum_stroke_px: 1.,
        }
    }
}
/// Measurements for one declared text region.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionResult {
    /// Stable zero-based baseline region index.
    pub region: usize,
    /// Region in the current capture, after proportional mapping.
    pub rect_px: [u32; 4],
    /// Pixel/optional OCR verdict with unavailable evidence kept separate.
    pub state: State,
    /// Actual measured background/foreground luminance ratio.
    pub contrast: Option<f64>,
    /// Minimum measurable body-height proxy; not inferred typographic x-height.
    pub x_height_px: Option<f64>,
    /// Minimum per-component normalized maximum edge step.
    pub sharpness: Option<f64>,
    /// Minimum component lower-quartile horizontal/vertical run width.
    pub stroke_px: Option<f64>,
    /// Optional OCR agreement with baseline in this region.
    pub ocr: OcrEvidence,
    /// Threshold failures or abstention reasons.
    pub reasons: Vec<String>,
}
/// One variant and all baseline-mapped regions.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// Input index (zero is first variant).
    pub index: usize,
    /// Exact encoded input identity.
    pub image_sha256: String,
    /// Capture dimensions.
    pub dimensions: [u32; 2],
    /// Every declared region, including unavailable evidence.
    pub regions: Vec<RegionResult>,
}
/// Baseline plus N variant results.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegibilityReport {
    /// Content-addressed transport report identity, absent before emission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// Caller-supplied capture backlinks, absent when not declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Versioned discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-text-legibility.v1")))]
    pub schema: String,
    /// Overall state; failures dominate missing evidence.
    pub state: State,
    /// Encoded baseline identity.
    pub baseline_sha256: String,
    /// Baseline capture dimensions.
    pub baseline_dimensions: [u32; 2],
    /// Effective thresholds.
    pub policy: Policy,
    /// Baseline measurements; an invalid baseline cannot qualify a variant.
    pub baseline: Vec<RegionResult>,
    /// Per-region results for every supplied variant.
    pub variants: Vec<Variant>,
    /// Explicit interpretation boundaries.
    pub limitations: Vec<String>,
}
/// Missing optional OCR with an explicit reason.
pub fn unavailable_ocr(reason: impl Into<String>) -> OcrEvidence {
    OcrEvidence {
        state: State::Unavailable,
        text: None,
        agrees_with_baseline: None,
        reason: reason.into(),
    }
}
#[cfg(feature = "text-quality")]
mod pixels;
#[cfg(feature = "text-quality")]
pub use pixels::{legibility, tofu, validate_rect};
