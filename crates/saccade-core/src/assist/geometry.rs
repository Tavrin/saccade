//! Original-pixel geometry and explicit crop/resize inversion.
use super::{Result, require};
use serde::{Deserialize, Serialize};

/// Canonical geometry in original image pixels; boxes have exclusive far edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    content = "pixels",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Geometry {
    /// [x,y,width,height], finite, positive area and wholly inside the image.
    Box([f64; 4]),
    /// [x,y], inside the image (far edges excluded).
    Point([f64; 2]),
}
impl Geometry {
    /// Rejects nonfinite, empty and out-of-bounds geometry.
    pub fn validate(&self, dimensions: [u32; 2]) -> Result<()> {
        require(!dimensions.contains(&0), "empty dimensions")?;
        let [w, h] = dimensions.map(f64::from);
        let ok = match *self {
            Self::Box([x, y, bw, bh]) => {
                [x, y, bw, bh].iter().all(|v| v.is_finite())
                    && x >= 0.
                    && y >= 0.
                    && bw > 0.
                    && bh > 0.
                    && x + bw <= w
                    && y + bh <= h
            }
            Self::Point([x, y]) => {
                x.is_finite() && y.is_finite() && x >= 0. && y >= 0. && x < w && y < h
            }
        };
        require(ok, "geometry outside original image")
    }
    /// Returns a bounding box; points remain zero-area locations.
    pub fn bounds(&self) -> [f64; 4] {
        match *self {
            Self::Box(b) => b,
            Self::Point([x, y]) => [x, y, 0., 0.],
        }
    }
}
/// A view's exact original crop and encoded size; no implicit resize policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Transform {
    /// Original-pixel x,y,width,height.
    pub crop: [u32; 4],
    /// Encoded view width,height.
    pub encoded: [u32; 2],
}
impl Transform {
    /// Ensures the recorded transform represents a nonempty view.
    pub fn validate(&self, dimensions: [u32; 2]) -> Result<()> {
        Geometry::Box(self.crop.map(f64::from)).validate(dimensions)?;
        require(!self.encoded.contains(&0), "empty encoded view")
    }
    /// Converts [0,1] provider coordinates to original pixels after validating the view.
    pub fn from_normalized(&self, geometry: &Geometry, dimensions: [u32; 2]) -> Result<Geometry> {
        self.validate(dimensions)?;
        geometry.validate([1, 1])?;
        let [x, y, w, h] = self.crop.map(f64::from);
        let result = match *geometry {
            Geometry::Box([gx, gy, gw, gh]) => {
                Geometry::Box([x + gx * w, y + gy * h, gw * w, gh * h])
            }
            Geometry::Point([gx, gy]) => Geometry::Point([x + gx * w, y + gy * h]),
        };
        result.validate(dimensions)?;
        Ok(result)
    }
}
