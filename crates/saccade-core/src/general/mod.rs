//! Bounded general-image comparison primitives added by Wave 6.
pub mod input;
pub mod registration;

/// Bounded result envelope for the general comparator families.
pub const RESULT_SCHEMA: &str = "saccade-general-result.v1";
/// No-reference blur, noise, compression-pattern and clipping measures.
pub mod assessment;
/// Bounded optional SVG/PDF rendering and document contracts.
pub mod documents;
/// Optional ONNX embeddings and cosine arithmetic.
pub mod embedding;
/// Perceptual hashes and Hamming search.
pub mod hashing;
/// Bounded image metadata and current compression evidence.
pub mod integrity;
/// OCR text observations, CER/WER and positional differences.
pub mod text;

// wave6b
/// Offline embedded Content Credentials validation.
pub mod credentials;

/// Pinned pure Rust OCR and observation adapter.
pub mod ocr;

/// Bounded unsigned XMP and IPTC extraction.
pub mod metadata;

/// Unqualified compression and resampling observations.
pub mod forensics;

/// Frozen export parity and held-out calibration.
pub mod embedding_qualification;
