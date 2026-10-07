//! Error type shared by the library.

use std::path::PathBuf;

/// Errors returned by `saccade-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A process-installed authentication guard refused this baseline.
    #[error("{code}: {message}")]
    ApprovalRefused {
        /// Stable refusal code supplied by the guard.
        code: &'static str,
        /// Public refusal explanation, without verifier diagnostics.
        message: String,
    },
    /// Baseline bytes at measurement differ from the authenticated inventory.
    #[error("approval_content_mismatch: baseline changed before measurement")]
    ApprovalContentMismatch,

    /// Stable document worker or cap failure.
    #[error("{code}: document operation refused")]
    Document {
        /// Machine-readable refusal code.
        code: &'static str,
    },
    /// Blind board input, binding or protocol violation.
    #[error("{code}: {message}")]
    ReviewBoard {
        /// Stable transport error code.
        code: &'static str,
        /// Human-readable finding.
        message: String,
    },
    /// Arm identity is incomplete or violates declared experiment variables.
    #[error("invalid_comparison: arm identity validation refused a verdict")]
    InvalidComparison(Box<crate::arms::Check>),
    /// A preregistered trial or displayed evidence changed after registration.
    #[error("trial_plan_changed: preregistered plan or evidence differs")]
    TrialPlanChanged,
    /// The FLIP backend rejected the input or viewing parameters.
    #[error("FLIP comparison failed: {0}")]
    Flip(#[from] flip_rs::FlipError),
    /// An image could not be opened or decoded.
    #[error("failed to decode image {path}: {source}")]
    Decode {
        /// The image path.
        path: PathBuf,
        /// The decoder error.
        #[source]
        source: image::ImageError,
    },
    /// The two images being compared have different dimensions.
    #[error("dimensions differ: capture is {test_w}x{test_h}, baseline is {ref_w}x{ref_h}")]
    DimensionMismatch {
        /// Capture width.
        test_w: u32,
        /// Capture height.
        test_h: u32,
        /// Baseline width.
        ref_w: u32,
        /// Baseline height.
        ref_h: u32,
    },
    /// One side of a pair is HDR and the other is LDR.
    #[error("cannot compare an HDR image with an LDR image: {0}")]
    HdrMismatch(String),
    /// An image has zero width or height.
    #[error("image is empty (0 pixels)")]
    EmptyImage,
    /// A filesystem operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being attempted, including the path.
        context: String,
        /// The IO error.
        #[source]
        source: std::io::Error,
    },
    /// Writing an image (heatmap, copy) failed.
    #[error("failed to write image {path}: {source}")]
    Encode {
        /// The output path.
        path: PathBuf,
        /// The encoder error.
        #[source]
        source: image::ImageError,
    },
    /// Input from a newer contract version requires a reader upgrade.
    #[error(
        "version_skew: written by {actual}; installed saccade supports up to {supported}, upgrade"
    )]
    VersionSkew {
        /// Producer's contract identifier or unsupported extension.
        actual: String,
        /// Latest supported contract identifier.
        supported: &'static str,
    },
    /// A configuration file or glob pattern is invalid.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// An image/performance noise input was supplied to the other consumer.
    #[error("wrong_noise_kind: expected {expected} noise; generate it with `saccade {command}`")]
    WrongNoiseKind {
        /// Required noise kind.
        expected: &'static str,
        /// Command producing the correct input.
        command: &'static str,
    },
    /// An output directory exists, is not empty and is not a previous saccade
    /// output; nothing in it is cleared or overwritten.
    #[error("{0}")]
    NotEmptyOutDir(String),
    /// A run performance sidecar is malformed.
    #[error(transparent)]
    Perf(#[from] crate::perf::PerfError),
    /// The requested computation was not compiled into this build.
    #[error("feature_unavailable: requires feature {feature}")]
    FeatureUnavailable {
        /// Cargo feature required for the operation.
        feature: &'static str,
    },
    /// Report (de)serialization failed.
    #[error("report JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;
