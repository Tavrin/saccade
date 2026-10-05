//! Hash-pinned RTen/ocrs OCR. No models or native libraries bundled.
#[cfg(feature = "ocr")]
use crate::ui_review;
use crate::{Error, Result, semantic};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ocr")]
use std::path::Path;
use std::path::PathBuf;
/// Rust OCR contract schema.
pub const SCHEMA: &str = "saccade-ocrs.v1";
/// Explicit runtime artifacts and alphabet; supplied licences remain operator declarations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// saccade-ocrs.v1.
    pub schema: String,
    /// Cache directory, relative to the contract file.
    pub cache: PathBuf,
    /// Detection RTen export, format checkpoint, role detection.
    pub detection: semantic::ModelArtifact,
    /// Recognition RTen export, format checkpoint, role recognition.
    pub recognition: semantic::ModelArtifact,
    /// CTC alphabet matching the export, excluding blank. Accents must be explicit.
    pub alphabet: String,
    /// Licence evidence source and review scope (not a qualification claim).
    pub license_evidence: String,
    /// Contract review status, never a qualification receipt.
    #[serde(default)]
    pub review_status: Option<String>,
}
fn manifest(c: &Contract) -> semantic::ModelManifest {
    semantic::ModelManifest { schema:"saccade-region-models.v1".into(), qualification:"supplied_ocr_exports_unqualified".into(), runtime:"ONNX Runtime 1.22".into(), preprocessing:"ocrs 0.10.4 with RTen 0.21.0, explicit CTC alphabet".into(), execution_provider:"CPU f32".into(), artifacts:vec![c.detection.clone(),c.recognition.clone()],residuals:vec!["cache transport manifest reuses semantic downloader; execution uses RTen, not ONNX Runtime".into()] }
}
/// Validates model pins and declared alphabet without inference/downloads.
pub fn validate(c: &Contract) -> Result<()> {
    if c.schema != SCHEMA
        || c.cache.as_os_str().is_empty()
        || c.detection.role != "detection"
        || c.recognition.role != "recognition"
        || c.detection.format != "checkpoint"
        || c.recognition.format != "checkpoint"
        || c.alphabet.is_empty()
        || c.alphabet.chars().count() > 1024
        || c.license_evidence.is_empty()
        || c.license_evidence.len() > 4096
        || c.alphabet
            .chars()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != c.alphabet.chars().count()
    {
        return Err(Error::Config(
            "invalid supplied OCR export/alphabet/licence contract".into(),
        ));
    }
    semantic::validate(&manifest(c))
}
/// Runs pinned models using the pure Rust engine. Download is explicit and hash verified.
/// The upstream API exposes boxes and characters but no recognition confidence; it stays absent.
#[cfg(feature = "ocr")]
pub fn recognize(
    c: &Contract,
    cache: &Path,
    encoded: &[u8],
    download: bool,
) -> Result<ui_review::Source> {
    Engine::load(c, cache, download)?.recognize(encoded)
}
/// Reusable pure-Rust OCR model sessions, loaded once by a media analyzer.
#[cfg(feature = "ocr")]
pub struct Engine {
    engine: ocrs::OcrEngine,
    contract: Contract,
}
#[cfg(feature = "ocr")]
impl Engine {
    /// Verify and load the supplied models exactly once, with explicit download opt-in.
    pub fn load(c: &Contract, cache: &Path, download: bool) -> Result<Self> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Self> {
            validate(c)?;
            if download {
                semantic::cache_models(&manifest(c), cache)?;
            }
            let load = |a: &semantic::ModelArtifact| -> Result<rten::Model> {
                let bytes = super::input::bytes(&semantic::artifact_path(cache, a)?, a.bytes)?;
                if bytes.len() as u64 != a.bytes || crate::localized::digest(&bytes) != a.sha256 {
                    return Err(Error::Config("OCR model hash/size mismatch".into()));
                }
                rten::Model::load(bytes).map_err(|e| Error::Config(format!("RTen model: {e}")))
            };
            let engine = ocrs::OcrEngine::new(ocrs::OcrEngineParams {
                detection_model: Some(load(&c.detection)?),
                recognition_model: Some(load(&c.recognition)?),
                alphabet: Some(c.alphabet.clone()),
                ..Default::default()
            })
            .map_err(|e| Error::Config(format!("ocrs: {e}")))?;
            Ok(Self {
                engine,
                contract: c.clone(),
            })
        }))
        .map_err(|_| Error::Config("Rust OCR model loading failed".into()))?
    }
    /// Recognize retained bytes using the already-loaded engine; confidence stays absent.
    pub fn recognize(&self, encoded: &[u8]) -> Result<ui_review::Source> {
        let image = super::input::decode(encoded)?;
        let rgb = crate::compare::flatten_over(&image, 255);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<ui_review::Source> {
        use ocrs::TextItem;
        let map_error = |e: String|Error::Config(format!("ocrs: {e}"));

        let source = ocrs::ImageSource::from_bytes(rgb.as_raw(),rgb.dimensions()).map_err(|e|map_error(e.to_string()))?;
        let prepared = self.engine.prepare_input(source).map_err(|e|map_error(e.to_string()))?;
        let words = self.engine.detect_words(&prepared).map_err(|e|map_error(e.to_string()))?;
        if words.len() > 2048 { return Err(Error::Config("OCR observation limit exceeded".into())); }
        let lines = self.engine.find_text_lines(&prepared,&words);
        let text = self.engine.recognize_text(&prepared,&lines).map_err(|e|map_error(e.to_string()))?;
        let mut nodes = Vec::new();
        for line in text.into_iter().flatten() { for word in line.words() {
            let r = word.bounding_rect();
            nodes.push(ui_review::Node {id:format!("ocrs-slot:{}",nodes.len()),text:word.to_string(),bounds:Some([f64::from(r.left()),f64::from(r.top()),f64::from(r.width()),f64::from(r.height())]),role:String::new(),reading_order:None,keyboard_order:None,disclosure:false,ocr_confidence:None});
            if nodes.len() > 2048 { return Err(Error::Config("OCR observation limit exceeded".into())); }
        } }
        let result = ui_review::Source { schema:"saccade-ui-source.v1".into(), capture_sha256:crate::localized::digest(encoded),dimensions:[image.width(),image.height()],kind:"ocrs".into(),complete:false,nodes,producer:serde_json::json!({"engine":"ocrs 0.10.4","runtime":"RTen 0.21.0","contract":self.contract,"confidence":"unavailable in upstream API; never synthesized","qualification":"supplied_models_unqualified"}) };
        result.validate(&result.capture_sha256,result.dimensions)?;
        Ok(result)
    })).map_err(|_|Error::Config("Rust OCR model/inference failed".into()))?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn artifact(role: &str) -> semantic::ModelArtifact {
        semantic::ModelArtifact {
            role: role.into(),
            version: "supplied-revision".into(),
            format: "checkpoint".into(),
            url: "https://example.org/pinned/model.rten".into(),
            bytes: 1,
            sha256: "a".repeat(64),
            license: "Apache-2.0".into(),
        }
    }
    #[test]
    fn contract_keeps_accents_and_refuses_ambiguous_alphabet_or_unlicensed_models() {
        let mut c = Contract {
            review_status: None,
            schema: SCHEMA.into(),
            cache: "cache".into(),
            detection: artifact("detection"),
            recognition: artifact("recognition"),
            alphabet: " caféÉ".into(),
            license_evidence: "supplied evidence".into(),
        };
        assert!(validate(&c).is_ok());
        c.alphabet.push('é');
        assert!(validate(&c).is_err());
        c.alphabet.pop();
        c.recognition.license = "unknown".into();
        assert!(validate(&c).is_err());
    }
    #[cfg(feature = "ocr")]
    #[test]
    fn model_pin_failure_never_falls_back_or_synthesizes_source_facts() {
        let cache = tempfile::tempdir().unwrap();
        let c = Contract {
            review_status: None,
            schema: SCHEMA.into(),
            cache: "cache".into(),
            detection: artifact("detection"),
            recognition: artifact("recognition"),
            alphabet: " CAFÉ".into(),
            license_evidence: "declared evidence".into(),
        };
        std::fs::write(cache.path().join(&c.detection.sha256), [0u8]).unwrap();
        let image = image::RgbImage::from_pixel(16, 16, image::Rgb([255; 3]));
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut encoded)
            .encode_image(&image)
            .unwrap();
        let error = recognize(&c, cache.path(), &encoded, false).unwrap_err();
        assert!(error.to_string().contains("hash/size mismatch"));
        let mut source = ui_review::Source {
            schema: "saccade-ui-source.v1".into(),
            capture_sha256: "b".repeat(64),
            dimensions: [16, 16],
            kind: "ocrs".into(),
            complete: true,
            nodes: Vec::new(),
            producer: serde_json::json!({}),
        };
        assert!(
            source
                .validate(&source.capture_sha256, source.dimensions)
                .is_err()
        );
        source.complete = false;
        assert!(
            source
                .validate(&source.capture_sha256, source.dimensions)
                .is_ok()
        );
    }
}
