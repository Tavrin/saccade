//! Error type shared by the library.

use std::path::PathBuf;

/// Errors returned by `saccade-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    /// A configuration file or glob pattern is invalid.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// An output directory exists, is not empty and is not a previous saccade
    /// output; nothing in it is cleared or overwritten.
    #[error("{0}")]
    NotEmptyOutDir(String),
    /// Report (de)serialization failed.
    #[error("report JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;
