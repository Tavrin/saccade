//! Dense image correspondence and supplied renderer-vector diagnostics.
//! Persisted types are available without the optional producer.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Buffer coordinate convention. Rows always run top to bottom.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Units {
    /// Displacement in capture pixels.
    Pixels,
    /// Displacement in normalized texture coordinates.
    Uv,
    /// Displacement in normalized device coordinates (-1 through +1).
    Ndc,
}
/// Vertical vector axis, independent of the buffer's row order.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Origin {
    /// Positive Y points down.
    TopLeft,
    /// Positive Y points up.
    BottomLeft,
}
/// Correspondence direction also determines the buffer grid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Direction {
    /// Reference-grid pixels point to candidate pixels.
    ReferenceToCandidate,
    /// Candidate-grid pixels point to reference pixels.
    CandidateToReference,
}
/// Supplied row-major float buffer, with explicit producer validity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Buffer {
    /// Contract identity.
    pub schema: String,
    /// One vector per pixel, no implicit resampling.
    pub vectors: Vec<[f32; 2]>,
    /// One validity bit per vector.
    pub valid: Vec<bool>,
}
/// Renderer convention pinned to images and exact encoded buffer bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Sidecar {
    /// Contract identity.
    pub schema: String,
    /// Width and height shared by both frames and the buffer.
    pub dimensions: [u32; 2],
    /// Lowercase SHA256 of encoded reference image.
    pub reference_sha256: String,
    /// Lowercase SHA256 of encoded candidate image.
    pub candidate_sha256: String,
    /// Lowercase SHA256 of encoded buffer JSON.
    pub vectors_sha256: String,
    /// Units of the stored components.
    pub units: Units,
    /// Vertical component convention.
    pub origin: Origin,
    /// Direction and grid of the stored vectors.
    pub direction: Direction,
    /// Reference image jitter, in top-left pixel units.
    pub reference_jitter_px: [f32; 2],
    /// Candidate image jitter, in top-left pixel units.
    pub candidate_jitter_px: [f32; 2],
    /// Whether the buffer contains the displacement due to jitter.
    pub includes_jitter: bool,
    /// Positive frame interval, retained as provenance; vectors are displacements.
    pub frame_interval_ms: f64,
    /// Renderer/source/config identity supplied by the producer, never inferred.
    pub producer: serde_json::Value,
}
/// Reasons a pixel is excluded from vector validation, not causal labels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum State {
    /// Locally textured, consistent, and photometrically supported.
    Qualified,
    /// Image or displaced footprint meets a boundary.
    Boundary,
    /// Flat, aperture-limited or locally repeated texture.
    AmbiguousTexture,
    /// Opposite-direction flow fails consistency; occlusion is one possible cause.
    PossibleOcclusionOrMismatch,
    /// Appearance does not satisfy the image-matching model.
    AppearanceUncertain,
    /// Nearby displacement disagreement suggests a motion boundary.
    MotionBoundary,
}
/// Measured row-major flow on one frame grid, in top-left screenshot pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Field {
    /// Direction/grid.
    pub direction: Direction,
    /// One vector per pixel, including estimates at uncertain pixels.
    pub vectors: Vec<[f32; 2]>,
    /// Qualification for every pixel.
    pub states: Vec<State>,
    /// Number of qualified pixels; not a probability of correctness.
    pub qualified_pixels: usize,
}
/// Endpoint and angular disagreement only on mutually valid pixels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Validation {
    /// Full convention and producer assertion retained for review.
    pub sidecar: Sidecar,
    /// Producer-valid count before measured-flow exclusions.
    pub declared_valid_pixels: usize,
    /// Number of pixels supporting endpoint statistics.
    pub compared_pixels: usize,
    /// Fraction of the entire declared grid actually compared.
    pub coverage: f64,
    /// Euclidean endpoint error in unjittered original-image pixels.
    pub mean_endpoint_px: Option<f64>,
    /// Nearest-rank 95th percentile endpoint error.
    pub p95_endpoint_px: Option<f64>,
    /// Maximum endpoint error.
    pub maximum_endpoint_px: Option<f64>,
    /// Mean angle in degrees, excluding either vector of length <= 0.1 px.
    pub mean_angle_degrees: Option<f64>,
    /// Angular comparison support, separate from endpoint support.
    pub angular_pixels: usize,
    /// Row-major pixels supporting endpoint statistics.
    pub compared: Vec<bool>,
}
/// Dense motion evidence with an unmodified raw FLIP measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Report {
    /// Contract identity.
    pub schema: String,
    /// Exact image identities, reference then candidate.
    pub image_sha256: [String; 2],
    /// Original dimensions.
    pub dimensions: [u32; 2],
    /// Native algorithm variant and all fixed settings.
    pub method: serde_json::Value,
    /// Unaligned full-frame perceptual measurements remain authoritative.
    pub raw_flip: crate::report::Metrics,
    /// Viewing parameter used for raw FLIP.
    pub pixels_per_degree: f32,
    /// Raw mean threshold; vectors cannot change it.
    pub maximum_raw_mean: f32,
    /// Raw threshold result, independent of motion diagnostics.
    pub raw_regression: bool,
    /// Reference and candidate grid fields, respectively.
    pub fields: [Field; 2],
    /// Optional renderer comparison.
    pub renderer: Option<Validation>,
    /// Qualification boundaries.
    pub limitations: Vec<String>,
}
fn invalid(message: &str) -> Error {
    Error::Config(message.into())
}
impl Sidecar {
    /// Validate identities and normalize a buffer without using measured flow.
    pub fn normalize(&self, buffer: &Buffer, pins: &[String; 3]) -> Result<Vec<[f32; 2]>> {
        let count = u64::from(self.dimensions[0]) * u64::from(self.dimensions[1]);
        if self.schema != "saccade-motion-vectors.v1"
            || buffer.schema != "saccade-vector-buffer.v1"
            || count == 0
            || count > 1024 * 1024
            || buffer.vectors.len() as u64 != count
            || buffer.valid.len() != buffer.vectors.len()
            || pins.iter().any(|p| {
                p.len() != 64
                    || !p
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            || self.reference_sha256 != pins[0]
            || self.candidate_sha256 != pins[1]
            || self.vectors_sha256 != pins[2]
            || !self.frame_interval_ms.is_finite()
            || self.frame_interval_ms <= 0.0
            || !self.producer.is_object()
            || self.producer.as_object().is_none_or(|p| p.is_empty())
            || self
                .reference_jitter_px
                .iter()
                .chain(&self.candidate_jitter_px)
                .any(|v| !v.is_finite() || v.abs() > 16.0)
        {
            return Err(invalid(
                "invalid motion sidecar, buffer length, producer or byte identity",
            ));
        }
        buffer
            .vectors
            .iter()
            .map(|&v| {
                if v.iter().any(|v| !v.is_finite()) {
                    return Err(invalid("nonfinite motion vector"));
                }
                let scale = match self.units {
                    Units::Pixels => [1.0, 1.0],
                    Units::Uv => self.dimensions.map(|v| v as f32),
                    Units::Ndc => self.dimensions.map(|v| v as f32 * 0.5),
                };
                let mut v = [v[0] * scale[0], v[1] * scale[1]];
                if self.origin == Origin::BottomLeft {
                    v[1] = -v[1];
                }
                if self.includes_jitter {
                    let sign = if self.direction == Direction::ReferenceToCandidate {
                        1.0
                    } else {
                        -1.0
                    };
                    for (i, component) in v.iter_mut().enumerate() {
                        *component -=
                            sign * (self.candidate_jitter_px[i] - self.reference_jitter_px[i]);
                    }
                }
                if v.iter().any(|v| !v.is_finite() || v.abs() > 16384.0) {
                    return Err(invalid("motion vector outside bounded displacement range"));
                }
                Ok(v)
            })
            .collect()
    }
}
/// Compute diagnostics. Report readers do not require the producer feature.
pub fn review(
    reference: &image::RgbaImage,
    candidate: &image::RgbaImage,
    image_sha256: [String; 2],
    supplied: Option<(&Sidecar, &Buffer, &str)>,
    pixels_per_degree: f32,
    maximum_raw_mean: f32,
) -> Result<Report> {
    #[cfg(feature = "dense-motion")]
    {
        producer::review(
            reference,
            candidate,
            image_sha256,
            supplied,
            pixels_per_degree,
            maximum_raw_mean,
        )
    }
    #[cfg(not(feature = "dense-motion"))]
    {
        let _ = (
            reference,
            candidate,
            image_sha256,
            supplied,
            pixels_per_degree,
            maximum_raw_mean,
        );
        Err(Error::FeatureUnavailable {
            feature: "dense-motion",
        })
    }
}
#[cfg(feature = "dense-motion")]
mod producer;
