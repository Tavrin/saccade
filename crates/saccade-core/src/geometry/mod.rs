//! Deterministic f64 triangle-surface measurements. No implicit registration.
mod load;
use crate::evidence::analysis::{Analysis, Capability, Provenance, Resource};
use parry3d_f64::{
    na::{Point3, Vector3},
    query::PointQueryWithLocation,
    shape::{TriMesh, TrianglePointLocation},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub use load::load;

/// Static mesh in a declared world coordinate frame, preserving f64 OBJ input.
pub struct Mesh {
    /// Ordered transformed vertices.
    pub vertices: Vec<Point3<f64>>,
    /// Ordered, oriented triangles.
    pub triangles: Vec<[u32; 3]>,
    /// Original document and mesh buffer identities.
    pub resources: Vec<Resource>,
    /// Loader scope and unsupported appearance features.
    pub limitations: Vec<String>,
}
impl Mesh {
    /// Validates finite, nondegenerate triangle geometry before spatial queries.
    pub fn validate(&self) -> crate::Result<()> {
        if self.vertices.is_empty()
            || self.triangles.is_empty()
            || self.vertices.len() > 1_000_000
            || self.triangles.len() > 1_000_000
        {
            return Err(error("geometry needs 1..1000000 vertices and triangles"));
        }
        if self
            .vertices
            .iter()
            .any(|p| p.coords.iter().any(|v| !v.is_finite()))
        {
            return Err(error("nonfinite mesh coordinates"));
        }
        for t in &self.triangles {
            if t.iter().any(|&i| i as usize >= self.vertices.len()) {
                return Err(error("triangle index out of range"));
            }
            let (area, _) = self.face(*t);
            if !area.is_finite() || area <= 0.0 {
                return Err(error("degenerate or overflowing triangle"));
            }
        }
        Ok(())
    }
    fn face(&self, t: [u32; 3]) -> (f64, Vector3<f64>) {
        let [a, b, c] = t.map(|i| self.vertices[i as usize]);
        let cross = (b - a).cross(&(c - a));
        let len = cross.norm();
        (len * 0.5, cross / len)
    }
    fn digest(&self, unit: &str) -> String {
        let mut h = Sha256::new();
        h.update(b"ordered-world-triangle-geometry/1\0");
        h.update((unit.len() as u64).to_le_bytes());
        h.update(unit.as_bytes());
        h.update((self.vertices.len() as u64).to_le_bytes());
        for p in &self.vertices {
            for x in p.coords.iter() {
                h.update(x.to_bits().to_le_bytes());
            }
        }
        h.update((self.triangles.len() as u64).to_le_bytes());
        for t in &self.triangles {
            for i in t {
                h.update(i.to_le_bytes());
            }
        }
        format!("{:x}", h.finalize())
    }
}
pub(crate) fn error(message: impl std::fmt::Display) -> crate::Error {
    crate::Error::Config(format!("geometry: {message}"))
}

/// Exact identity of the declared geometry, distinct from sampled surface similarity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Identity {
    /// Exact ordered f64 world vertices and u32 triangle indices, including unit declaration.
    pub ordered_geometry_identical: bool,
    /// Source document and loaded geometry buffer bytes match in resource order.
    pub source_geometry_resources_identical: bool,
    /// Content identities of the decoded ordered geometry.
    pub geometry_sha256: [String; 2],
    /// Units supplied by the caller; no implicit unit conversion.
    pub unit: String,
    /// This proof does not establish equal UVs, materials, textures or shading normals.
    pub appearance_attributes: Capability,
}

/// Proves exact ordered geometric identity; reordered topology may still have zero distance.
pub fn identity(a: &Mesh, b: &Mesh, unit: &str) -> crate::Result<Analysis<Identity>> {
    a.validate()?;
    b.validate()?;
    if unit.trim().is_empty() {
        return Err(error("declare a nonempty unit"));
    }
    let hashes = [a.digest(unit), b.digest(unit)];
    Ok(Analysis {
        capability: Capability::Available,
        provenance: Some(provenance(a, b, "mesh-identity/1", unit)),
        evidence: Some(Identity {
            ordered_geometry_identical: hashes[0] == hashes[1],
            source_geometry_resources_identical: a
                .resources
                .iter()
                .map(|r| (&r.role, &r.sha256))
                .eq(b.resources.iter().map(|r| (&r.role, &r.sha256)))
                && !a.resources.is_empty()
                && a.resources
                    .iter()
                    .chain(&b.resources)
                    .all(|r| r.sha256.is_some()),
            geometry_sha256: hashes,
            unit: unit.into(),
            appearance_attributes: Capability::Unsupported {
                reason: "geometry_identity_does_not_prove_appearance_attributes".into(),
            },
        }),
        limitations: limitations(a, b),
    })
}
fn limitations(a: &Mesh, b: &Mesh) -> Vec<String> {
    let mut limits = vec!["Static triangle geometry only; no ICP, registration or unit conversion. Appearance, topology equivalence and renderer quality are separate claims.".into()];
    limits.extend(a.limitations.iter().chain(&b.limitations).cloned());
    limits.sort();
    limits.dedup();
    limits
}
fn provenance(a: &Mesh, b: &Mesh, algorithm: &str, unit: &str) -> Provenance {
    let mut p = Provenance::native(algorithm);
    for (side, m) in [("baseline", a), ("capture", b)] {
        for r in &m.resources {
            let mut r = r.clone();
            r.name = format!("{side}:{}", r.name);
            p.resources.push(r);
        }
    }
    p.settings.extend([
        ("unit".into(), unit.into()),
        (
            "query".into(),
            "parry3d-f64 0.21.1; surface projection solid=false; Apache-2.0".into(),
        ),
        (
            "coordinates".into(),
            "f64 world coordinates; source node transforms applied; no alignment".into(),
        ),
    ]);
    p
}

/// Directed area-weighted distance distribution and separate defect probes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Directed {
    /// Deterministic area samples; each triangle gets at least one.
    pub area_samples: usize,
    /// Additional vertex/edge probes used only for the maximum.
    pub defect_probes: usize,
    /// Total sampled source surface area.
    pub surface_area: f64,
    /// Maximum among area samples and defect probes; a sampled lower estimate.
    pub maximum: f64,
    /// Unsquared distance in declared length units, weighted by triangle area.
    pub mean: f64,
    /// Root mean squared distance in declared length units.
    pub rms: f64,
    /// Weighted 95th percentile over area samples.
    pub p95: f64,
    /// Weighted 99th percentile over area samples.
    pub p99: f64,
    /// Area-weighted oriented face-normal angle in degrees where correspondence is unambiguous.
    pub normal_mean_degrees: Option<f64>,
    /// Largest measured oriented face-normal angle.
    pub normal_max_degrees: Option<f64>,
    /// Fraction of area samples whose projection hits an edge/vertex; normal correspondence unknown.
    pub normal_ambiguous_area_fraction: f64,
}
/// Bidirectional surface evidence, never a certified continuous Hausdorff bound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct GeometryDiff {
    /// Declared length units.
    pub unit: String,
    /// Ordered geometry identity, a separate exact claim.
    pub identity: Identity,
    /// Baseline surface samples projected to capture triangles.
    pub baseline_to_capture: Directed,
    /// Capture surface samples projected to baseline triangles.
    pub capture_to_baseline: Directed,
    /// Maximum of the directed sampled maxima; lower estimate subject to floating-point error.
    pub sampled_hausdorff: f64,
    /// Arithmetic average of the directed unsquared means.
    pub mean: f64,
    /// Square root of the average directed mean squared distances.
    pub rms: f64,
}
fn radical_inverse(mut i: usize) -> f64 {
    let (mut out, mut weight) = (0.0, 0.5);
    while i > 0 {
        out += (i & 1) as f64 * weight;
        weight *= 0.5;
        i >>= 1;
    }
    out
}
fn directed(source: &Mesh, target: &Mesh, budget: usize) -> crate::Result<Directed> {
    let surface = TriMesh::new(target.vertices.clone(), target.triangles.clone()).map_err(error)?;
    let total: f64 = source.triangles.iter().map(|t| source.face(*t).0).sum();
    if !total.is_finite() || total <= 0.0 {
        return Err(error("invalid total surface area"));
    }
    let (mut maximum, mut sum, mut squares, mut angle_sum, mut angle_max, mut normal_weight) =
        (0.0f64, 0.0, 0.0, 0.0, 0.0f64, 0.0);
    let mut distances = Vec::new();
    let mut probes = 0;
    for t in &source.triangles {
        let [a, b, c] = t.map(|i| source.vertices[i as usize]);
        let (area, normal) = source.face(*t);
        // Per-triangle stratification guarantees tiny-component coverage. The
        // requested budget is approximate; actual count <= budget + triangles.
        let n = ((budget as f64 * area / total).ceil() as usize).max(1);
        let weight = area / total / n as f64;
        for i in 0..n {
            let u = ((i as f64 + 0.5) / n as f64).sqrt();
            let v = radical_inverse(i + 1);
            let point = a + (b - a) * (u * (1.0 - v)) + (c - a) * (u * v);
            let (projection, (face, location)) =
                surface.project_local_point_and_get_location(&point, false);
            let distance = (point - projection.point).norm();
            if !distance.is_finite() {
                return Err(error("nonfinite surface distance"));
            }
            maximum = maximum.max(distance);
            sum += distance * weight;
            squares += distance * distance * weight;
            distances.push((distance, weight));
            if matches!(location, TrianglePointLocation::OnFace(..)) {
                let (_, other) = target.face(target.triangles[face as usize]);
                let angle = normal.dot(&other).clamp(-1.0, 1.0).acos().to_degrees();
                angle_sum += angle * weight;
                angle_max = angle_max.max(angle);
                normal_weight += weight;
            }
        }
        for point in [
            a,
            b,
            c,
            Point3::from((a.coords + b.coords) * 0.5),
            Point3::from((b.coords + c.coords) * 0.5),
            Point3::from((c.coords + a.coords) * 0.5),
        ] {
            let (projection, _) = surface.project_local_point_and_get_location(&point, false);
            let distance = (point - projection.point).norm();
            if !distance.is_finite() {
                return Err(error("nonfinite defect-probe distance"));
            }
            maximum = maximum.max(distance);
            probes += 1;
        }
    }
    if !squares.is_finite() {
        return Err(error("distance reduction overflow"));
    }
    distances.sort_by(|a, b| a.0.total_cmp(&b.0));
    let quantile = |q: f64| {
        let mut cumulative = 0.0;
        for &(d, w) in &distances {
            cumulative += w;
            if cumulative >= q {
                return d;
            }
        }
        distances.last().map_or(0.0, |v| v.0)
    };
    Ok(Directed {
        area_samples: distances.len(),
        defect_probes: probes,
        surface_area: total,
        maximum,
        mean: sum,
        rms: squares.sqrt(),
        p95: quantile(0.95),
        p99: quantile(0.99),
        normal_mean_degrees: (normal_weight > 0.0)
            .then_some(angle_sum / normal_weight.max(f64::MIN_POSITIVE)),
        normal_max_degrees: (normal_weight > 0.0).then_some(angle_max),
        normal_ambiguous_area_fraction: (1.0 - normal_weight).clamp(0.0, 1.0),
    })
}
/// Measures both directions with deterministic area samples and defect probes.
pub fn compare(
    a: &Mesh,
    b: &Mesh,
    unit: &str,
    samples: usize,
) -> crate::Result<Analysis<GeometryDiff>> {
    if !(1..=100_000).contains(&samples) {
        return Err(error("samples must be 1..100000"));
    }
    let identity = identity(a, b, unit)?
        .evidence
        .ok_or_else(|| error("identity unavailable"))?;
    let forward = directed(a, b, samples)?;
    let backward = directed(b, a, samples)?;
    let mut p = provenance(a, b, "area-stratified-surface/1", unit);
    p.settings.extend([("sampler".into(),"per-face Hammersley barycentric strata; ceil(budget * area / total), min 1; vertices and edge midpoints are max-only probes".into()),("requested_samples_per_direction".into(),samples.to_string()),("reduction_order".into(),"source triangle then sample index, sequential f64".into()),("random_stream".into(),"none; deterministic radical-inverse sequence starts at 1 per triangle".into())]);
    let mut limits = limitations(a, b);
    limits.push("Sampled Hausdorff is a lower estimate, not a certified upper bound; narrow defects between probes can be missed. Normal ties at edges/vertices are unknown; overlapping surfaces may remain ambiguous.".into());
    Ok(Analysis {
        capability: Capability::Available,
        provenance: Some(p),
        evidence: Some(GeometryDiff {
            unit: unit.into(),
            identity,
            sampled_hausdorff: forward.maximum.max(backward.maximum),
            mean: (forward.mean + backward.mean) * 0.5,
            rms: forward.rms.hypot(backward.rms) / 2.0f64.sqrt(),
            baseline_to_capture: forward,
            capture_to_baseline: backward,
        }),
        limitations: limits,
    })
}
/// Loads and measures two mesh files in their declared coordinate frame.
pub fn compare_files(
    a: &Path,
    b: &Path,
    unit: &str,
    samples: usize,
) -> crate::Result<Analysis<GeometryDiff>> {
    compare(&load(a)?, &load(b)?, unit, samples)
}

/// Versioned standalone mesh evidence document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Document {
    /// Always saccade-geometry.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-geometry.v1")))]
    pub schema: String,
    /// Optional supplied finite-camera render evidence. Historical reports omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<crate::asset_views::Report>,
    /// Discriminated measurement or exact proof.
    #[serde(flatten)]
    pub operation: Operation,
}
/// Separates sampled measurements from exact geometric identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Operation {
    /// Sampled bidirectional measurements.
    Geometry {
        /// Measurement and its declared limits.
        analysis: Box<Analysis<GeometryDiff>>,
    },
    /// Exact ordered geometry proof.
    MeshIdentity {
        /// Proof and its declared limits.
        analysis: Box<Analysis<Identity>>,
    },
}
