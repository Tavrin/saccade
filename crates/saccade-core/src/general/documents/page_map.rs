//! Explicit, complete one-to-one page correspondence; no positional multipage fallback.
use serde::{Deserialize, Serialize};
/// Declared map schema.
pub const SCHEMA: &str = "saccade-page-map.v1";
/// One correspondence, or a visible insertion/removal. Page numbers are one-based.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// Reference page; null for an insertion.
    #[serde(deserialize_with = "required_page")]
    pub reference: Option<usize>,
    /// Candidate page; null for a removal.
    #[serde(deserialize_with = "required_page")]
    pub candidate: Option<usize>,
}
fn required_page<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<usize>, D::Error> {
    Option::<usize>::deserialize(deserializer)
}
/// Input-bound declared correspondence. Every page must appear exactly once.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    /// Must be `saccade-page-map.v1`.
    pub schema: String,
    /// Reference encoded SHA-256.
    pub reference_sha256: String,
    /// Candidate encoded SHA-256.
    pub candidate_sha256: String,
    /// Complete one-to-one correspondence, including unmatched pages.
    pub pairs: Vec<Pair>,
}
impl Map {
    /// Rejects stale, duplicate, omitted and out-of-range correspondence.
    pub fn validate(
        &self,
        reference: usize,
        candidate: usize,
        a_hash: &str,
        b_hash: &str,
    ) -> crate::Result<()> {
        if self.schema != SCHEMA
            || self.reference_sha256 != a_hash
            || self.candidate_sha256 != b_hash
            || reference == 0
            || candidate == 0
            || reference > super::worker::CAPS.pages
            || candidate > super::worker::CAPS.pages
            || self.pairs.is_empty()
            || self.pairs.len() > reference + candidate
        {
            return Err(super::worker::error("document_page_map_invalid"));
        }
        let mut a = vec![false; reference];
        let mut b = vec![false; candidate];
        for pair in &self.pairs {
            if pair.reference.is_none() && pair.candidate.is_none() {
                return Err(super::worker::error("document_page_map_invalid"));
            }
            for (page, seen) in [(pair.reference, &mut a), (pair.candidate, &mut b)] {
                if let Some(page) = page {
                    let slot = page
                        .checked_sub(1)
                        .and_then(|p| seen.get_mut(p))
                        .ok_or_else(|| super::worker::error("document_page_map_invalid"))?;
                    if *slot {
                        return Err(super::worker::error("document_page_map_invalid"));
                    }
                    *slot = true;
                }
            }
        }
        if a.iter().chain(&b).any(|&s| !s) {
            return Err(super::worker::error("document_page_map_invalid"));
        }
        Ok(())
    }
}
