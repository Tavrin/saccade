//! Explicit wave 7 observations attached to wave 6 image evidence.
use crate::agent::CliError;
use saccade_core::wave7::{faces, models, vision::VisionImage, watermark};
use serde_json::{Value, json};
use std::path::PathBuf;
#[derive(clap::Args, Default)]
pub(crate) struct Checks {
    /// Run local face detection; never downloads models implicitly.
    #[arg(long)]
    faces: bool,
    /// Image-bound face receipt; explicitly labelled replay.
    #[arg(long)]
    face_observations: Option<PathBuf>,
    /// Shared model registry (vision, embedding and OCR pins).
    #[arg(long)]
    model_registry: Option<PathBuf>,
    #[arg(long)]
    model_cache: Option<PathBuf>,
    #[arg(long)]
    runtime_library: Option<PathBuf>,
    /// Inspect named watermark decoders; unavailable decoders stay explicit.
    #[arg(long)]
    watermark: bool,
    /// Known legacy DWT message bytes in hex; arbitrary bits are not detection.
    #[arg(long, requires = "watermark")]
    watermark_payload: Option<String>,
    /// Declared face-protection crop in original pixels: X,Y,W,H.
    #[arg(long, requires = "faces", value_delimiter = ',', num_args = 1)]
    face_crop: Option<Vec<f32>>,
}
impl Checks {
    pub(crate) fn measure(&self, path: &std::path::Path) -> Result<Value, CliError> {
        let mut value = json!({});
        if !(self.faces || self.face_observations.is_some() || self.watermark) {
            return Ok(value);
        }
        let image = VisionImage::load(path).map_err(crate::wave7_cmd::error)?;
        if self.faces || self.face_observations.is_some() {
            let report = if let Some(p) = &self.face_observations {
                let mut r: faces::FaceReport = serde_json::from_slice(
                    &models::read_bounded(p, 1024 * 1024).map_err(crate::wave7_cmd::error)?,
                )?;
                r.validate(&image).map_err(crate::wave7_cmd::error)?;
                r.provenance.runtime = "replay".into();
                r.provenance.source_parity = false;
                r
            } else {
                self.detect(&image)?
            };
            if let Some(crop) = &self.face_crop {
                let c: [f32; 4] = crop
                    .clone()
                    .try_into()
                    .map_err(|_| CliError::usage("face crop needs X,Y,W,H"))?;
                value["crop_check"] = serde_json::to_value(
                    faces::crop_check(
                        &image,
                        &report,
                        &[faces::CropSpec::Rectangle {
                            rect: saccade_core::wave7::vision::Rect {
                                x: c[0],
                                y: c[1],
                                width: c[2],
                                height: c[3],
                            },
                        }],
                        None,
                    )
                    .map_err(crate::wave7_cmd::error)?,
                )?;
            }
            value["faces"] = serde_json::to_value(report)?;
        }
        if self.watermark {
            let legacy = self
                .watermark_payload
                .as_deref()
                .map(crate::wave7_cmd::unhex)
                .transpose()?
                .map(|expected_payload| watermark::DwtConfig {
                    expected_payload,
                    quantization_step: 36.,
                    minimum_agreement: 0.9,
                });
            value["watermark"] = serde_json::to_value(
                watermark::inspect(&image, None, legacy.as_ref())
                    .map_err(crate::wave7_cmd::error)?,
            )?;
        }
        Ok(value)
    }
    #[cfg(feature = "local-models")]
    fn detect(&self, image: &VisionImage) -> Result<faces::FaceReport, CliError> {
        let registry = crate::wave7_cmd::registry(self.model_registry.as_deref())?;
        let cache = crate::wave7_cmd::cache(self.model_cache.as_deref())?;
        let library =
            saccade_core::wave7::runtime_install::resolve(self.runtime_library.as_deref(), &cache)
                .map_err(crate::wave7_cmd::error)?;
        let mut runtime = saccade_core::wave7::runtime::OnnxModel::load(
            registry
                .model("yunet-2026may")
                .map_err(crate::wave7_cmd::error)?,
            &cache,
            &library,
            false,
        )
        .map_err(crate::wave7_cmd::error)?;
        faces::detect(image, &mut runtime).map_err(crate::wave7_cmd::error)
    }
    #[cfg(not(feature = "local-models"))]
    fn detect(&self, _image: &VisionImage) -> Result<faces::FaceReport, CliError> {
        Err(CliError::new(
            "feature_unavailable",
            "face inference requires local-models",
        ))
    }
}
