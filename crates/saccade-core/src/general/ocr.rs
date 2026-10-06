//! SHA-pinned PP-OCRv5 detector and Latin CTC recognizer on CPU ONNX Runtime.
use crate::{Error, Result, semantic};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(feature = "ocr")]
use {crate::ui_review, std::path::Path};
/// PP-OCRv5 model contract. Historical OCR outputs remain readable.
pub const SCHEMA: &str = "saccade-paddle-ocr.v1";
/// Fixed processor revision, bound to the supplied official inference configurations.
pub const PROCESSOR: &str = "ppocr-v5-latin/1";
/// Explicit model and dictionary pins; no implicit network access.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// SCHEMA.
    pub schema: String,
    /// Content-addressed cache, relative to the contract file.
    pub cache: PathBuf,
    /// Self-contained ONNX detection graph.
    pub detection: semantic::ModelArtifact,
    /// Self-contained ONNX Latin recognition graph.
    pub recognition: semantic::ModelArtifact,
    /// Official recognition inference.yml, containing the ordered character dictionary.
    pub dictionary: semantic::ModelArtifact,
    /// Immutable licence/source receipts, never a quality claim.
    pub license_evidence: String,
    /// Review status, independent of inference acceptance.
    #[serde(default)]
    pub review_status: Option<String>,
}
fn manifest(c: &Contract) -> semantic::ModelManifest {
    semantic::ModelManifest {
        schema: "saccade-region-models.v1".into(),
        qualification: "generated_contracts_pending_review".into(),
        runtime: "ONNX Runtime 1.22".into(),
        preprocessing: PROCESSOR.into(),
        execution_provider: "CPU f32".into(),
        artifacts: vec![
            c.detection.clone(),
            c.recognition.clone(),
            c.dictionary.clone(),
        ],
        residuals: vec![
            "Source/export parity and production accuracy are not established by fixture gates"
                .into(),
        ],
    }
}
/// Validate roles, formats, immutable revisions and licences without IO.
pub fn validate(c: &Contract) -> Result<()> {
    if c.schema != SCHEMA
        || c.cache.as_os_str().is_empty()
        || c.detection.role != "detection"
        || c.recognition.role != "recognition"
        || c.dictionary.role != "dictionary"
        || c.detection.format != "onnx"
        || c.recognition.format != "onnx"
        || c.dictionary.format != "checkpoint"
        || c.license_evidence.is_empty()
        || c.license_evidence.len() > 4096
        || [&c.detection, &c.recognition, &c.dictionary]
            .iter()
            .any(|a| {
                a.version.len() != 40
                    || !a.version.bytes().all(|v| v.is_ascii_hexdigit())
                    || !a.url.contains(&a.version)
            })
    {
        return Err(Error::Config(
            "invalid PP-OCRv5 graph/dictionary/licence contract".into(),
        ));
    }
    semantic::validate(&manifest(c))
}
/// Built-in default pins; cache location remains caller-owned.
pub fn default_contract() -> Result<Contract> {
    let c: Contract = serde_json::from_str(include_str!("../../assets/paddle-ocr.json"))
        .map_err(|e| Error::Config(e.to_string()))?;
    validate(&c)?;
    Ok(c)
}
#[cfg(feature = "ocr")]
fn ort_error(_: ort::Error) -> Error {
    Error::Config("PP-OCRv5 ONNX runtime operation failed".into())
}
/// Check optional runtime/artifact availability before input work. Explicit downloads
/// are performed by loading/inference, never by this discovery-only preflight.
#[cfg(feature = "ocr")]
pub fn preflight(c: &Contract, cache: &Path, download: bool) -> Result<()> {
    validate(c)?;
    let library = crate::wave7::runtime_install::resolve(None, cache)
        .map_err(|e| Error::Config(e.to_string()))?;
    crate::optional::require_library(&library)?;
    if !download {
        for a in [&c.detection, &c.recognition, &c.dictionary] {
            if !semantic::artifact_path(cache, a)?.is_file() {
                return Err(Error::Config("optional OCR artifact missing; fix: saccade text A B --download-model (add --ocr-contract FILE for a custom contract)".into()));
            }
        }
    }
    Ok(())
}
/// Reusable detector and recognizer sessions, one CPU thread each.
#[cfg(feature = "ocr")]
pub struct Engine {
    detection: ort::session::Session,
    recognition: ort::session::Session,
    dictionary: Vec<String>,
    contract: Contract,
}
/// Runs verified PP-OCRv5 sessions. Downloads require an explicit opt-in.
#[cfg(feature = "ocr")]
pub fn recognize(
    c: &Contract,
    cache: &Path,
    encoded: &[u8],
    download: bool,
) -> Result<ui_review::Source> {
    Engine::load(c, cache, download)?.recognize(encoded)
}
#[cfg(feature = "ocr")]
impl Engine {
    /// Verify every pin before loading any graph; resolve the existing runtime cache or ORT_DYLIB_PATH.
    pub fn load(c: &Contract, cache: &Path, download: bool) -> Result<Self> {
        validate(c)?;
        let library = crate::wave7::runtime_install::resolve(None, cache)
            .map_err(|e| Error::Config(e.to_string()))?;
        crate::optional::require_library(&library)?;
        if download {
            semantic::cache_models(&manifest(c), cache)?;
        }
        let verify = |a: &semantic::ModelArtifact| -> Result<Vec<u8>> {
            let path = semantic::artifact_path(cache, a)?;
            if !path.is_file() {
                return Err(Error::Config("optional OCR artifact missing; fix: saccade text A B --download-model (add --ocr-contract FILE for a custom contract)".into()));
            }
            let b = super::input::bytes(&path, a.bytes)?;
            if b.len() as u64 != a.bytes || crate::localized::digest(&b) != a.sha256 {
                return Err(Error::Config("OCR model hash/size mismatch".into()));
            }
            Ok(b)
        };
        let det = verify(&c.detection)?;
        let rec = verify(&c.recognition)?;
        let dict = verify(&c.dictionary)?;
        let config: serde_yaml::Value = serde_yaml::from_slice(&dict)
            .map_err(|_| Error::Config("OCR dictionary configuration".into()))?;
        let chars = config["PostProcess"]["character_dict"]
            .as_sequence()
            .ok_or_else(|| Error::Config("OCR dictionary missing".into()))?;
        if chars.is_empty() || chars.len() > 4096 {
            return Err(Error::Config("OCR dictionary bound".into()));
        }
        let mut dictionary = vec![String::new()];
        for char in chars {
            let char = char
                .as_str()
                .ok_or_else(|| Error::Config("OCR dictionary entry".into()))?;
            if char.chars().count() != 1 {
                return Err(Error::Config("OCR dictionary scalar".into()));
            }
            dictionary.push(char.into());
        }
        dictionary.push(" ".into()); // official CTC blank=0 and use_space_char=true; duplicates retain their indices.

        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Self> {
            ort::init_from(library.display().to_string())
                .commit()
                .map_err(ort_error)?;
            let load = |b: &[u8]| -> Result<ort::session::Session> {
                let s = ort::session::Session::builder()
                    .map_err(ort_error)?
                    .with_intra_threads(1)
                    .map_err(ort_error)?
                    .with_inter_threads(1)
                    .map_err(ort_error)?
                    .commit_from_memory(b)
                    .map_err(ort_error)?;
                if s.inputs.len() != 1
                    || s.inputs[0].name != "x"
                    || s.outputs.len() != 1
                    || s.outputs[0].name != "fetch_name_0"
                {
                    return Err(Error::Config("PP-OCRv5 tensor names".into()));
                }
                Ok(s)
            };
            Ok(Self {
                detection: load(&det)?,
                recognition: load(&rec)?,
                dictionary,
                contract: c.clone(),
            })
        }))
        .map_err(|_| Error::Config("OCR dynamic runtime ABI/load failure".into()))?
    }
    /// Detect, rectify and decode exact Unicode; confidence is mean selected CTC score, not calibrated probability.
    pub fn recognize(&mut self, encoded: &[u8]) -> Result<ui_review::Source> {
        let image = super::input::decode(encoded)?;
        let rgb = crate::compare::flatten_over(&image, 255);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<ui_review::Source> {
            let ratio=960. / f64::from(rgb.width().max(rgb.height()));
            let w=((f64::from(rgb.width())*ratio) as u32).div_ceil(128)*128;
            let h=((f64::from(rgb.height())*ratio) as u32).div_ceil(128)*128;
            let resized=super::ocr_geometry::resize(&rgb,w.max(128),h.max(128))?;
            let (shape,map)=run(&mut self.detection,&resized,true,w.max(128))?;
            if shape!=[1,1,i64::from(resized.height()),i64::from(resized.width())] { return Err(Error::Config("DB output shape".into())); }
            let boxes=super::ocr_geometry::boxes(&map,resized.width(),resized.height(),rgb.dimensions())?;
            let mut nodes=Vec::new(); let mut quads=Vec::new();
            for quad in boxes {
                let crop=super::ocr_geometry::rectify(&rgb,quad)?;
                let rw=((48.*f64::from(crop.width())/f64::from(crop.height())).ceil() as u32).max(1);
                if rw>3200 { return Err(Error::Config("OCR recognition crop width limit".into())); }
                let crop=super::ocr_geometry::resize(&crop,rw,48)?;
                let (shape,values)=run(&mut self.recognition,&crop,false,rw.max(320))?;
                if shape.len()!=3 || shape[0]!=1 || shape[2]!=self.dictionary.len() as i64 { return Err(Error::Config("CTC output/dictionary shape mismatch".into())); }
                let (text,confidence)=decode_ctc(&values,self.dictionary.len(),&self.dictionary)?;
                if text.trim().is_empty() { continue; }
                let x=quad.iter().map(|p|p[0]).fold(f32::INFINITY,f32::min).max(0.);
                let y=quad.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min).max(0.);
                let right=quad.iter().map(|p|p[0]).fold(0.,f32::max).min(rgb.width() as f32);
                let bottom=quad.iter().map(|p|p[1]).fold(0.,f32::max).min(rgb.height() as f32);
                nodes.push(ui_review::Node { id:format!("paddle-line:{}",nodes.len()),text,bounds:Some([f64::from(x),f64::from(y),f64::from(right-x),f64::from(bottom-y)]),role:String::new(),reading_order:None,keyboard_order:None,disclosure:false,ocr_confidence:Some(confidence*100.) });
                quads.push(quad);
            }
            let source=ui_review::Source {schema:"saccade-ui-source.v1".into(),capture_sha256:crate::localized::digest(encoded),dimensions:[image.width(),image.height()],kind:"paddle_ocr".into(),complete:false,nodes,producer:serde_json::json!({"engine":"PP-OCRv5-mobile/Latin","runtime":"ONNX Runtime 1.22 CPU f32","processor":PROCESSOR,"contract":self.contract,"quadrilaterals":quads,"units":"detected text lines; word boxes unavailable","confidence":"mean retained CTC scores, uncalibrated","orientation":"no optional 180-degree classifier in pinned set"})};
            source.validate(&source.capture_sha256,source.dimensions)?; Ok(source)
        })).map_err(|_|Error::Config("OCR model/inference failed".into()))?
    }
}
#[cfg(feature = "ocr")]
fn run(
    session: &mut ort::session::Session,
    rgb: &image::RgbImage,
    detection: bool,
    width: u32,
) -> Result<(Vec<i64>, Vec<f32>)> {
    let h = rgb.height() as usize;
    let w = width as usize;
    let mut data = vec![0.; 3 * h * w];
    for (x, y, p) in rgb.enumerate_pixels() {
        for c in 0..3 {
            // Both official processors decode BGR.
            let v = f32::from(p[2 - c]) / 255.;
            data[c * h * w + y as usize * w + x as usize] = if detection {
                (v - [0.485, 0.456, 0.406][c]) / [0.229, 0.224, 0.225][c]
            } else {
                (v - 0.5) / 0.5
            };
        }
    }
    let tensor = ort::value::Tensor::from_array(([1, 3, h, w], data)).map_err(ort_error)?;
    let outputs = session.run(ort::inputs!["x"=>tensor]).map_err(ort_error)?;
    let (shape, values) = outputs["fetch_name_0"]
        .try_extract_tensor::<f32>()
        .map_err(ort_error)?;
    if values.len() > 8 * 1024 * 1024
        || values.iter().any(|v| {
            !v.is_finite() || !(-4.0 * f32::EPSILON..=1.0 + 4.0 * f32::EPSILON).contains(v)
        })
    {
        return Err(Error::Config(format!(
            "OCR output bound/probability range: len={}, min={}, max={}",
            values.len(),
            values.iter().copied().fold(f32::INFINITY, f32::min),
            values.iter().copied().fold(f32::NEG_INFINITY, f32::max)
        )));
    }
    Ok((shape.to_vec(), values.to_vec()))
}
/// CTC greedy decode: blank-separated duplicates retained; exact dictionary index order.
#[cfg(any(test, feature = "ocr"))]
fn decode_ctc(values: &[f32], classes: usize, dictionary: &[String]) -> Result<(String, f64)> {
    if classes != dictionary.len()
        || classes < 2
        || values.is_empty()
        || !values.len().is_multiple_of(classes)
        || values.iter().any(|v| {
            !v.is_finite() || !(-4.0 * f32::EPSILON..=1.0 + 4.0 * f32::EPSILON).contains(v)
        })
    {
        return Err(Error::Config("invalid CTC tensor".into()));
    }
    let mut text = String::new();
    let mut previous = usize::MAX;
    let mut sum = 0.;
    let mut count = 0;
    for row in values.chunks_exact(classes) {
        let (index, score) = row
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(&a.0)))
            .ok_or_else(|| Error::Config("empty CTC row".into()))?;
        if index != 0 && index != previous {
            text.push_str(&dictionary[index]);
            sum += f64::from(score.clamp(0., 1.));
            count += 1;
        }
        previous = index;
    }
    Ok((text, if count == 0 { 0. } else { sum / count as f64 }))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn default_pins_validate_and_ctc_preserves_accents_and_blank_separated_duplicates() {
        validate(&default_contract().unwrap()).unwrap();
        let dict = vec!["".into(), "é".into(), "ß".into()];
        let v = [0., 1., 0., 0., 1., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
        assert_eq!(decode_ctc(&v, 3, &dict).unwrap(), ("ééß".into(), 1.));
        assert!(decode_ctc(&[f32::NAN; 3], 3, &dict).is_err());
        assert_eq!(
            decode_ctc(&[0., 1. + f32::EPSILON, 0.], 3, &dict).unwrap(),
            ("é".into(), 1.)
        );
        assert!(decode_ctc(&[0., 1.01, 0.], 3, &dict).is_err());
        let mut c = default_contract().unwrap();
        c.recognition.url = c.recognition.url.replace(&c.recognition.version, "main");
        assert!(validate(&c).is_err());
    }
    #[cfg(feature = "ocr")]
    #[test]
    fn pin_failure_precedes_runtime_loading() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = default_contract().unwrap();
        c.detection.bytes = 1;
        std::fs::write(dir.path().join(&c.detection.sha256), [0]).unwrap();
        assert!(
            Engine::load(&c, dir.path(), false)
                .err()
                .unwrap()
                .to_string()
                .contains("hash/size mismatch")
        );
    }
}
