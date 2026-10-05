//! Native layer predicates, shared by effect occupancy and comparison scope.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Selection rule evaluated in native units (ID PNGs retain integer values).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
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
            Self::Ids { values } => values.iter().any(|v| f64::from(*v) == value),
            Self::Range { min, max } => value >= *min && value <= *max,
            Self::Above { threshold } => value > *threshold,
            Self::Mask => value != 0.0,
        }
    }
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
