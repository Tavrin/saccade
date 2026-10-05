//! Compression-specific, externally encoded quality sweeps.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Frozen input artifact, relative to the manifest directory.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Relative filename.
    pub path: PathBuf,
    /// Expected lowercase SHA-256 of the encoded bytes.
    pub sha256: String,
}
/// One externally encoded pipeline stage.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    /// Stable pipeline stage ID.
    pub id: String,
    /// Encoder and version, including settings other than quality.
    pub encoder: String,
    /// Encoder-specific quality setting; no universal visibility interpretation.
    pub quality: f64,
    /// Explicit subsampling, such as 4:4:4.
    pub subsampling: String,
    /// Must be normalized_srgb_opaque; profile/orientation normalization is external.
    pub pixel_policy: String,
    /// Expected decoded dimensions.
    pub dimensions: [u32; 2],
    /// Encoded output.
    pub output: Artifact,
}
/// A candidate's ordered stages, all measured against the resized reference.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// Unique candidate name.
    pub id: String,
    /// Processing order, for example Drupal then delivered Imgix image.
    pub stages: Vec<Stage>,
}
/// Frozen sweep policy and its externally produced candidates.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be saccade-quality-sweep.v1.
    pub schema: String,
    /// Immutable original; never rewritten by the sweep.
    pub original: Artifact,
    /// Orientation/profile-normalized, resized opaque sRGB reference.
    pub reference: Artifact,
    /// Minimum cumulative SSIMULACRA2 score for every stage.
    pub minimum_score: f64,
    /// Maximum bytes of the delivered stage.
    pub maximum_bytes: u64,
    /// Display, scale, viewing distance, background and review conditions.
    pub viewing_conditions: String,
    /// Nonempty candidate list. Missing or malformed candidates prevent selection.
    pub candidates: Vec<Candidate>,
}
/// A measured processing stage. Scores are not additive across stages.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    /// Frozen stage declaration, including output hash.
    pub stage: Stage,
    /// Exact encoded bytes.
    pub bytes: u64,
    /// Quality against the preceding stage (or reference for the first stage).
    pub incremental_score: f64,
    /// Quality against the retained resized reference.
    pub cumulative_score: f64,
    /// Butteraugli distance from preceding stage, at 80 cd/m2; not additive.
    pub incremental_butteraugli: f64,
    /// Butteraugli distance from the retained reference, at 80 cd/m2.
    pub cumulative_butteraugli: f64,
}
/// One candidate's evidence or explicit error.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResult {
    /// Candidate ID.
    pub id: String,
    /// All measured stages before any failure.
    pub stages: Vec<StageResult>,
    /// True only when every stage meets the score budget and delivery meets byte budget.
    pub meets_policy: bool,
    /// Missing, hash-mismatched or unusable candidate reason.
    pub error: Option<String>,
}
/// Advisory sweep output. No visibility or approval assertion is made.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sweep {
    /// saccade-quality-report.v1.
    pub schema: String,
    /// Frozen policy and original/reference identities.
    pub manifest: Manifest,
    /// SHA-256 of the exact manifest bytes, assigned by the caller.
    pub manifest_sha256: String,
    /// Exact bytes of the preserved original.
    pub original_bytes: u64,
    /// Metric implementation and license.
    pub metric: String,
    /// Every supplied candidate, including errors.
    pub candidates: Vec<CandidateResult>,
    /// Smallest measured delivered file meeting policy; null for incomplete sweeps.
    pub selected_candidate: Option<String>,
    /// Lowest final encoder quality meeting policy, only for one encoder/settings family.
    pub lowest_quality_candidate: Option<String>,
    /// incomplete, no_candidate_meets_policy or complete.
    pub coverage: String,
    /// Always pending; a numerical threshold cannot prove invisibility.
    pub human_visual_review: String,
}

fn invalid(e: impl std::fmt::Display) -> Error {
    Error::Config(e.to_string())
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn bytes(root: &Path, a: &Artifact) -> Result<Vec<u8>> {
    if a.path.is_absolute()
        || a.path
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err(Error::Config(
            "sweep paths must be relative without traversal".into(),
        ));
    }
    let root = std::fs::canonicalize(if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    })
    .map_err(invalid)?;
    let path = std::fs::canonicalize(root.join(&a.path)).map_err(invalid)?;
    if !path.starts_with(&root) {
        return Err(Error::Config("sweep input escapes root".into()));
    }
    let data = std::fs::read(path).map_err(invalid)?;
    if digest(&data) != a.sha256 {
        return Err(Error::Config(format!(
            "hash mismatch: {}",
            a.path.display()
        )));
    }
    Ok(data)
}
fn decode(data: &[u8]) -> Result<image::RgbImage> {
    use image::ImageDecoder;
    let decoder = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(invalid)?
        .into_decoder()
        .map_err(invalid)?;
    let mut decoder = decoder;
    if decoder.icc_profile().map_err(invalid)?.is_some()
        || decoder.exif_metadata().map_err(invalid)?.is_some()
    {
        return Err(Error::Config(
            "normalize embedded profiles and orientation externally before sweeping".into(),
        ));
    }
    if decoder.color_type() != image::ColorType::Rgb8 {
        return Err(Error::Config(
            "quality sweep requires opaque RGB8, normalized sRGB".into(),
        ));
    }
    Ok(image::DynamicImage::from_decoder(decoder)
        .map_err(invalid)?
        .to_rgb8())
}
/// Computes pinned SSIMULACRA2 for normalized, opaque sRGB samples.
pub fn score(reference: &image::RgbImage, candidate: &image::RgbImage) -> Result<f64> {
    if reference.dimensions() != candidate.dimensions()
        || reference.width() < 8
        || reference.height() < 8
    {
        return Err(Error::Config(
            "SSIMULACRA2 requires equal dimensions, at least 8x8".into(),
        ));
    }
    fn rgb(i: &image::RgbImage) -> Result<ssimulacra2::Rgb> {
        ssimulacra2::Rgb::new(
            i.pixels().map(|p| p.0.map(|v| v as f32 / 255.0)).collect(),
            i.width() as usize,
            i.height() as usize,
            ssimulacra2::TransferCharacteristic::SRGB,
            ssimulacra2::ColorPrimaries::BT709,
        )
        .map_err(|e| Error::Config(e.to_string()))
    }
    let score = ssimulacra2::compute_frame_ssimulacra2(rgb(reference)?, rgb(candidate)?)
        .map_err(|e| Error::Config(e.to_string()))?;
    if !score.is_finite() {
        return Err(Error::Config("nonfinite SSIMULACRA2".into()));
    }
    Ok(score)
}
/// Computes pinned Butteraugli distance for opaque sRGB at 80 cd/m2.
/// This is supplementary measured evidence; sweep policy remains SSIMULACRA2.
pub fn butteraugli_distance(
    reference: &image::RgbImage,
    candidate: &image::RgbImage,
) -> Result<f64> {
    if reference.dimensions() != candidate.dimensions()
        || reference.width() < 8
        || reference.height() < 8
    {
        return Err(Error::Config(
            "Butteraugli requires equal dimensions, at least 8x8".into(),
        ));
    }
    let pixels = |i: &image::RgbImage| {
        butteraugli::Img::new(
            i.pixels()
                .map(|p| butteraugli::RGB8::new(p[0], p[1], p[2]))
                .collect::<Vec<_>>(),
            i.width() as usize,
            i.height() as usize,
        )
    };
    let a = pixels(reference);
    let b = pixels(candidate);
    let distance = butteraugli::butteraugli(
        a.as_ref(),
        b.as_ref(),
        &butteraugli::ButteraugliParams::default(),
    )
    .map_err(invalid)?
    .score;
    if !distance.is_finite() {
        return Err(Error::Config("nonfinite Butteraugli".into()));
    }
    Ok(distance)
}
/// Measures all candidates without invoking an encoder or modifying original files.
pub fn sweep(root: &Path, manifest: Manifest) -> Result<Sweep> {
    if manifest.schema != "saccade-quality-sweep.v1"
        || manifest.viewing_conditions.trim().is_empty()
        || !manifest.minimum_score.is_finite()
        || manifest.minimum_score > 100.0
        || manifest.maximum_bytes == 0
        || manifest.candidates.is_empty()
        || manifest.candidates.len() > 256
    {
        return Err(Error::Config(
            "invalid sweep schema, viewing conditions, budget or candidate count (1..256)".into(),
        ));
    }
    let original_bytes = bytes(root, &manifest.original)?.len() as u64;
    let reference = decode(&bytes(root, &manifest.reference)?)?;
    let mut ids = BTreeSet::new();
    let mut results = Vec::new();
    for candidate in &manifest.candidates {
        let mut result = CandidateResult {
            id: candidate.id.clone(),
            stages: vec![],
            meets_policy: false,
            error: None,
        };
        let measured = (|| -> Result<()> {
            if candidate.id.trim().is_empty()
                || !ids.insert(&candidate.id)
                || candidate.stages.is_empty()
                || candidate.stages.len() > 8
            {
                return Err(Error::Config(
                    "empty/duplicate candidate ID or stage count outside 1..8".into(),
                ));
            }
            let mut stage_ids = BTreeSet::new();
            let mut previous = reference.clone();
            for stage in &candidate.stages {
                if stage.id.trim().is_empty()
                    || !stage_ids.insert(&stage.id)
                    || stage.encoder.trim().is_empty()
                    || stage.subsampling.trim().is_empty()
                    || !stage.quality.is_finite()
                    || stage.pixel_policy != "normalized_srgb_opaque"
                {
                    return Err(Error::Config("incomplete encoder or pixel policy".into()));
                }
                let data = bytes(root, &stage.output)?;
                let image = decode(&data)?;
                if [image.width(), image.height()] != stage.dimensions {
                    return Err(Error::Config("declared dimensions differ".into()));
                }
                result.stages.push(StageResult {
                    stage: stage.clone(),
                    bytes: data.len() as u64,
                    incremental_score: score(&previous, &image)?,
                    cumulative_score: score(&reference, &image)?,
                    incremental_butteraugli: butteraugli_distance(&previous, &image)?,
                    cumulative_butteraugli: butteraugli_distance(&reference, &image)?,
                });
                previous = image;
            }
            result.meets_policy = result
                .stages
                .iter()
                .all(|s| s.cumulative_score >= manifest.minimum_score)
                && result
                    .stages
                    .last()
                    .is_some_and(|s| s.bytes <= manifest.maximum_bytes);
            Ok(())
        })();
        result.error = measured.err().map(|e| e.to_string());
        results.push(result);
    }
    let complete = results.iter().all(|r| r.error.is_none());
    let eligible: Vec<_> = results
        .iter()
        .filter(|r| r.meets_policy)
        .filter_map(|r| r.stages.last().map(|s| (r, s)))
        .collect();
    let selected = complete
        .then(|| {
            eligible
                .iter()
                .min_by_key(|(r, s)| (s.bytes, &r.id))
                .map(|(r, _)| r.id.clone())
        })
        .flatten();
    let families: BTreeSet<_> = eligible
        .iter()
        .map(|(_, s)| {
            (
                &s.stage.encoder,
                &s.stage.subsampling,
                &s.stage.pixel_policy,
                s.stage.dimensions,
            )
        })
        .collect();
    let lowest = if complete && families.len() == 1 {
        eligible
            .iter()
            .min_by(|(a, sa), (b, sb)| {
                sa.stage
                    .quality
                    .total_cmp(&sb.stage.quality)
                    .then(a.id.cmp(&b.id))
            })
            .map(|(r, _)| r.id.clone())
    } else {
        None
    };
    let coverage = if !complete {
        "incomplete"
    } else if selected.is_none() {
        "no_candidate_meets_policy"
    } else {
        "complete"
    }
    .into();
    Ok(Sweep {
        schema: "saccade-quality-report.v1".into(),
        manifest,
        manifest_sha256: String::new(),
        original_bytes,
        metric: "ssimulacra2 0.5.1 (BSD-2-Clause); butteraugli 0.4.0 (BSD-3-Clause), 80 cd/m2; sRGB/BT.709; synthetic reference qualification: libjxl v0.12.0 and Cloudinary v2.1".into(),
        candidates: results,
        selected_candidate: selected,
        lowest_quality_candidate: lowest,
        coverage,
        human_visual_review: "pending".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn sweep_counts_bytes_stages_and_refuses_missing_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let original = image::RgbImage::from_fn(32, 32, |x, y| {
            image::Rgb([(x * 7) as u8, (y * 5) as u8, 120])
        });
        original.save(dir.path().join("reference.png")).unwrap();
        let artifact = |name: &str| Artifact {
            path: name.into(),
            sha256: digest(&std::fs::read(dir.path().join(name)).unwrap()),
        };
        let mut candidates = vec![];
        for quality in [40u8, 80, 95] {
            let name = format!("q{quality}.jpg");
            let mut data = vec![];
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, quality)
                .encode_image(&original)
                .unwrap();
            std::fs::write(dir.path().join(&name), data).unwrap();
            let stage = Stage {
                id: "drupal".into(),
                encoder: "image 0.25 JPEG".into(),
                quality: quality as f64,
                subsampling: "4:4:4".into(),
                pixel_policy: "normalized_srgb_opaque".into(),
                dimensions: [32, 32],
                output: artifact(&name),
            };
            let mut delivery = stage.clone();
            delivery.id = "delivery".into();
            candidates.push(Candidate {
                id: format!("q{quality}"),
                stages: vec![stage, delivery],
            });
        }
        let manifest = Manifest {
            schema: "saccade-quality-sweep.v1".into(),
            original: artifact("reference.png"),
            reference: artifact("reference.png"),
            minimum_score: 0.0,
            maximum_bytes: 100_000,
            viewing_conditions: "sRGB display; 100% scale; 60cm; black background; review pending"
                .into(),
            candidates,
        };
        assert!((score(&original, &original).unwrap() - 100.0).abs() < 1e-5);
        let result = sweep(dir.path(), manifest.clone()).unwrap();
        assert_eq!(result.coverage, "complete");
        assert_eq!(result.lowest_quality_candidate.as_deref(), Some("q40"));
        for c in &result.candidates {
            assert_eq!(c.stages[1].incremental_score, 100.0);
            assert_eq!(
                c.stages[0].bytes,
                std::fs::metadata(dir.path().join(&c.stages[0].stage.output.path))
                    .unwrap()
                    .len()
            );
        }
        let mut missing = manifest.clone();
        missing.candidates[0].stages[0].output.path = "missing.jpg".into();
        let result = sweep(dir.path(), missing).unwrap();
        assert_eq!(result.coverage, "incomplete");
        assert!(result.selected_candidate.is_none());
        let mut mismatch = manifest.clone();
        mismatch.candidates[0].stages[0].output.sha256 = "wrong".into();
        assert!(
            sweep(dir.path(), mismatch)
                .unwrap()
                .selected_candidate
                .is_none()
        );
        let mut wrong = manifest;
        wrong.viewing_conditions.clear();
        assert!(sweep(dir.path(), wrong).is_err());
        let rgba = image::RgbaImage::from_pixel(32, 32, image::Rgba([1, 2, 3, 128]));
        let mut data = std::io::Cursor::new(vec![]);
        rgba.write_to(&mut data, image::ImageFormat::Png).unwrap();
        assert!(decode(data.get_ref()).is_err());
    }
}
