//! Bounded general-image comparison primitives added by Wave 6.
pub mod input;
pub mod registration;

/// Bounded result envelope for the general comparator families.
pub const RESULT_SCHEMA: &str = "saccade-general-result.v1";
/// Perceptual hashes and Hamming search.
pub mod hashing;
