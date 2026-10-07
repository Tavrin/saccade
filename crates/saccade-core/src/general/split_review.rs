//! Bounded split-aware candidate review, using existing hash and geometric routes.
use super::{embedding, hashing, input, registration};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

/// Declared split membership input schema.
pub const MANIFEST_SCHEMA: &str = "saccade-split-manifest.v1";
/// Unlinked review schema; persistence adds links using its v2 successor.
pub const SCHEMA: &str = "saccade-split-review.v1";
/// Exhaustive pair review is deliberately bounded, unlike the retrieval index.
pub const MAX_ENTRIES: usize = 128;
/// One explicitly declared file and partition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Portable path relative to the manifest directory.
    pub path: String,
    /// Arbitrary named partition (including a single partition for burst review).
    pub split: String,
}
/// A known injected cross-partition pair for recall measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Injection {
    /// First declared path.
    pub a: String,
    /// Second declared path.
    pub b: String,
    /// Declared transformation label, not an inferred transformation.
    pub transform: String,
}
/// Complete scope of a review; undeclared files are outside its scope.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must equal MANIFEST_SCHEMA.
    pub schema: String,
    /// At most MAX_ENTRIES explicit entries.
    pub entries: Vec<Entry>,
    /// Optional injected truth; absent truth yields null recall, never perfect recall.
    #[serde(default)]
    pub injected_pairs: Vec<Injection>,
}
fn invalid(message: &str) -> Error {
    Error::Config(message.into())
}
fn key(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.into(), b.into())
    } else {
        (b.into(), a.into())
    }
}
impl Manifest {
    /// Validate membership/truth and resolve all inputs inside the manifest's directory.
    /// Canonical aliases are rejected so one file cannot receive ambiguous membership.
    pub fn resolve(&self, root: &Path) -> Result<Vec<PathBuf>> {
        if self.schema != MANIFEST_SCHEMA
            || self.entries.is_empty()
            || self.entries.len() > MAX_ENTRIES
            || self.injected_pairs.len() > MAX_ENTRIES * (MAX_ENTRIES - 1) / 2
        {
            return Err(invalid("invalid split manifest schema/count"));
        }
        let root = crate::paths::canonicalize(root)
            .map_err(crate::run::io_err("resolving split root".into()))?;
        let mut names = BTreeMap::new();
        let mut canonical = BTreeSet::new();
        let mut paths = Vec::new();
        for entry in &self.entries {
            if entry.path.is_empty()
                || entry.path.len() > 4096
                || entry.path.contains('\\')
                || !Path::new(&entry.path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                || entry.split.trim().is_empty()
                || entry.split.len() > 128
                || names
                    .insert(entry.path.as_str(), entry.split.as_str())
                    .is_some()
            {
                return Err(invalid("invalid or duplicate split membership"));
            }
            let path = crate::paths::canonicalize(root.join(&entry.path))
                .map_err(crate::run::io_err("resolving split input".into()))?;
            if !path.starts_with(&root) || !canonical.insert(path.clone()) {
                return Err(invalid("split input escapes root or aliases another entry"));
            }
            paths.push(path);
        }
        let mut truth = BTreeSet::new();
        for pair in &self.injected_pairs {
            let a = names.get(pair.a.as_str());
            let b = names.get(pair.b.as_str());
            if a.is_none()
                || b.is_none()
                || a == b
                || pair.transform.trim().is_empty()
                || pair.transform.len() > 128
                || !truth.insert(key(&pair.a, &pair.b))
            {
                return Err(invalid(
                    "injected pairs must be unique, declared and cross-split",
                ));
            }
        }
        Ok(paths)
    }
}
struct Fingerprint {
    sha256: String,
    size: [u32; 2],
    hash: u64,
    points: Vec<registration::Keypoint>,
    vector: Option<Vec<f32>>,
}
/// Optional provisioned embedding route. Implementations must never download implicitly.
pub trait EmbeddingRoute {
    /// Serialized model/preprocessing identity included in evidence.
    fn identity(&self) -> &str;
    /// Compute one vector from the same decoded input used by the stock routes.
    fn embed(&mut self, image: &image::RgbaImage) -> Result<Vec<f32>>;
}
/// Review every unordered pair, emit only cross-split candidates and within-split groups.
/// Decode/runtime errors fail the review rather than yielding a misleading clean list.
pub fn review(
    manifest: &Manifest,
    root: &Path,
    hash_threshold: u32,
    cosine_threshold: f64,
    mut embedder: Option<&mut dyn EmbeddingRoute>,
) -> Result<Value> {
    if hash_threshold > 64
        || !cosine_threshold.is_finite()
        || !(-1.0..=1.0).contains(&cosine_threshold)
    {
        return Err(invalid("invalid split-review threshold"));
    }
    let paths = manifest.resolve(root)?;
    let model = embedder.as_ref().map(|e| e.identity().to_owned());
    if model
        .as_ref()
        .is_some_and(|id| !crate::wave7::models::valid_hash(id))
    {
        return Err(invalid(
            "embedding identity must be a model-contract SHA-256",
        ));
    }
    let mut measured = Vec::new();
    let mut entries = Vec::new();
    for (entry, path) in manifest.entries.iter().zip(paths) {
        let bytes = input::bytes(&path, input::MAX_BYTES)?;
        let image = input::decode(&bytes)?;
        let mut vector = embedder.as_mut().map(|e| e.embed(&image)).transpose()?;
        if let Some(v) = &mut vector {
            if v.len() > 4096 {
                return Err(invalid("embedding vector exceeds 4096 dimensions"));
            }
            embedding::normalize(v)?;
        }
        let fp = Fingerprint {
            sha256: crate::localized::digest(&bytes),
            size: [image.width(), image.height()],
            hash: hashing::hash(&image).phash,
            points: registration::fingerprint(&image),
            vector,
        };
        entries.push(
            json!({"path":entry.path,"split":entry.split,"encoded_sha256":fp.sha256,
            "dimensions":fp.size,"phash":format!("{:016x}",fp.hash),"keypoints":fp.points.len()}),
        );
        measured.push(fp);
    }
    let mut candidates = Vec::new();
    let mut within = Vec::new();
    let mut detected = BTreeSet::new();
    let mut parent: Vec<_> = (0..measured.len()).collect();
    let mut geometry_matches = 0;
    let mut geometry_attempts = 0;
    for a in 0..measured.len() {
        for b in a + 1..measured.len() {
            let left = &measured[a];
            let right = &measured[b];
            let distance = (left.hash ^ right.hash).count_ones();
            let mut evidence = Vec::new();
            if left.sha256 == right.sha256 {
                evidence.push(
                    json!({"route":"hash","basis":"encoded_sha256_equality","sha256":left.sha256}),
                );
            } else {
                if distance <= hash_threshold {
                    evidence.push(json!({"route":"hash","basis":"phash_candidate","distance":distance,"threshold":hash_threshold}));
                }
                geometry_attempts += 1;
                match registration::fit_fingerprint(
                    left.size,
                    &left.points,
                    right.size,
                    &right.points,
                    registration::Model::Auto,
                ) {
                    Ok((matrix, model, matches, inliers, residual)) => {
                        geometry_matches += 1;
                        evidence.push(json!({"route":"geometric","basis":"FAST-oriented-BRIEF-RANSAC/1",
                            "direction":"a_to_b","matrix":matrix,"model":model,"matches":matches,
                            "inliers":inliers,"residual_px":residual,"calibration":"uncalibrated geometric consensus"}));
                    }
                    Err(registration::RegistrationError::InsufficientInliers { .. }) => {}
                    Err(e) => return Err(invalid(&e.to_string())),
                }
            }
            if let (Some(x), Some(y)) = (&left.vector, &right.vector) {
                let cosine = embedding::cosine(x, y)?;
                if cosine >= cosine_threshold {
                    evidence.push(json!({"route":"embedding","basis":"cosine_candidate","cosine":cosine,
                        "threshold":cosine_threshold,"model_id":model,"calibration":"raw cosine; not identity"}));
                }
            }
            if evidence.is_empty() {
                continue;
            }
            let x = &manifest.entries[a];
            let y = &manifest.entries[b];
            let row = json!({"a":x.path,"b":y.path,"a_split":x.split,"b_split":y.split,"evidence":evidence});
            if x.split != y.split {
                detected.insert(key(&x.path, &y.path));
                candidates.push(row);
            } else {
                within.push(row);
                let ra = component(&mut parent, a);
                let rb = component(&mut parent, b);
                parent[rb] = ra;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for (i, entry) in manifest.entries.iter().enumerate() {
        groups
            .entry(component(&mut parent, i))
            .or_default()
            .push(&entry.path);
    }
    let groups: Vec<_> = groups
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(i, members)| json!({"split":manifest.entries[i].split,"members":members}))
        .collect();
    let mut by_transform: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut misses = Vec::new();
    let mut hits = 0;
    for pair in &manifest.injected_pairs {
        let found = detected.contains(&key(&pair.a, &pair.b));
        hits += usize::from(found);
        let tally = by_transform.entry(&pair.transform).or_default();
        tally.0 += 1;
        tally.1 += usize::from(found);
        if !found {
            misses.push(pair);
        }
    }
    let recall = |n: usize, h: usize| {
        if n == 0 {
            None
        } else {
            Some(h as f64 / n as f64)
        }
    };
    let by_transform: Vec<_> = by_transform.into_iter().map(|(transform,(total,found))|
        json!({"transform":transform,"total":total,"found":found,"recall":recall(total,found)})).collect();
    let limitations = [
        "No candidates is not proof of no leakage; only declared files and enabled routes were reviewed.",
        "Perceptual hashes collide, especially on flat images, and can miss crops or large edits.",
        "Geometry runs independently of global hash distance; low texture, repeated patterns, severe crops and transforms can fail consensus or create false matches.",
        "Embedding cosine is semantic candidate evidence, not duplicate identity; an absent model leaves this route unused.",
        "Groups are transitive connected components within each split; not every pair in a group necessarily matches. No ranking, deletion or approval is implied.",
        "Recall covers only declared injected pairs, including misses; it is not population recall or precision. Exhaustive review is capped at 128 files.",
    ];
    Ok(json!({"schema":SCHEMA,"operation":"split-review",
        "verdict":if candidates.is_empty(){"no_candidates"}else{"review_required"},
        "summary":if candidates.is_empty(){"No cross-split candidates found by the declared routes; this is not proof of no leakage."}else{"Cross-split reuse candidates require review; route evidence is not approval."},
        "manifest_sha256":crate::localized::digest(&serde_json::to_vec(manifest)?),
        "routes":[{"route":"hash","status":"used","algorithm":"phash-white-triangle-srgb-luma-v1","threshold":hash_threshold},
            {"route":"geometric","status":"used","attempted_pairs":geometry_attempts,"matched_pairs":geometry_matches},
            {"route":"embedding","status":if model.is_some(){"used"}else{"not_enabled"},"model_id":model,"threshold":cosine_threshold}],
        "counts":{"images":entries.len(),"cross_split_candidates":candidates.len(),"within_split_groups":groups.len()},
        "entries":entries,"cross_split_pairs":candidates,"within_split_pairs":within,"groups":groups,
        "injected_recall":{"total":manifest.injected_pairs.len(),"found":hits,"recall":recall(manifest.injected_pairs.len(),hits),"by_transform":by_transform,"missed_pairs":misses},
        "limitations":limitations}))
}
fn component(parent: &mut [usize], mut id: usize) -> usize {
    while parent[id] != id {
        parent[id] = parent[parent[id]];
        id = parent[id];
    }
    id
}
/// CSV for human review; every pair includes all route evidence and split names.
pub fn csv(report: &Value) -> Result<String> {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = String::from("a,b,a_split,b_split,evidence\n");
    for row in report["cross_split_pairs"]
        .as_array()
        .ok_or_else(|| invalid("missing split pairs"))?
    {
        let mut cells = Vec::new();
        for field in ["a", "b", "a_split", "b_split"] {
            cells.push(quote(
                row[field]
                    .as_str()
                    .ok_or_else(|| invalid("invalid split pair"))?,
            ));
        }
        cells.push(quote(&serde_json::to_string(&row["evidence"])?));
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    Ok(out)
}
