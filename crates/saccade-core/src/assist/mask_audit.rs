//! Individual-mask accounting, with overlap retained and union counted once.
use super::{
    Error, Result,
    catalog::{Catalog, Mask},
    require,
};
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};

/// Hash-bound declarations for reports that did not preserve individual masks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// saccade-assist-masks.v1.
    pub schema: String,
    /// Exact report bytes this declaration supplements.
    pub report_hash: Digest,
    /// Individually identified memberships, origins and stated rationales.
    pub masks: Vec<Mask>,
}
/// Per-mask area and raw changed pixels, independent of configured verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Individual mask identity.
    pub id: String,
    /// Original membership area.
    pub pixels: u64,
    /// Native pixel changes inside this mask; missing originals stay unavailable.
    pub changed_pixels: Option<u64>,
    /// Pixels shared with other masks; memberships remain attributable to each.
    pub overlapping_pixels: u64,
    /// Every intersecting catalog region; relevance needs a visible/expected-content contract.
    pub intersecting_regions: Vec<String>,
    /// Missing original pixels are unavailable, never safe.
    pub availability: String,
}
/// Per-mask and union accounting; overlap is not double-counted in union totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audit {
    /// Individual membership and change measures.
    pub masks: Vec<Finding>,
    /// Union area with every excluded pixel counted once.
    pub union_pixels: u64,
    /// Raw pixel changes in union; not a perceptual/regression verdict.
    pub union_changed_pixels: Option<u64>,
    /// Union row-major mask content hash.
    pub union_hash: Digest,
}
/// Audit preserved/declared membership against actual original RGBA pixels.
/// Pixels pre-masked by the producer cannot establish relevance or absence.
pub fn audit(catalog: &Catalog, original: Option<(&[u8], &[u8])>) -> Result<Audit> {
    catalog.validate()?;
    let image = catalog
        .images
        .first()
        .ok_or(Error::Invalid("mask image missing"))?;
    let pixels = image.dimensions[0] as usize * image.dimensions[1] as usize;
    let original = original.filter(|_| {
        catalog.images.iter().all(|i| i.original_pixels)
            && catalog.exclusions.iter().all(|m| m.original_pixels)
    });
    if let Some((a, b)) = original {
        require(
            a.len() == pixels * 4 && b.len() == pixels * 4,
            "raw mask audit dimensions",
        )?;
    }
    let mut membership = vec![0u16; pixels];
    let bits: Vec<_> = catalog
        .exclusions
        .iter()
        .map(Mask::bits)
        .collect::<Result<_>>()?;
    for mask in &bits {
        for (count, &bit) in membership.iter_mut().zip(mask) {
            *count += u16::from(bit);
        }
    }
    let changed = |index: usize| {
        original.map(|(a, b)| a[index * 4..index * 4 + 4] != b[index * 4..index * 4 + 4])
    };
    let mut masks = Vec::new();
    for (mask, bits) in catalog.exclusions.iter().zip(&bits) {
        let mut area = 0;
        let mut changes = 0;
        let mut overlap = 0;
        for (index, &bit) in bits.iter().enumerate().filter(|(_, b)| **b != 0) {
            area += u64::from(bit);
            changes += u64::from(changed(index).unwrap_or(false));
            overlap += u64::from(membership[index] > 1);
        }
        let regions = catalog
            .regions
            .iter()
            .filter(|r| {
                let [x, y, w, h] = r.rect;
                (y..y + h).any(|row| {
                    (x..x + w).any(|col| {
                        bits[row as usize * image.dimensions[0] as usize + col as usize] != 0
                    })
                })
            })
            .map(|r| r.id.clone())
            .collect();
        masks.push(Finding {
            id: mask.id.clone(),
            pixels: area,
            changed_pixels: original.map(|_| changes),
            overlapping_pixels: overlap,
            intersecting_regions: regions,
            availability: if original.is_some() {
                "available"
            } else {
                "unavailable"
            }
            .into(),
        });
    }
    let union: Vec<u8> = membership.iter().map(|n| u8::from(*n > 0)).collect();
    Ok(Audit {
        masks,
        union_pixels: union.iter().map(|n| u64::from(*n)).sum(),
        union_changed_pixels: original.map(|_| {
            union
                .iter()
                .enumerate()
                .filter(|(_, b)| **b != 0)
                .map(|(i, _)| u64::from(changed(i).unwrap_or(false)))
                .sum()
        }),
        union_hash: Digest::of_bytes(&union),
    })
}
