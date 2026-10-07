//! Hash-pinned, explicitly configured CPU ONNX embeddings; no bundled checkpoint/export.
use crate::{Error, Result, semantic};
use serde::{Deserialize, Serialize};
#[cfg(feature = "embeddings")]
use std::path::Path;
/// Supplied embedding export contract schema.
pub const MODEL_SCHEMA: &str = "saccade-embedding-model.v1";
/// Similarity evidence schema.
pub const SIMILAR_SCHEMA: &str = "saccade-similar.v1";
/// Flat embedding index metadata schema.
pub const INDEX_SCHEMA: &str = "saccade-embedding-index.v1";
/// Query evidence schema.
pub const QUERY_SCHEMA: &str = "saccade-embedding-query.v1";
/// A supplied calibration band, never inferred from cosine alone.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    /// Inclusive lower cosine bound.
    pub minimum: f64,
    /// Calibration's label.
    pub label: String,
}
/// Supplied calibration provenance; still requires external qualification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Calibration {
    /// SHA-256 identifying the calibration corpus.
    pub corpus_sha256: String,
    /// Scope and qualification limitations.
    pub scope: String,
    /// Increasing lower bounds; at most sixteen bands.
    pub bands: Vec<Band>,
}
/// Joint text tower and tokenizer, bound into the same index identity as the image tower.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextModel {
    /// Hash-pinned local CPU ONNX text tower.
    pub artifact: semantic::ModelArtifact,
    /// Official tokenizer JSON from the same immutable checkpoint revision.
    pub tokenizer: semantic::ModelArtifact,
    /// Exact int64 `[1,length]` input name.
    pub input: String,
    /// Exact float32 `[1,dimensions]` output name.
    pub output: String,
    /// SigLIP 2 fixed context length.
    pub length: usize,
    /// Official right-padding token id.
    pub pad_id: u32,
}
/// Fully declared export interface and preprocessing, bound to a hash-pinned artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// saccade-embedding-model.v1.
    pub schema: String,
    /// dinov2-small; the checkpoint licence is Apache-2.0 per the lane specification.
    pub family: String,
    /// The reviewed self-contained ONNX export; weights are never vendored.
    pub artifact: semantic::ModelArtifact,
    /// Exact NCHW input tensor name.
    pub input: String,
    /// Exact pooled `[1,dimensions]` f32 output tensor name.
    pub output: String,
    /// Resize width,height; explicit triangle resize over white, no hidden crop.
    pub size: [u32; 2],
    /// RGB mean after dividing bytes by 255.
    pub mean: [f32; 3],
    /// RGB standard deviation.
    pub std: [f32; 3],
    /// Output embedding length, 1..4096.
    pub dimensions: usize,
    /// Optional supplied calibration; absent means raw cosine only.
    pub calibration: Option<Calibration>,
    /// Optional joint tower; absent historical contracts retain their serialized identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextModel>,
}
/// Validates export pins, preprocessing and calibration without models or network.
pub fn validate(model: &Model) -> Result<()> {
    if model.schema != MODEL_SCHEMA
        || !matches!(model.family.as_str(), "dinov2-small" | "siglip2-base")
        || model.artifact.license != "Apache-2.0"
        || model.artifact.format != "onnx"
        || model.input.is_empty()
        || model.input.len() > 256
        || model.output.is_empty()
        || model.output.len() > 256
        || model.size.iter().any(|&v| v == 0 || v > 1024)
        || model.mean.iter().any(|v| !v.is_finite())
        || model.std.iter().any(|v| !v.is_finite() || *v <= 0.)
        || model.dimensions == 0
        || model.dimensions > 4096
    {
        return Err(Error::Config("invalid embedding export interface".into()));
    }
    if model.family == "siglip2-base" {
        let text = model
            .text
            .as_ref()
            .ok_or_else(|| Error::Config("joint text tower required".into()))?;
        if model.dimensions != 768
            || model.size != [224, 224]
            || model.mean != [0.5; 3]
            || model.std != [0.5; 3]
            || text.length != 64
            || text.pad_id != 0
            || text.input != "input_ids"
            || text.output != "embedding"
            || text.artifact.format != "onnx"
            || text.tokenizer.format != "tokenizer"
            || text.artifact.license != "Apache-2.0"
            || text.tokenizer.license != "Apache-2.0"
            || model.calibration.is_some()
        {
            return Err(Error::Config("invalid SigLIP 2 joint interface".into()));
        }
    } else if model.text.is_some() {
        return Err(Error::Config(
            "image-only family cannot supply text tower".into(),
        ));
    }
    semantic::validate(&cache_manifest(model))?;
    if let Some(c) = &model.calibration {
        let hash = &c.corpus_sha256;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || c.scope.is_empty()
            || c.scope.len() > 4096
            || c.bands.is_empty()
            || c.bands.len() > 16
            || c.bands.iter().any(|b| {
                !b.minimum.is_finite()
                    || !(-1.0..=1.0).contains(&b.minimum)
                    || b.label.is_empty()
                    || b.label.len() > 128
            })
            || c.bands.windows(2).any(|w| w[0].minimum >= w[1].minimum)
        {
            return Err(Error::Config(
                "invalid supplied embedding calibration".into(),
            ));
        }
    }
    Ok(())
}
/// Download and verify every pinned artifact of the supplied embedding contract into `cache`.
/// This is the explicit provisioning step behind `saccade models pull embedding`.
#[cfg(feature = "embeddings")]
pub fn provision(model: &Model, cache: &Path) -> Result<Vec<std::path::PathBuf>> {
    validate(model)?;
    semantic::cache_models(&cache_manifest(model), cache)
}
fn cache_manifest(model: &Model) -> semantic::ModelManifest {
    semantic::ModelManifest {
        schema: "saccade-region-models.v1".into(),
        qualification: "supplied_export_unqualified".into(),
        runtime: "ONNX Runtime 1.22".into(),
        preprocessing: "explicit embedding model contract".into(),
        execution_provider: "CPU f32".into(),
        artifacts: {
            let mut artifacts = vec![model.artifact.clone()];
            if let Some(text) = &model.text {
                artifacts.extend([text.artifact.clone(), text.tokenizer.clone()]);
            }
            artifacts
        },
        residuals: vec![
            "checkpoint/export parity and calibration qualification require external receipts"
                .into(),
        ],
    }
}
/// L2-normalizes a finite nonzero vector, accumulating in f64.
pub fn normalize(vector: &mut [f32]) -> Result<()> {
    let norm = vector
        .iter()
        .map(|&v| f64::from(v).powi(2))
        .sum::<f64>()
        .sqrt();
    if vector.is_empty() || !norm.is_finite() || norm <= 1e-12 {
        return Err(Error::Config("nonfinite or zero embedding".into()));
    }
    for v in vector {
        *v = (f64::from(*v) / norm) as f32;
    }
    Ok(())
}
/// Cosine between finite nonzero, equally sized vectors; no semantic verdict is inferred.
pub fn cosine(a: &[f32], b: &[f32]) -> Result<f64> {
    if a.len() != b.len() || a.is_empty() || a.len() > 4096 {
        return Err(Error::Config("embedding dimension mismatch".into()));
    }
    let aa = a.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>();
    let bb = b.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>();
    let dot = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| f64::from(x) * f64::from(y))
        .sum::<f64>();
    let denominator = (aa * bb).sqrt();
    if !denominator.is_finite() || denominator <= 1e-12 || !dot.is_finite() {
        return Err(Error::Config("invalid embedding".into()));
    }
    Ok((dot / denominator).clamp(-1., 1.))
}
/// Returns the supplied band's label, or None if uncalibrated/outside its declared range.
pub fn band(model: &Model, value: f64) -> Option<&str> {
    model
        .calibration
        .as_ref()?
        .bands
        .iter()
        .rev()
        .find(|b| value >= b.minimum)
        .map(|b| b.label.as_str())
}
/// Produces the exact NCHW input used by inference, for independent checkpoint/export parity.
pub fn preprocess(model: &Model, image: &image::RgbaImage) -> Result<Vec<f32>> {
    validate(model)?;
    let [w, h] = model.size;
    let rgb = crate::compare::flatten_over(image, 255);
    let resized = image::imageops::resize(&rgb, w, h, image::imageops::FilterType::Triangle);
    let n = w as usize * h as usize;
    let mut data = vec![0.; 3 * n];
    for (i, p) in resized.pixels().enumerate() {
        for c in 0..3 {
            data[c * n + i] = (f32::from(p[c]) / 255. - model.mean[c]) / model.std[c];
        }
    }
    Ok(data)
}
/// CPU-only runtime boundary; downloads occur only when explicitly requested.
#[cfg(feature = "embeddings")]
pub struct Engine {
    session: ort::session::Session,
    model: Model,
    cache: std::path::PathBuf,
    text_session: Option<(ort::session::Session, tokenizers::Tokenizer)>,
}
#[cfg(feature = "embeddings")]
impl Engine {
    /// Loads a supplied hash-pinned export from a content-addressed cache.
    /// The dynamic runtime file must already exist. Ordinary loading does not download.
    pub fn load(model: Model, cache: &Path, library: &Path, download: bool) -> Result<Self> {
        validate(&model)?;
        crate::optional::require_library(library)?;
        if download {
            semantic::cache_models(&cache_manifest(&model), cache)?;
        }
        let path = semantic::artifact_path(cache, &model.artifact)?;
        let file = std::fs::File::open(&path)
            .map_err(crate::run::io_err("opening pinned embedding export; fix: saccade models pull MODEL_ID --registry REGISTRY --cache CACHE (or this command --download-model for the supplied pinned contract)".into()))?;
        use sha2::{Digest, Sha256};
        use std::io::Read;
        if file
            .metadata()
            .map_err(crate::run::io_err("model metadata".into()))?
            .len()
            != model.artifact.bytes
        {
            return Err(Error::Config("embedding model size mismatch".into()));
        }
        let mut reader = file.take(model.artifact.bytes + 1);
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65536];
        let mut count = 0;
        loop {
            let n = reader
                .read(&mut buffer)
                .map_err(crate::run::io_err("hashing pinned model".into()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            count += n as u64;
        }
        if count != model.artifact.bytes
            || format!("{:x}", hasher.finalize()) != model.artifact.sha256
        {
            return Err(Error::Config("embedding model hash mismatch".into()));
        }
        // Buffer loading has no model-file base directory for external initializer data.
        // Rebind the exact bytes passed to the runtime after reading the cache.
        let model_bytes = super::input::bytes(&path, model.artifact.bytes)?;
        if crate::localized::digest(&model_bytes) != model.artifact.sha256 {
            return Err(Error::Config(
                "embedding model changed after verification".into(),
            ));
        }
        std::panic::catch_unwind(|| -> Result<Self> {
            ort::init_from(library.display().to_string())
                .commit()
                .map_err(|e| Error::Config(e.to_string()))?;
            let session = ort::session::Session::builder()
                .map_err(|e| Error::Config(e.to_string()))?
                .with_intra_threads(1)
                .map_err(|e| Error::Config(e.to_string()))?
                .with_inter_threads(1)
                .map_err(|e| Error::Config(e.to_string()))?
                .commit_from_memory(&model_bytes)
                .map_err(|e| Error::Config(e.to_string()))?;
            if session.inputs.len() != 1
                || session.inputs[0].name != model.input
                || !session.outputs.iter().any(|o| o.name == model.output)
            {
                return Err(Error::Config(
                    "ONNX export tensor names do not match model contract".into(),
                ));
            }
            Ok(Self {
                session,
                model,
                cache: cache.into(),
                text_session: None,
            })
        })
        .map_err(|_| Error::Config("ONNX runtime dynamic ABI/load failure".into()))?
    }
    /// Runs a pinned SigLIP 2 text tower. Image-only contracts fail explicitly.
    pub fn embed_text(&mut self, text: &str) -> Result<Vec<f32>> {
        if text.trim().is_empty() || text.len() > 16384 {
            return Err(Error::Config(
                "text query must contain 1..16384 UTF-8 bytes".into(),
            ));
        }
        let contract =
            self.model.text.as_ref().ok_or_else(|| {
                Error::Config("text_embedding_unavailable: image-only model".into())
            })?;
        if self.text_session.is_none() {
            let retained = |artifact: &semantic::ModelArtifact| -> Result<Vec<u8>> {
                let path = semantic::artifact_path(&self.cache, artifact)?;
                let bytes = super::input::bytes(&path, artifact.bytes)?;
                if bytes.len() as u64 != artifact.bytes
                    || crate::localized::digest(&bytes) != artifact.sha256
                {
                    return Err(Error::Config("text artifact size/hash mismatch".into()));
                }
                Ok(bytes)
            };
            let graph = retained(&contract.artifact)?;
            let tokenizer_bytes = retained(&contract.tokenizer)?;
            let mut tokenizer = tokenizers::Tokenizer::from_bytes(&tokenizer_bytes)
                .map_err(|_| Error::Config("invalid pinned tokenizer JSON".into()))?;
            tokenizer.with_padding(Some(tokenizers::PaddingParams {
                strategy: tokenizers::PaddingStrategy::Fixed(contract.length),
                pad_id: contract.pad_id,
                pad_token: "<pad>".into(),
                ..Default::default()
            }));
            tokenizer
                .with_truncation(Some(tokenizers::TruncationParams {
                    max_length: contract.length,
                    ..Default::default()
                }))
                .map_err(|_| Error::Config("invalid tokenizer truncation".into()))?;
            let session = ort::session::Session::builder()
                .map_err(|e| Error::Config(e.to_string()))?
                .with_intra_threads(1)
                .map_err(|e| Error::Config(e.to_string()))?
                .with_inter_threads(1)
                .map_err(|e| Error::Config(e.to_string()))?
                .commit_from_memory(&graph)
                .map_err(|e| Error::Config(e.to_string()))?;
            if session.inputs.len() != 1
                || session.inputs[0].name != contract.input
                || !session.outputs.iter().any(|o| o.name == contract.output)
            {
                return Err(Error::Config(
                    "text export tensor interface mismatch".into(),
                ));
            }
            self.text_session = Some((session, tokenizer));
        }
        let (session, tokenizer) = self
            .text_session
            .as_mut()
            .ok_or_else(|| Error::Config("text session unavailable".into()))?;
        let encoding = tokenizer
            .encode(text, true)
            .map_err(|_| Error::Config("text tokenization failed".into()))?;
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| i64::from(id)).collect();
        if ids.len() != contract.length {
            return Err(Error::Config("text token length mismatch".into()));
        }
        let tensor = ort::value::Tensor::from_array(([1, contract.length], ids.into_boxed_slice()))
            .map_err(|e| Error::Config(e.to_string()))?;
        let outputs = session
            .run(ort::inputs![contract.input.as_str()=>tensor])
            .map_err(|e| Error::Config(e.to_string()))?;
        let output = outputs
            .get(&contract.output)
            .ok_or_else(|| Error::Config("text output missing".into()))?;
        let (shape, values) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::Config(e.to_string()))?;
        if shape.as_ref() != [1, self.model.dimensions as i64]
            || values.len() != self.model.dimensions
        {
            return Err(Error::Config("text output dimensions mismatch".into()));
        }
        let mut vector = values.to_vec();
        normalize(&mut vector)?;
        Ok(vector)
    }
    /// Declared model identity.
    pub fn model(&self) -> &Model {
        &self.model
    }
    /// Runs the explicit export preprocessing and returns one normalized embedding.
    pub fn embed(&mut self, image: &image::RgbaImage) -> Result<Vec<f32>> {
        let [w, h] = self.model.size;
        let data = preprocess(&self.model, image)?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Vec<f32>> {
            let tensor = ort::value::Tensor::from_array((
                [1usize, 3, h as usize, w as usize],
                data.into_boxed_slice(),
            ))
            .map_err(|e| Error::Config(e.to_string()))?;
            let outputs = self
                .session
                .run(ort::inputs![self.model.input.as_str()=>tensor])
                .map_err(|e| Error::Config(e.to_string()))?;
            let output = outputs
                .get(&self.model.output)
                .ok_or_else(|| Error::Config("pooled output tensor unavailable".into()))?;
            let (shape, values) = output
                .try_extract_tensor::<f32>()
                .map_err(|e| Error::Config(e.to_string()))?;
            if shape.as_ref() != [1, self.model.dimensions as i64]
                || values.len() != self.model.dimensions
            {
                return Err(Error::Config(
                    "embedding output must be pooled [1,dimensions]".into(),
                ));
            }
            let mut vector = values.to_vec();
            normalize(&mut vector)?;
            Ok(vector)
        }))
        .map_err(|_| Error::Config("ONNX runtime inference failure".into()))?
    }
}
/// Read a historical model contract or its projection from the shared registry.
pub fn parse_model(bytes: &[u8]) -> Result<Model> {
    let value = crate::wave7::models::contract(bytes, MODEL_SCHEMA)
        .map_err(|e| Error::Config(e.to_string()))?;
    let model: Model = serde_json::from_value(value)?;
    validate(&model)?;
    Ok(model)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn cosine_refuses_invalid_vectors_and_preserves_geometry() {
        assert_eq!(cosine(&[1., 0.], &[1., 0.]).expect("cosine"), 1.);
        assert_eq!(cosine(&[1., 0.], &[0., 1.]).expect("cosine"), 0.);
        assert_eq!(cosine(&[1., 0.], &[-1., 0.]).expect("cosine"), -1.);
        assert!(cosine(&[0., 0.], &[1., 0.]).is_err());
        assert!(cosine(&[f32::NAN], &[1.]).is_err());
        assert!(cosine(&[1.], &[1., 0.]).is_err());
    }
    #[cfg(feature = "embeddings")]
    #[test]
    #[ignore = "heavy: embeddings"]
    fn supplied_pinned_model_runs_on_generated_images() {
        let path = std::env::var_os("SACCADE_W6_EMBEDDING_MODEL")
            .expect("coordinator supplies model contract");
        let model: Model = serde_json::from_slice(
            &super::super::input::bytes(Path::new(&path), 65536).expect("model contract"),
        )
        .expect("model");
        let cache = std::env::var_os("SACCADE_W6_MODEL_CACHE").expect("cache");
        let library = std::env::var_os("SACCADE_W6_ORT_LIBRARY").expect("runtime");
        let mut engine = Engine::load(model, Path::new(&cache), Path::new(&library), false)
            .expect("pinned model");
        let image = image::RgbaImage::from_fn(64, 64, |x, y| {
            image::Rgba([(x * 3) as u8, (y * 3) as u8, 100, 255])
        });
        let a = engine.embed(&image).expect("embed");
        let b = engine.embed(&image).expect("embed");
        assert!(cosine(&a, &b).expect("cosine") > 0.99999);
    }
}
