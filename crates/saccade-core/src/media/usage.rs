//! Generic image usage matching using hashes and the wave 6 keypoint estimator.
use super::{MediaError, Record, Result, SCHEMA as MEDIA_SCHEMA, Status};
use crate::{
    general::{hashing, input, registration},
    wave7::models,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
/// Versioned usage matches.
pub const SCHEMA: &str = "saccade-usage.v1";
/// A compact source suitable for matching without source pixels.
#[derive(Clone, Serialize, Deserialize)]
pub struct Source {
    /// Retained encoded content identity.
    pub sha256: String,
    /// Original dimensions.
    pub size: [u32; 2],
    /// Hash prefilter candidates, never a crop rejection threshold.
    pub hashes: hashing::Hashes,
    /// Bounded keypoints, <=1200.
    pub keypoints: Vec<registration::Keypoint>,
}
impl Source {
    /// Analyze source image bytes into usage primitives.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let im = input::decode(bytes)?;
        Ok(Self {
            sha256: models::digest(bytes),
            size: [im.width(), im.height()],
            hashes: hashing::hash(&im),
            keypoints: registration::fingerprint(&im),
        })
    }
    /// Load a current image media record or a raster, rejecting unavailable fingerprints.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = input::bytes(path, input::MAX_BYTES)?;
        if bytes.iter().copied().find(|c| !c.is_ascii_whitespace()) != Some(b'{') {
            return Self::from_bytes(&bytes);
        }
        let record: Record = serde_json::from_slice(&bytes)?;
        if record.schema != MEDIA_SCHEMA
            || record.identity.status != Status::Ok
            || record.fingerprints.status != Status::Ok
            || record.fingerprints.data["keypoint_algorithm"] != "FAST-oriented-BRIEF/1"
        {
            return Err(MediaError::new(
                "invalid_media_input",
                "usage source needs an image record with available pinned fingerprints",
            ));
        }
        let sha256 = record.identity.data["sha256"]
            .as_str()
            .ok_or_else(|| MediaError::new("invalid_media_input", "source record digest missing"))?
            .to_owned();
        let size = serde_json::from_value(record.fingerprints.data["size"].clone())?;
        let hashes = serde_json::from_value(record.fingerprints.data["hashes"].clone())?;
        let keypoints = serde_json::from_value(record.fingerprints.data["keypoints"].clone())?;
        let source = Self {
            sha256,
            size,
            hashes,
            keypoints,
        };
        source.validate()?;
        Ok(source)
    }
    fn validate(&self) -> Result<()> {
        if !models::valid_hash(&self.sha256)
            || self.size.contains(&0)
            || u64::from(self.size[0]) * u64::from(self.size[1]) > input::MAX_PIXELS
            || self.keypoints.len() > 1200
            || self.keypoints.iter().any(|p| {
                p.point.iter().any(|v| !v.is_finite())
                    || p.point[0] < 0.
                    || p.point[1] < 0.
                    || p.point[0] >= f64::from(self.size[0])
                    || p.point[1] >= f64::from(self.size[1])
            })
        {
            return Err(MediaError::new(
                "invalid_media_input",
                "source fingerprint geometry/count/digest invalid",
            ));
        }
        Ok(())
    }
}
fn project(m: &[f64; 9], x: f64, y: f64) -> Option<[f64; 2]> {
    let z = m[6] * x + m[7] * y + m[8];
    if !z.is_finite() || z.abs() < 1e-10 {
        return None;
    }
    let p = [
        (m[0] * x + m[1] * y + m[2]) / z,
        (m[3] * x + m[4] * y + m[5]) / z,
    ];
    p.iter().all(|v| v.is_finite()).then_some(p)
}
fn inverse(m: [f64; 9]) -> Option<[f64; 9]> {
    let [a, b, c, d, e, f, g, h, i] = m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !det.is_finite() || det.abs() < 1e-10 {
        return None;
    }
    Some([
        (e * i - f * h) / det,
        (c * h - b * i) / det,
        (b * f - c * e) / det,
        (f * g - d * i) / det,
        (a * i - c * g) / det,
        (c * d - a * f) / det,
        (d * h - e * g) / det,
        (b * g - a * h) / det,
        (a * e - b * d) / det,
    ])
}
fn crop(matrix: [f64; 9], source: [u32; 2], target: [u32; 2]) -> Option<[f64; 4]> {
    let inverse = inverse(matrix)?;
    let points = [
        [0., 0.],
        [target[0] as f64, 0.],
        [0., target[1] as f64],
        [target[0] as f64, target[1] as f64],
    ]
    .into_iter()
    .map(|p| project(&inverse, p[0], p[1]))
    .collect::<Option<Vec<_>>>()?;
    let minx = points
        .iter()
        .map(|p| p[0])
        .fold(f64::INFINITY, f64::min)
        .clamp(0., source[0] as f64);
    let maxx = points
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max)
        .clamp(0., source[0] as f64);
    let miny = points
        .iter()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min)
        .clamp(0., source[1] as f64);
    let maxy = points
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max)
        .clamp(0., source[1] as f64);
    (maxx > minx && maxy > miny).then_some([minx, miny, maxx - minx, maxy - miny])
}
/// Match a retained source fingerprint to one retained target.
/// Confidence is a geometric consensus score, explicitly uncalibrated, not a probability.
pub fn match_bytes(source: &Source, bytes: &[u8]) -> Result<Value> {
    source.validate()?;
    let image = input::decode(bytes)?;
    let target = [image.width(), image.height()];
    let hash = models::digest(bytes);
    let perceptual = hashing::hash(&image);
    let distance = (source.hashes.phash ^ perceptual.phash).count_ones();
    if hash == source.sha256 && target == source.size {
        return Ok(
            json!({"status":"matched","basis":"encoded-content-equality","target_sha256":hash,"transform":[1.,0.,0.,0.,1.,0.,0.,0.,1.],"crop_rectangle":[0.,0.,source.size[0] as f64,source.size[1] as f64],"confidence":1.,"calibration":"exact identity","hash_distance":distance}),
        );
    }
    // High global hash distance is not a negative crop match; registration still runs.
    let points = registration::fingerprint(&image);
    let (matrix, model, matches, inliers, residual) = match registration::fit_fingerprint(
        source.size,
        &source.keypoints,
        target,
        &points,
        registration::Model::Auto,
    ) {
        Ok(v) => v,
        Err(registration::RegistrationError::InsufficientInliers { matches }) => {
            return Ok(
                json!({"status":"no_match","basis":"insufficient keypoint consensus","target_sha256":hash,"hash_distance":distance,"matches":matches,"confidence":null}),
            );
        }
        Err(_) => {
            return Err(MediaError::new(
                "invalid_media_input",
                "usage fingerprint geometry invalid",
            ));
        }
    };
    let crop = crop(matrix, source.size, target).ok_or_else(|| {
        MediaError::new(
            "invalid_media_input",
            "usage transform has no source overlap",
        )
    })?;
    let confidence = (inliers as f64 / matches as f64) / (1. + residual / 3.);
    Ok(
        json!({"status":"matched","basis":"FAST-oriented-BRIEF-RANSAC/1","target_sha256":hash,"hash_distance":distance,"transform":matrix,"model":model,"matches":matches,"inliers":inliers,"residual_px":residual,"crop_rectangle":crop,"crop_basis":"source-space bounding rectangle of target coverage; projective/rotated edges are approximated","confidence":confidence,"calibration":"uncalibrated geometric consensus score"}),
    )
}
/// Search generic target files, preserving failed inputs as failed rows.
pub fn find_usage(source: &Source, targets: &[String]) -> Result<Value> {
    source.validate()?;
    if targets.is_empty() || targets.len() > 1000 {
        return Err(MediaError::new(
            "invalid_media_options",
            "declare 1..1000 target images",
        ));
    }
    let mut rows = Vec::new();
    for target in targets {
        let result =
            (|| match_bytes(source, &input::bytes(Path::new(target), input::MAX_BYTES)?))();
        let mut row = match result {
            Ok(r) => r,
            Err(e) => json!({"status":"failed","error_code":e.code,"reason":e.message}),
        };
        row["target"] = json!(target);
        rows.push(row);
    }
    Ok(
        json!({"schema":SCHEMA,"source_sha256":source.sha256,"targets":rows,"limitations":["hash distance is a candidate hint; crop candidates still run keypoint registration","low-texture images may have insufficient keypoints","geometric confidence is uncalibrated and does not establish rights or publication provenance"]}),
    )
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn encoded(im: &image::RgbaImage) -> Vec<u8> {
        let mut c = std::io::Cursor::new(Vec::new());
        im.write_to(&mut c, image::ImageFormat::Png).unwrap();
        c.into_inner()
    }
    fn texture() -> image::RgbaImage {
        let mut state = 938475u64;
        image::RgbaImage::from_fn(192, 160, |x, y| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let v = (state % 256) as u8;
            image::Rgba([v, ((x * 17 + y * 23) % 256) as u8, 255 - v, 255])
        })
    }
    #[test]
    fn generated_crop_and_jpeg_reencoding_have_consensus() {
        let im = texture();
        let source = Source::from_bytes(&encoded(&im)).unwrap();
        let crop = image::imageops::crop_imm(&im, 20, 12, 144, 128).to_image();
        let hit = match_bytes(&source, &encoded(&crop)).unwrap();
        assert_eq!(hit["status"], "matched", "{hit}");
        assert!(hit["inliers"].as_u64().unwrap() >= 6);
        assert!(hit["crop_rectangle"][0].as_f64().unwrap() > 10.);
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(im.clone())
            .to_rgb8()
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .unwrap();
        assert_eq!(
            match_bytes(&source, &jpeg.into_inner()).unwrap()["status"],
            "matched"
        );
        let resized = image::imageops::resize(&im, 136, 113, image::imageops::FilterType::Triangle);
        assert_eq!(
            match_bytes(&source, &encoded(&resized)).unwrap()["status"],
            "matched"
        );
        assert_eq!(
            match_bytes(&source, &encoded(&im)).unwrap()["basis"],
            "encoded-content-equality"
        );
    }
    #[test]
    fn insufficient_texture_and_failed_target_are_explicit() {
        let im = image::RgbaImage::from_pixel(40, 30, image::Rgba([40, 40, 40, 255]));
        let s = Source::from_bytes(&encoded(&im)).unwrap();
        let other = image::RgbaImage::from_pixel(40, 30, image::Rgba([90, 90, 90, 255]));
        assert_eq!(
            match_bytes(&s, &encoded(&other)).unwrap()["status"],
            "no_match"
        );
        let r = find_usage(&s, &["/nonexistent/image.png".into()]).unwrap();
        assert_eq!(r["targets"][0]["status"], "failed");
    }
}
