//! Stable parsing of the shared one-flag mask and required-effect grammar.
//! Parsing performs no file IO and preserves producer names and native units.
/// Parse `NAME=PREDICATE`; see the underlying grammar documentation.
pub use crate::evidence_quality::layers::parse_layer_spec;
use crate::{Error, Result, evidence_quality::layers::Predicate};
/// Selection parsed from a required-effect specification.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectSelection {
    /// Producer-named layer and validated native predicate.
    Layer {
        /// Case-sensitive producer layer name.
        name: String,
        /// Validated native predicate.
        predicate: Predicate,
    },
    /// Explicit mask image; resolution and dimensions are validated by the caller.
    Mask {
        /// Producer-supplied image path, unmodified.
        image: String,
    },
}
/// Parsed required-effect syntax; source manifest/dump resolution is caller-owned.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectSpec {
    /// Selection spelling without the optional coverage suffix.
    pub name: String,
    /// Inclusion selection, never an exclusion mask.
    pub selection: EffectSelection,
    /// Positive pixel minimum; one when the suffix is absent.
    pub min_pixels: u64,
}
/// Parse `NAME=PREDICATE[:MIN_PIXELS]` or `mask:PATH[:MIN_PIXELS]`.
///
/// A final colon followed by ASCII digits is the coverage suffix; other colons
/// belong to the selection. The minimum must be a positive `u64`. Empty mask
/// paths and invalid predicates are rejected. No whitespace is trimmed or files
/// opened. To resolve a manifest/dump, expand the parsed layer into the existing
/// [`crate::evidence_quality::effect::Selection::NamedLayer`] policy.
///
/// ```
/// let spec = saccade_core::mask_spec::parse_effect_spec("sample=id=12:8")?;
/// assert_eq!(spec.min_pixels, 8);
/// # Ok::<(), saccade_core::Error>(())
/// ```
pub fn parse_effect_spec(spec: &str) -> Result<EffectSpec> {
    let (body, min_pixels) = match spec.rsplit_once(':') {
        Some((body, count)) if !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()) => (
            body,
            count
                .parse::<u64>()
                .map_err(|_| Error::Config("invalid minimum pixels".into()))?,
        ),
        _ => (spec, 1),
    };
    if min_pixels == 0 {
        return Err(Error::Config("minimum pixels must be positive".into()));
    }
    let selection = if let Some(image) = body.strip_prefix("mask:") {
        if image.is_empty() {
            return Err(Error::Config("mask image path is empty".into()));
        }
        EffectSelection::Mask {
            image: image.into(),
        }
    } else {
        let (name, predicate) = parse_layer_spec(body)?;
        EffectSelection::Layer { name, predicate }
    };
    Ok(EffectSpec {
        name: body.into(),
        selection,
        min_pixels,
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn public_grammar_preserves_native_units_and_has_real_refusals() {
        assert_eq!(
            parse_layer_spec("specimen=id=12,65535").unwrap().1,
            Predicate::Ids {
                values: vec![12, 65535]
            }
        );
        assert_eq!(
            parse_layer_spec("paper=material=disclosure*").unwrap().1,
            parse_layer_spec("paper=label=disclosure*").unwrap().1
        );
        assert_eq!(
            parse_effect_spec("mask:sample.png:42").unwrap().min_pixels,
            42
        );
        assert_eq!(
            parse_effect_spec("sample=range=0.5,1.5")
                .unwrap()
                .min_pixels,
            1
        );
        for value in [
            "sample=id=",
            "sample=above=NaN",
            "sample=range=2,1",
            "=mask",
            "sample=label=[",
        ] {
            assert!(parse_layer_spec(value).is_err(), "{value}");
        }
        for value in ["mask:", "sample=mask:0", "sample=mask:18446744073709551616"] {
            assert!(parse_effect_spec(value).is_err(), "{value}");
        }
    }
}
