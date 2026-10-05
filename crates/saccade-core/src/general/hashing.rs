//! Deterministic 64-bit perceptual hashes and bounded BK-tree near-duplicate clustering.
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// Hash evidence schema.
pub const HASH_SCHEMA: &str = "saccade-hash.v1";
/// Dedupe evidence schema.
pub const DEDUPE_SCHEMA: &str = "saccade-dedupe.v1";
/// Algorithm used by a Hamming index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    /// Mean luminance threshold over 8x8 pixels.
    Ahash,
    /// Horizontal gradient sign over 9x8 pixels.
    Dhash,
    /// Median low-frequency DCT coefficients (DC bit is zero).
    Phash,
}
/// All hashes from a single decoded raster. Not cryptographic identity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Hashes {
    /// Average hash.
    pub ahash: u64,
    /// Difference hash.
    pub dhash: u64,
    /// DCT perceptual hash; 63 informative bits plus a zero DC bit.
    pub phash: u64,
}
impl Hashes {
    /// Selects the algorithm; index and query must agree.
    pub fn get(self, algorithm: Algorithm) -> u64 {
        match algorithm {
            Algorithm::Ahash => self.ahash,
            Algorithm::Dhash => self.dhash,
            Algorithm::Phash => self.phash,
        }
    }
}
/// Computes all hashes in one decode; transparency is composited over white.
/// Resize uses triangle filtering, luminance follows the image crate's sRGB luma conversion.
pub fn hash(image: &RgbaImage) -> Hashes {
    let rgb = crate::compare::flatten_over(image, 255);
    let gray = image::DynamicImage::ImageRgb8(rgb).to_luma8();
    let a = image::imageops::resize(&gray, 8, 8, image::imageops::FilterType::Triangle);
    let mean = a.as_raw().iter().map(|&v| u32::from(v)).sum::<u32>() as f64 / 64.;
    let ahash = a
        .as_raw()
        .iter()
        .enumerate()
        .fold(0, |h, (i, &v)| h | (u64::from(f64::from(v) > mean) << i));
    let d = image::imageops::resize(&gray, 9, 8, image::imageops::FilterType::Triangle);
    let mut dhash = 0;
    for y in 0..8 {
        for x in 0..8 {
            if d.get_pixel(x, y)[0] > d.get_pixel(x + 1, y)[0] {
                dhash |= 1 << (y * 8 + x);
            }
        }
    }
    let p = image::imageops::resize(&gray, 32, 32, image::imageops::FilterType::Triangle);
    let cos: Vec<[f64; 32]> = (0..8)
        .map(|k| {
            std::array::from_fn(|x| {
                ((2 * x + 1) as f64 * k as f64 * std::f64::consts::PI / 64.).cos()
            })
        })
        .collect();
    let mut rows = [[0.; 8]; 32];
    for (y, row) in rows.iter_mut().enumerate() {
        for (u, value) in row.iter_mut().enumerate() {
            *value = (0..32)
                .map(|x| f64::from(p.get_pixel(x as u32, y as u32)[0]) * cos[u][x])
                .sum();
        }
    }
    let mut coefficients = [0.; 64];
    for v in 0..8 {
        for u in 0..8 {
            coefficients[v * 8 + u] = (0..32).map(|y| rows[y][u] * cos[v][y]).sum();
        }
    }
    let mut sorted = coefficients[1..].to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[31];
    let phash = coefficients
        .iter()
        .enumerate()
        .skip(1)
        .fold(0, |h, (i, &v)| h | (u64::from(v > median) << i));
    Hashes {
        ahash,
        dhash,
        phash,
    }
}
struct Node {
    hash: u64,
    id: usize,
    children: BTreeMap<u32, usize>,
}
/// A Hamming BK-tree stores only unique hashes and one representative per hash.
/// A capacity bounds memory; lookup never loads pixels.
pub struct BkTree {
    nodes: Vec<Node>,
    capacity: usize,
}
impl BkTree {
    /// Creates a bounded tree. Maximum accepted capacity is 100000.
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::new(),
            capacity: capacity.min(100000),
        }
    }
    /// Inserts one unique hash. Returns the previous representative for an exact duplicate.
    pub fn insert(&mut self, hash: u64, id: usize) -> crate::Result<Option<usize>> {
        if self.nodes.is_empty() {
            if self.capacity == 0 {
                return Err(crate::Error::Config("hash index capacity exceeded".into()));
            }
            self.nodes.push(Node {
                hash,
                id,
                children: BTreeMap::new(),
            });
            return Ok(None);
        }
        let mut at = 0;
        loop {
            let d = (hash ^ self.nodes[at].hash).count_ones();
            if d == 0 {
                return Ok(Some(self.nodes[at].id));
            }
            if let Some(&child) = self.nodes[at].children.get(&d) {
                at = child;
            } else {
                if self.nodes.len() >= self.capacity {
                    return Err(crate::Error::Config("hash index capacity exceeded".into()));
                }
                let next = self.nodes.len();
                self.nodes.push(Node {
                    hash,
                    id,
                    children: BTreeMap::new(),
                });
                self.nodes[at].children.insert(d, next);
                return Ok(None);
            }
        }
    }
    /// Visits one representative per distinct hash within the inclusive radius (0..64).
    /// Streaming callback bounds memory even when every hash matches.
    pub fn query(&self, hash: u64, radius: u32, mut found: impl FnMut(usize, u32)) {
        if self.nodes.is_empty() {
            return;
        }
        let radius = radius.min(64);
        let mut pending = vec![0];
        while let Some(at) = pending.pop() {
            let node = &self.nodes[at];
            let d = (hash ^ node.hash).count_ones();
            if d <= radius {
                found(node.id, d);
            }
            for (_, child) in node
                .children
                .range(d.saturating_sub(radius)..=d.saturating_add(radius))
            {
                pending.push(*child);
            }
        }
    }
}
/// Connected components under a Hamming threshold, deterministic in supplied order.
/// A cluster is transitive: two members may be farther apart than the threshold.
/// Exact duplicates collapse in the index, avoiding quadratic work for repeated copies.
pub fn clusters(hashes: &[u64], threshold: u32) -> crate::Result<Vec<Vec<usize>>> {
    if hashes.len() > 100000 || threshold > 64 {
        return Err(crate::Error::Config(
            "dedupe supports <=100000 images and Hamming radius <=64".into(),
        ));
    }
    if threshold == 64 {
        return Ok(if hashes.is_empty() {
            vec![]
        } else {
            vec![(0..hashes.len()).collect()]
        });
    }
    let mut parent: Vec<_> = (0..hashes.len()).collect();
    fn root(parent: &mut [usize], mut id: usize) -> usize {
        while parent[id] != id {
            parent[id] = parent[parent[id]];
            id = parent[id];
        }
        id
    }
    let mut tree = BkTree::new(hashes.len());
    for (id, &hash) in hashes.iter().enumerate() {
        tree.query(hash, threshold, |other, _| {
            let a = root(&mut parent, id);
            let b = root(&mut parent, other);
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        });
        tree.insert(hash, id)?;
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for id in 0..hashes.len() {
        let r = root(&mut parent, id);
        groups.entry(r).or_default().push(id);
    }
    Ok(groups.into_values().collect())
}
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn hashes_are_deterministic_and_transparent_rgb_is_irrelevant() {
        let a = RgbaImage::from_fn(32, 32, |x, y| {
            image::Rgba([(x * 7) as u8, (y * 7) as u8, 80, 255])
        });
        let mut b = a.clone();
        a.pixels()
            .zip(b.pixels_mut())
            .for_each(|(a, b)| assert_eq!(a, b));
        assert_eq!(hash(&a).phash, hash(&b).phash);
        let a = RgbaImage::from_pixel(32, 32, image::Rgba([0, 80, 20, 0]));
        let b = RgbaImage::from_pixel(32, 32, image::Rgba([220, 0, 90, 0]));
        assert_eq!(hash(&a).phash, hash(&b).phash);
        assert_eq!(hash(&a).ahash, 0);
        assert_eq!(hash(&a).dhash, 0);
    }
    #[test]
    fn bk_tree_matches_exhaustive_search_inclusive_radius() {
        let values: Vec<_> = (0u64..90)
            .map(|i| i.wrapping_mul(0x9e3779b97f4a7c15))
            .collect();
        let mut tree = BkTree::new(values.len());
        for (i, &v) in values.iter().enumerate() {
            tree.insert(v, i).expect("insert");
        }
        for query in [0, values[20], u64::MAX] {
            for radius in [0, 4, 25, 64] {
                let mut got = Vec::new();
                tree.query(query, radius, |id, d| got.push((id, d)));
                got.sort();
                let expected: Vec<_> = values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &v)| {
                        let d = (v ^ query).count_ones();
                        (d <= radius).then_some((i, d))
                    })
                    .collect();
                assert_eq!(got, expected);
            }
        }
    }
    #[test]
    fn clustering_preserves_transitive_links_and_collapses_duplicates() {
        assert_eq!(
            clusters(&[0, 1, 3, 3, u64::MAX], 1).expect("clusters"),
            vec![vec![0, 1, 2, 3], vec![4]]
        );
        let mut tree = BkTree::new(1);
        tree.insert(0, 0).expect("insert");
        assert_eq!(tree.insert(0, 1).expect("duplicate"), Some(0));
        assert!(tree.insert(1, 2).is_err());
    }
    #[test]
    #[ignore = "heavy: hashing-scale"]
    fn hundred_thousand_exact_duplicates_have_linear_storage() {
        let groups = clusters(&vec![42; 100000], 4).expect("100k");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 100000);
    }
}
