//! Explicit dynamically loaded CPU ONNX graph adapter; graph success is not parity.
use super::{
    models::{Model, Result, VisionError, ensure},
    vision::{Provenance, VisionImage},
};
use ort::{session::Session, value::Tensor};
use std::path::{Path, PathBuf};
/// Bounded float output, with shape retained for decoder validation.
#[derive(Debug)]
pub struct FloatOutput {
    /// Exact tensor dimensions.
    pub shape: Vec<i64>,
    /// Row-major values.
    pub values: Vec<f32>,
}
/// One pinned graph; all companion artifacts verified before loading.
pub struct OnnxModel {
    session: Session,
    /// Exact pinned model/input/export contract.
    pub model: Model,
}
pub(crate) fn ort_error(_: ort::Error) -> VisionError {
    VisionError::Unavailable("ONNX runtime operation failed".into())
}
impl OnnxModel {
    /// Verify graph bytes and explicitly load ONNX Runtime 1.22, with one CPU thread.
    pub fn load(model: &Model, cache: &Path, library: &Path, allow_download: bool) -> Result<Self> {
        Self::load_role(model, cache, library, allow_download, "graph")
    }
    /// Load one verified graph from a split encoder/decoder bundle.
    pub fn load_role(
        model: &Model,
        cache: &Path,
        library: &Path,
        allow_download: bool,
        role: &str,
    ) -> Result<Self> {
        super::models::Registry {
            schema: super::models::REGISTRY_SCHEMA.into(),
            models: vec![model.clone()],
            contracts: Default::default(),
        }
        .validate()?;
        if model.runtime != "onnx" || !library.is_file() {
            return Err(VisionError::Unavailable(format!(
                "explicit ONNX graph/runtime library required; fix: saccade models pull runtime; set ORT_DYLIB_PATH or --runtime-library; ONNX Runtime {}",
                super::runtime_install::REQUIRED_VERSION
            )));
        }
        ensure(model, cache, allow_download)?;
        let graph=model.artifacts.iter().find(|a|a.role==role).ok_or_else(||VisionError::Unavailable("self-contained graph role required (external-data exports need a qualified adapter)".into()))?;
        let graph = super::models::verify(cache, graph)?;
        let library = std::fs::canonicalize(library)?;
        static LIBRARY: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        let configured = LIBRARY.get_or_init(|| library.clone());
        if configured != &library {
            return Err(VisionError::Invalid(
                "runtime library cannot change inside one process".into(),
            ));
        }
        std::panic::catch_unwind(|| {
            ort::init_from(library.display().to_string())
                .commit()
                .map_err(ort_error)?;
            let session = Session::builder()
                .map_err(ort_error)?
                .with_intra_threads(1)
                .map_err(ort_error)?
                .commit_from_file(graph)
                .map_err(ort_error)?;
            Ok(Self {
                session,
                model: model.clone(),
            })
        })
        .map_err(runtime_panic)?
    }
    /// Run explicitly named tensors and retrieve only needed outputs (no OWLv2 feature map).
    pub(crate) fn run_named(
        &mut self,
        inputs: Vec<(
            std::borrow::Cow<'_, str>,
            ort::session::SessionInputValue<'_>,
        )>,
        names: &[&str],
        max_values: usize,
    ) -> Result<std::collections::BTreeMap<String, FloatOutput>> {
        use ort::session::run_options::{OutputSelector, RunOptions};
        let mut selector = OutputSelector::no_default();
        for name in names {
            selector = selector.with(*name);
        }
        let options = RunOptions::new().map_err(ort_error)?.with_outputs(selector);
        let outputs = self
            .session
            .run_with_options(inputs, &options)
            .map_err(ort_error)?;
        let mut result = std::collections::BTreeMap::new();
        let mut total = 0usize;
        for (name, value) in outputs.iter() {
            let (shape, data) = value.try_extract_tensor::<f32>().map_err(ort_error)?;
            total = total.saturating_add(data.len());
            // DINO intentionally fills unused text positions with -infinity logits.
            // NaN/+infinity and non-logit infinities remain invalid.
            let padded_logits = self.model.input.adapter == "grounding-dino-v1" && name == "logits";
            if total > max_values || data.iter().any(|v| !valid_float(*v, padded_logits)) {
                return Err(VisionError::Invalid("graph output bounds/nonfinite".into()));
            }
            result.insert(
                name.to_owned(),
                FloatOutput {
                    shape: shape.to_vec(),
                    values: data.to_vec(),
                },
            );
        }
        Ok(result)
    }
    /// NCHW processor defined by the pinned contract; no undocumented model defaults.
    pub fn tensor(&self, image: &VisionImage) -> Result<Tensor<f32>> {
        let i = &self.model.input;
        let [w, h] = if i.adapter == "yunet-v1" {
            image.size().map(|v| v.div_ceil(32) * 32)
        } else if i.resolution == [0, 0] {
            image.size()
        } else {
            i.resolution
        };
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > super::vision::MAX_PIXELS {
            return Err(VisionError::Invalid("runtime input dimensions".into()));
        }
        let pixels = if i.adapter == "yunet-v1" {
            let mut padded = image::RgbImage::new(w, h);
            image::imageops::replace(&mut padded, &image.pixels, 0, 0);
            padded
        } else if image.size() == [w, h] {
            image.pixels.clone()
        } else {
            image::imageops::resize(&image.pixels, w, h, image::imageops::FilterType::Triangle)
        };
        let n = w as usize * h as usize;
        let mut data = vec![0.; n * 3];
        for (offset, p) in pixels.pixels().enumerate() {
            for c in 0..3 {
                let channel = if i.color == "BGR" { 2 - c } else { c };
                data[c * n + offset] = (f32::from(p[channel]) * i.scale - i.mean[c]) / i.std[c];
            }
        }
        Tensor::from_array(([1usize, 3, h as usize, w as usize], data)).map_err(ort_error)
    }
    /// Execute the declared one-/two-image graph and retain named float outputs.
    pub fn run(
        &mut self,
        image: &VisionImage,
        reference: Option<&VisionImage>,
    ) -> Result<std::collections::BTreeMap<String, FloatOutput>> {
        let mut inputs = ort::inputs![self.model.input.image_input.clone()=>self.tensor(image)?];
        if let Some(r) = reference {
            let name =
                self.model.input.reference_input.as_ref().ok_or_else(|| {
                    VisionError::Invalid("pair graph reference input missing".into())
                })?;
            inputs.push((name.clone().into(), self.tensor(r)?.into()));
        }
        let outputs = self.session.run(inputs).map_err(ort_error)?;
        let mut result = std::collections::BTreeMap::new();
        let mut total = 0usize;
        for (name, value) in outputs.iter() {
            let (shape, data) = value.try_extract_tensor::<f32>().map_err(ort_error)?;
            total = total.saturating_add(data.len());
            if total > 1_048_576 || data.iter().any(|v| !v.is_finite()) {
                return Err(VisionError::Invalid("graph output bounds/nonfinite".into()));
            }
            result.insert(
                name.to_owned(),
                FloatOutput {
                    shape: shape.to_vec(),
                    values: data.to_vec(),
                },
            );
        }
        Ok(result)
    }
    /// Full consumed artifact identity and declared original-resolution handling.
    pub fn provenance(&self, image: &VisionImage) -> Provenance {
        let size = if self.model.input.adapter == "yunet-v1" {
            image.size().map(|v| v.div_ceil(32) * 32)
        } else if self.model.input.resolution == [0, 0] {
            image.size()
        } else {
            self.model.input.resolution
        };
        Provenance {
            model_id: self.model.id.clone(),
            version: self.model.version.clone(),
            artifact_sha256: self
                .model
                .artifacts
                .iter()
                .map(|a| a.sha256.clone())
                .collect(),
            runtime: "onnx-cpu".into(),
            input_resolution: size,
            resolution_handling: format!(
                "{}; {}; input {}x{}, graph {}x{}",
                self.model.input.preprocessing,
                self.model.input.adapter,
                image.size()[0],
                image.size()[1],
                size[0],
                size[1]
            ),
            source_parity: self.model.parity_sha256.is_some(),
        }
    }
}

fn valid_float(value: f32, dino_padding: bool) -> bool {
    value.is_finite() || (dino_padding && value == f32::NEG_INFINITY)
}
fn runtime_panic(payload: Box<dyn std::any::Any + Send>) -> VisionError {
    let detail = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("dynamic ONNX ABI/library failure");
    VisionError::RuntimeIncompatible {
        required: super::runtime_install::REQUIRED_VERSION.into(),
        detail: detail.chars().take(2048).collect(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn accepts_observed_dino_padding_without_accepting_nonfinite_geometry() {
        assert!(super::valid_float(f32::NEG_INFINITY, true));
        assert!(!super::valid_float(f32::NEG_INFINITY, false));
        for value in [f32::NAN, f32::INFINITY] {
            assert!(!super::valid_float(value, true));
        }
    }
}
