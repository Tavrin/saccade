//! Standalone wave 7 commands; no ambient model downloads or provider calls.
use crate::agent::CliError;
use saccade_core::wave7::models::{self, Registry, VisionError};
use std::path::{Path, PathBuf};

pub(crate) fn error(e: VisionError) -> CliError {
    let code = match &e {
        VisionError::RuntimeIncompatible { .. } => "runtime_incompatible",
        VisionError::Unavailable(_) => "vision_unavailable",
        VisionError::Integrity(_) => "model_integrity",
        VisionError::Invalid(_) => "invalid_vision_input",
        VisionError::Io(_) => "io_error",
        VisionError::Json(_) => "invalid_json",
    };
    CliError::new(code, e.to_string())
}
fn home() -> Result<PathBuf, CliError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| CliError::usage("HOME unavailable; supply registry/cache paths"))
}
/// Explicit `--runtime-library` (deprecated) over the shared configuration.
#[cfg(feature = "local-models")]
pub(crate) fn runtime_flag(flag: Option<&Path>) -> Result<Option<PathBuf>, CliError> {
    Ok(config()?
        .with_overrides(saccade_core::model_config::Overrides {
            runtime_library: flag,
            ..Default::default()
        })
        .runtime_library)
}
/// Registry status plus runtime status: the one model-status report CLI and MCP both emit.
pub(crate) fn status(registry: &Registry, cache: &Path) -> serde_json::Value {
    let status = registry.status(cache);
    #[cfg(feature = "local-models")]
    let status = {
        let mut status = status;
        status["runtime"] = saccade_core::wave7::runtime_install::status(cache);
        status
    };
    status
}
/// The operator's model configuration (environment, config file), shared with every surface.
pub(crate) fn config() -> Result<saccade_core::model_config::ModelConfig, CliError> {
    Ok(saccade_core::model_config::ModelConfig::resolve()?)
}
pub(crate) fn registry(path: Option<&Path>) -> Result<Registry, CliError> {
    let cfg = config()?.with_overrides(saccade_core::model_config::Overrides {
        registry: path,
        ..Default::default()
    });
    match cfg.registry {
        Some(p) => Registry::load(&p).map_err(error),
        None => {
            // Compatibility reader: the registry file location used before the shared config.
            let legacy = home()?.join(".config").join("saccade").join("models.json");
            if legacy.exists() {
                Registry::load(&legacy).map_err(error)
            } else {
                Registry::pinned_wave7().map_err(error)
            }
        }
    }
}
pub(crate) fn cache(path: Option<&Path>) -> Result<PathBuf, CliError> {
    Ok(config()?
        .with_overrides(saccade_core::model_config::Overrides {
            dir: path,
            ..Default::default()
        })
        .dir)
}
/// Cache and optional registry for the media analyzer, from the shared configuration.
/// `None` registry means the analyzer's own pinned default.
pub(crate) fn analyzer_inputs(
    dir: Option<&Path>,
    registry_flag: Option<&Path>,
) -> Result<(PathBuf, Option<Registry>), CliError> {
    let dir = cache(dir)?;
    let cfg = config()?.with_overrides(saccade_core::model_config::Overrides {
        registry: registry_flag,
        ..Default::default()
    });
    let registry = cfg
        .registry
        .map(|p| Registry::load(&p).map_err(error))
        .transpose()?;
    Ok((dir, registry))
}
fn emit<T: serde::Serialize>(value: &T, json: bool) -> Result<u8, CliError> {
    let text = if json {
        serde_json::to_string(value)?
    } else {
        serde_json::to_string_pretty(value)?
    };
    crate::emit(&format!("{text}\n"))?;
    Ok(0)
}
#[derive(clap::Args)]
pub(crate) struct ModelsArgs {
    #[command(subcommand)]
    operation: ModelsOperation,
}
#[derive(clap::Subcommand)]
enum ModelsOperation {
    /// Inspect selections, real pins, cache integrity and source-parity status.
    List {
        /// Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Deprecated: set SACCADE_MODELS_DIR or [models].dir.
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Show the resolved model configuration and where each value came from.
    Config {
        #[arg(long)]
        json: bool,
    },
    /// The one provisioning verb: download and verify the named pinned artifacts.
    ///
    /// ID is a registry model, `runtime` (ONNX Runtime), `ocr` (the pinned OCR
    /// contract, or --contract FILE) or `embedding` (--contract FILE or the
    /// configured embedding contract). Nothing else downloads on request.
    Pull {
        id: String,
        /// Contract file for `ocr` / `embedding`.
        #[arg(long)]
        contract: Option<PathBuf>,
        /// Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry.
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Deprecated: set SACCADE_MODELS_DIR or [models].dir.
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}
pub(crate) fn models(args: ModelsArgs) -> Result<u8, CliError> {
    match args.operation {
        ModelsOperation::List {
            registry: p,
            cache: c,
            json,
        } => {
            let cache = cache(c.as_deref())?;
            emit(&status(&registry(p.as_deref())?, &cache), json)
        }
        ModelsOperation::Config { json } => emit(&config()?.to_json(), json),
        ModelsOperation::Pull {
            id,
            contract,
            registry: p,
            cache: c,
            json,
        } => {
            if id == "runtime" {
                #[cfg(feature = "local-models")]
                {
                    let cache = cache(c.as_deref())?;
                    let library =
                        saccade_core::wave7::runtime_install::pull(&cache).map_err(error)?;
                    return emit(
                        &serde_json::json!({"schema":models::MODELS_SCHEMA,
                        "runtime":saccade_core::wave7::runtime_install::status(&cache),
                        "ORT_DYLIB_PATH":library}),
                        json,
                    );
                }
                #[cfg(not(feature = "local-models"))]
                return Err(error(VisionError::Unavailable(
                    "compile local-models to provision the runtime".into(),
                )));
            }
            if id == "ocr" || id == "embedding" {
                return pull_contract(&id, contract.as_deref(), c.as_deref(), json);
            }
            let r = registry(p.as_deref())?;
            let m = r.model(&id).map_err(error)?;
            let paths = models::ensure(m, &cache(c.as_deref())?, true).map_err(error)?;
            emit(
                &serde_json::json!({"schema":models::MODELS_SCHEMA,"model":m,"status":"cached_verified","artifact_count":paths.len(),"source_parity":m.parity_sha256.is_some()}),
                json,
            )
        }
    }
}

/// `saccade models pull ocr|embedding`: provision a contract's pinned artifacts.
fn pull_contract(
    id: &str,
    contract: Option<&Path>,
    cache_flag: Option<&Path>,
    json: bool,
) -> Result<u8, CliError> {
    let dir = cache(cache_flag)?;
    #[cfg(any(feature = "ocr", feature = "embeddings"))]
    {
        let paths = if id == "ocr" {
            #[cfg(feature = "ocr")]
            {
                let c = match contract {
                    Some(p) => serde_json::from_slice(&saccade_core::general::input::bytes(
                        p,
                        2 * 1024 * 1024,
                    )?)?,
                    None => saccade_core::general::ocr::default_contract()?,
                };
                saccade_core::general::ocr::provision(&c, &dir)?
            }
            #[cfg(not(feature = "ocr"))]
            return Err(CliError::new(
                "feature_unavailable",
                "this build has no OCR runtime; install the media or full bundle",
            ));
        } else {
            #[cfg(feature = "embeddings")]
            {
                let path = match contract {
                    Some(p) => p.to_path_buf(),
                    None => config()?.embedding_contract.ok_or_else(|| {
                        CliError::usage(
                            "embedding needs --contract FILE or SACCADE_MODELS_EMBEDDING_CONTRACT / [models].embedding_contract",
                        )
                    })?,
                };
                let model = saccade_core::general::embedding::parse_model(
                    &saccade_core::general::input::bytes(&path, 2 * 1024 * 1024)?,
                )?;
                saccade_core::general::embedding::provision(&model, &dir)?
            }
            #[cfg(not(feature = "embeddings"))]
            return Err(CliError::new(
                "feature_unavailable",
                "this build has no embedding runtime; install the full bundle",
            ));
        };
        emit(
            &serde_json::json!({"schema":models::MODELS_SCHEMA,"pulled":id,"status":"cached_verified","artifact_count":paths.len(),"cache":dir}),
            json,
        )
    }
    #[cfg(not(any(feature = "ocr", feature = "embeddings")))]
    {
        let _ = (id, contract, dir, json);
        Err(CliError::new(
            "feature_unavailable",
            "this build has no OCR or embedding runtime; install the media or full bundle",
        ))
    }
}

#[derive(clap::Args)]
pub(crate) struct ModelOptions {
    /// Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry.
    #[arg(long)]
    registry: Option<PathBuf>,
    /// Deprecated: set SACCADE_MODELS_DIR or [models].dir.
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library.
    /// ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache.
    #[arg(long)]
    runtime_library: Option<PathBuf>,
    /// Deprecated: provision with `saccade models pull <id>` instead.
    #[arg(long)]
    allow_download: bool,
}
impl ModelOptions {
    /// Deprecated `--allow-download`: still provisions, now with a notice.
    fn download(&self) -> bool {
        if self.allow_download {
            saccade_core::model_config::deprecated_download_flag("--allow-download");
        }
        self.allow_download
    }
    /// ONNX runtime library: deprecated flag, else the shared configuration.
    #[cfg(feature = "local-models")]
    fn runtime(&self) -> Result<Option<PathBuf>, CliError> {
        runtime_flag(self.runtime_library.as_deref())
    }
    // Dependency resolution precedes pixel reads; observation replay bypasses inference.
    fn preflight(&self, requested: &[&str]) -> Result<(), CliError> {
        #[cfg(not(feature = "local-models"))]
        {
            let _ = requested;
            Err(error(VisionError::Unavailable("optional model runtime missing; fix: install a saccade build with local-models; saccade models list --json, then saccade models pull MODEL_ID and saccade models pull runtime".into())))
        }
        #[cfg(feature = "local-models")]
        {
            let reg = registry(self.registry.as_deref())?;
            let cache = cache(self.cache.as_deref())?;
            let library =
                saccade_core::wave7::runtime_install::resolve(self.runtime()?.as_deref(), &cache)
                    .map_err(error)?;
            saccade_core::optional::require_library(&library)?;
            for id in requested {
                let fallback = match *id {
                    "grounding-dino-tiny" => "owlv2-base",
                    "sam-2.1-tiny" => "efficientsam-ti",
                    _ => id,
                };
                let model = reg
                    .model(id)
                    .or_else(|_| reg.model(fallback))
                    .map_err(error)?;
                models::ensure(model, &cache, self.download()).map_err(|_|error(VisionError::Unavailable(format!("optional model {} missing or corrupt; fix: saccade models pull {} (use the same --registry and --cache); saccade models list --json",model.id,model.id))))?;
            }
            Ok(())
        }
    }
}
#[derive(clap::Args)]
pub(crate) struct LocateArgs {
    image: PathBuf,
    phrase: String,
    #[arg(long)]
    segment: bool,
    #[arg(long, default_value = "grounding-dino-tiny")]
    detector: String,
    #[arg(long, default_value = "sam-2.1-tiny")]
    segmenter: String,
    /// Explicit generated/frozen observation receipt; output is labelled replay.
    #[arg(long)]
    observations: Option<PathBuf>,
    /// New overlay PNG; existing files are never overwritten.
    #[arg(long)]
    overlay: Option<PathBuf>,
    #[command(flatten)]
    model: ModelOptions,
    #[arg(long)]
    json: bool,
}
pub(crate) fn write_png(path: &Path, pixels: &image::RgbImage) -> Result<(), CliError> {
    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);
    image::DynamicImage::ImageRgb8(pixels.clone())
        .write_to(&mut writer, image::ImageFormat::Png)
        .map_err(|e| CliError::io(e.to_string()))?;
    writer.flush().map_err(|e| CliError::io(e.to_string()))
}

pub(crate) fn locate(a: LocateArgs) -> Result<u8, CliError> {
    let json = a.json;
    let r = locate_measure(a)?;
    emit(&r, json)
}
fn locate_measure(a: LocateArgs) -> Result<saccade_core::wave7::vision::LocateReport, CliError> {
    use saccade_core::wave7::vision::{LocateReport, VisionImage};
    if a.observations.is_none() {
        let mut ids = vec![a.detector.as_str()];
        if a.segment {
            ids.push(a.segmenter.as_str());
        }
        a.model.preflight(&ids)?;
    }
    let image = VisionImage::load(&a.image).map_err(error)?;
    let r = if let Some(p) = &a.observations {
        if a.model.download() {
            return Err(CliError::usage(
                "observation replay does not download models",
            ));
        }
        LocateReport::replay(p, &image, &a.phrase, a.segment).map_err(error)?
    } else {
        let reg = registry(a.model.registry.as_deref())?;
        #[cfg(feature = "local-models")]
        {
            use saccade_core::wave7::{
                native::{EfficientSam, TextDetector},
                vision,
            };
            let cache = cache(a.model.cache.as_deref())?;
            let library = saccade_core::wave7::runtime_install::resolve(
                a.model.runtime()?.as_deref(),
                &cache,
            )
            .map_err(error)?;
            let detector_id =
                if a.detector == "grounding-dino-tiny" && reg.model(&a.detector).is_err() {
                    "owlv2-base"
                } else {
                    &a.detector
                };
            let mut detector = TextDetector::load(
                reg.model(detector_id).map_err(error)?,
                &cache,
                &library,
                a.model.download(),
            )
            .map_err(error)?;
            let mut segmenter = if a.segment {
                let id = if a.segmenter == "sam-2.1-tiny" && reg.model(&a.segmenter).is_err() {
                    "efficientsam-ti"
                } else {
                    &a.segmenter
                };
                Some(
                    EfficientSam::load(
                        reg.model(id).map_err(error)?,
                        &cache,
                        &library,
                        a.model.download(),
                    )
                    .map_err(error)?,
                )
            } else {
                None
            };
            vision::locate(
                &image,
                &a.phrase,
                &mut detector,
                segmenter.as_mut().map(|s| s as &mut dyn vision::Segmenter),
            )
            .map_err(error)?
        }
        #[cfg(not(feature = "local-models"))]
        {
            let _ = reg;
            return Err(error(VisionError::Unavailable(
                "compile local-models for native detection".into(),
            )));
        }
    };
    let out = a
        .overlay
        .clone()
        .unwrap_or_else(|| a.image.with_extension("locate.png"));
    write_png(
        &out,
        &saccade_core::wave7::vision::overlay(&image, &r.detections).map_err(error)?,
    )?;
    Ok(r)
}

#[cfg(feature = "local-vlm")]
#[derive(clap::Args)]
pub(crate) struct ObserveArgs {
    /// Bounded saccade observation request JSON with exact encoded images/transforms.
    request: PathBuf,
    #[arg(long)]
    endpoint: String,
    #[arg(long)]
    runtime_revision: String,
    /// Decode an explicitly recorded response without making any HTTP request.
    #[arg(long)]
    response: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
#[cfg(feature = "local-vlm")]
pub(crate) fn observe(a: ObserveArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::{
        local_vlm::LocalVlm,
        observation::{ObservationProvider, ObservationRequest},
    };
    let r: ObservationRequest = serde_json::from_slice(
        &models::read_bounded(&a.request, 24 * 1024 * 1024).map_err(error)?,
    )?;
    let mut p = LocalVlm {
        endpoint: a.endpoint,
        runtime_revision: a.runtime_revision,
    };
    let mut report = if let Some(path) = &a.response {
        p.decode(&r, &models::read_bounded(path, 1024 * 1024).map_err(error)?)
            .map_err(error)?
    } else {
        p.observe(&r).map_err(error)?
    };
    if report.provenance.runtime == "external-http" && a.response.is_some() {
        report.provenance.runtime = "replay".into();
    }
    emit(&report, a.json)
}

#[derive(clap::Args)]
pub(crate) struct QualityArgs {
    image: PathBuf,
    /// Full-reference metric command needs an explicit reference.
    #[arg(long)]
    reference: Option<PathBuf>,
    #[arg(long, default_value = "musiq")]
    metric: String,
    /// Explicit stand-in/frozen measurement receipt, always labelled replay.
    #[arg(long)]
    observations: Option<PathBuf>,
    #[command(flatten)]
    model: ModelOptions,
    #[arg(long)]
    json: bool,
}
fn learned_metric(name: &str) -> Result<saccade_core::wave7::quality::LearnedMetric, CliError> {
    use saccade_core::wave7::quality::LearnedMetric as M;
    match name {
        "lpips" => Ok(M::LpipsAlexV01),
        "dists" => Ok(M::Dists),
        "musiq" => Ok(M::MusiqTechnical),
        _ => Err(CliError::usage("metric must be lpips, dists or musiq")),
    }
}
pub(crate) fn quality(a: QualityArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::{
        quality::{self, QualityReport},
        vision::VisionImage,
    };
    let metric = learned_metric(&a.metric)?;
    if a.observations.is_none() {
        a.model.preflight(&[metric.model_id()])?;
    }
    let image = VisionImage::load(&a.image).map_err(error)?;
    let reference = a
        .reference
        .as_deref()
        .map(VisionImage::load)
        .transpose()
        .map_err(error)?;
    let r = if let Some(p) = &a.observations {
        let mut r: QualityReport =
            serde_json::from_slice(&models::read_bounded(p, 1024 * 1024).map_err(error)?)?;
        r.validate(metric, &image, reference.as_ref())
            .map_err(error)?;
        for m in &mut r.named_metrics {
            m.provenance.runtime = "replay".into();
            m.provenance.source_parity = false;
        }
        r
    } else {
        let reg = registry(a.model.registry.as_deref())?;
        let m = reg.model(metric.model_id()).map_err(error)?;
        #[cfg(feature = "local-models")]
        {
            let model_cache = cache(a.model.cache.as_deref())?;
            let library = saccade_core::wave7::runtime_install::resolve(
                a.model.runtime()?.as_deref(),
                &model_cache,
            )
            .map_err(error)?;
            let mut runtime = saccade_core::wave7::runtime::OnnxModel::load(
                m,
                &cache(a.model.cache.as_deref())?,
                &library,
                a.model.download(),
            )
            .map_err(error)?;
            quality::measure(metric, &image, reference.as_ref(), &mut runtime).map_err(error)?
        }
        #[cfg(not(feature = "local-models"))]
        {
            let _ = m;
            let _ = quality::QUALITY_SCHEMA;
            return Err(error(VisionError::Unavailable(
                "compile local-models for learned quality inference".into(),
            )));
        }
    };
    emit(&r, a.json)
}

#[derive(clap::Args)]
pub(crate) struct WatermarkArgs {
    image: PathBuf,
    /// Known legacy message bytes in hex; arbitrary recovered bits are not detection.
    #[arg(long)]
    expected_payload: Option<String>,
    #[arg(long, default_value_t = 36.)]
    quantization_step: f32,
    #[arg(long, default_value_t = 0.9)]
    minimum_agreement: f32,
    /// Explicit frozen/generated primary-decoder observation report.
    #[arg(long)]
    observations: Option<PathBuf>,
    /// Run the pinned Q neural graph; ECC/resize qualification remains unavailable.
    #[arg(long)]
    trustmark: bool,
    #[command(flatten)]
    model: ModelOptions,
    #[arg(long)]
    json: bool,
}
pub(crate) fn unhex(s: &str) -> Result<Vec<u8>, CliError> {
    if s.is_empty()
        || s.len() > 128
        || !s.len().is_multiple_of(2)
        || !s.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(CliError::usage("payload needs 1..64 hex-encoded bytes"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| CliError::usage("invalid hex payload"))
        })
        .collect()
}
pub(crate) fn watermark(a: WatermarkArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::{
        vision::VisionImage,
        watermark::{self, DwtConfig, WatermarkReport},
    };
    if a.trustmark && a.observations.is_none() {
        a.model.preflight(&["trustmark"])?;
    }
    let image = VisionImage::load(&a.image).map_err(error)?;
    let r = if let Some(p) = a.observations {
        let mut r: WatermarkReport =
            serde_json::from_slice(&models::read_bounded(&p, 1024 * 1024).map_err(error)?)?;
        r.validate(&image).map_err(error)?;
        for f in &mut r.findings {
            if let Some(p) = &mut f.provenance {
                p.runtime = "replay".into();
                p.source_parity = false;
            }
            f.interpretation = format!("Explicit observation replay. {}", f.interpretation);
        }
        r
    } else {
        let legacy = a
            .expected_payload
            .as_ref()
            .map(|s| {
                unhex(s).map(|expected_payload| DwtConfig {
                    expected_payload,
                    quantization_step: a.quantization_step,
                    minimum_agreement: a.minimum_agreement,
                })
            })
            .transpose()?;
        if a.trustmark {
            #[cfg(feature = "local-models")]
            {
                let reg = registry(a.model.registry.as_deref())?;
                let cache = cache(a.model.cache.as_deref())?;
                let library = saccade_core::wave7::runtime_install::resolve(
                    a.model.runtime()?.as_deref(),
                    &cache,
                )
                .map_err(error)?;
                let mut decoder = saccade_core::wave7::trustmark::TrustMarkQ::load(
                    reg.model("trustmark").map_err(error)?,
                    &cache,
                    &library,
                    a.model.download(),
                )
                .map_err(error)?;
                watermark::inspect(&image, Some(&mut decoder), legacy.as_ref()).map_err(error)?
            }
            #[cfg(not(feature = "local-models"))]
            return Err(error(VisionError::Unavailable(
                "compile local-models for TrustMark Q".into(),
            )));
        } else {
            watermark::inspect(&image, None, legacy.as_ref()).map_err(error)?
        }
    };
    emit(&r, a.json)
}

#[derive(clap::Args)]
pub(crate) struct FacesArgs {
    image: PathBuf,
    #[arg(long, default_value = "yunet-2026may")]
    detector: String,
    #[arg(long)]
    observations: Option<PathBuf>,
    /// Write a new strongly redacted PNG; never overwrite an original.
    #[arg(long)]
    blur_faces: Option<PathBuf>,
    #[command(flatten)]
    model: ModelOptions,
    #[arg(long)]
    json: bool,
}
fn face_report(
    a: &FacesArgs,
    image: &saccade_core::wave7::vision::VisionImage,
) -> Result<saccade_core::wave7::faces::FaceReport, CliError> {
    use saccade_core::wave7::faces::{self, FaceReport};
    if let Some(p) = &a.observations {
        let mut r: FaceReport =
            serde_json::from_slice(&models::read_bounded(p, 1024 * 1024).map_err(error)?)?;
        r.validate(image).map_err(error)?;
        r.provenance.runtime = "replay".into();
        r.provenance.source_parity = false;
        return Ok(r);
    }
    let reg = registry(a.model.registry.as_deref())?;
    let m = reg.model(&a.detector).map_err(error)?;
    #[cfg(feature = "local-models")]
    {
        let model_cache = cache(a.model.cache.as_deref())?;
        let library = saccade_core::wave7::runtime_install::resolve(
            a.model.runtime()?.as_deref(),
            &model_cache,
        )
        .map_err(error)?;
        let mut runtime = saccade_core::wave7::runtime::OnnxModel::load(
            m,
            &cache(a.model.cache.as_deref())?,
            &library,
            a.model.download(),
        )
        .map_err(error)?;
        faces::detect(image, &mut runtime).map_err(error)
    }
    #[cfg(not(feature = "local-models"))]
    {
        let _ = m;
        let _ = faces::FACES_SCHEMA;
        Err(error(VisionError::Unavailable(
            "compile local-models for face inference".into(),
        )))
    }
}
pub(crate) fn faces(a: FacesArgs) -> Result<u8, CliError> {
    if a.observations.is_none() {
        a.model.preflight(&[a.detector.as_str()])?;
    }
    let image = saccade_core::wave7::vision::VisionImage::load(&a.image).map_err(error)?;
    let r = face_report(&a, &image)?;
    if let Some(p) = &a.blur_faces {
        write_png(
            p,
            &saccade_core::wave7::faces::blur_faces(&image, &r).map_err(error)?,
        )?;
    }
    emit(&r, a.json)
}
#[derive(clap::Args)]
pub(crate) struct CropArgs {
    #[command(flatten)]
    faces: FacesArgs,
    /// Repeat aspect ratio W:H or original-pixel rectangle X,Y,W,H.
    #[arg(long, required = true)]
    crop: Vec<String>,
    /// Original-pixel X,Y, optional.
    #[arg(long)]
    focal_point: Option<String>,
}
fn numbers(s: &str, separator: char, count: usize) -> Result<Vec<f32>, CliError> {
    let values = s
        .split(separator)
        .map(|s| {
            s.parse::<f32>()
                .map_err(|_| CliError::usage("crop coordinates must be numbers"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != count || values.iter().any(|v| !v.is_finite()) {
        return Err(CliError::usage("invalid crop coordinate count/value"));
    }
    Ok(values)
}
pub(crate) fn crop(a: CropArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::{
        faces::{self, CropSpec},
        vision::{Rect, VisionImage},
    };
    if a.faces.observations.is_none() {
        a.faces.model.preflight(&[a.faces.detector.as_str()])?;
    }
    let image = VisionImage::load(&a.faces.image).map_err(error)?;
    let r = face_report(&a.faces, &image)?;
    let specs = a
        .crop
        .iter()
        .map(|s| {
            if s.contains(':') {
                numbers(s, ':', 2).map(|v| CropSpec::Ratio {
                    width: v[0],
                    height: v[1],
                })
            } else {
                numbers(s, ',', 4).map(|v| CropSpec::Rectangle {
                    rect: Rect {
                        x: v[0],
                        y: v[1],
                        width: v[2],
                        height: v[3],
                    },
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let focal = a
        .focal_point
        .as_deref()
        .map(|s| numbers(s, ',', 2).map(|v| [v[0], v[1]]))
        .transpose()?;
    let report = faces::crop_check(&image, &r, &specs, focal).map_err(error)?;
    if let Some(p) = &a.faces.blur_faces {
        write_png(p, &faces::blur_faces(&image, &r).map_err(error)?)?;
    }
    emit(&report, a.faces.json)
}

#[cfg(feature = "vision-providers")]
#[derive(clap::Args)]
pub(crate) struct ProviderArgs {
    request: PathBuf,
    #[arg(long)]
    provider: String,
    /// Startup env-file mapping for generic OpenAI-compatible or Azure deployment endpoints.
    #[arg(long,value_parser=["openai-compatible","azure-openai"])]
    endpoint_profile: Option<String>,
    /// Explicit recorded response; omit to show request mapping only (no credentials).
    #[arg(long)]
    response: Option<PathBuf>,
    #[arg(long, default_value = "pixels")]
    coordinates: String,
    #[arg(long)]
    json: bool,
}
#[cfg(feature = "vision-providers")]
pub(crate) fn provider(a: ProviderArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::{
        observation::ObservationRequest,
        providers::{Coordinates, Provider, ProviderAdapter},
    };
    let provider = match a.provider.as_str() {
        "claude" => Provider::Claude,
        "gpt" => Provider::Gpt,
        _ => return Err(CliError::usage("provider must be claude or gpt")),
    };
    let coordinates = match a.coordinates.as_str() {
        "pixels" => Coordinates::Pixels,
        "unit" => Coordinates::Unit,
        "thousand" => Coordinates::Thousand,
        _ => {
            return Err(CliError::usage(
                "coordinates must be pixels, unit or thousand",
            ));
        }
    };
    let r: ObservationRequest = serde_json::from_slice(
        &models::read_bounded(&a.request, 24 * 1024 * 1024).map_err(error)?,
    )?;
    let adapter = ProviderAdapter {
        provider,
        coordinates,
    };
    // wave8: fixture-only endpoint mapping, credentials never enter emitted JSON.
    if let Some(profile) = a.endpoint_profile {
        use saccade_core::media::endpoints::{Endpoint, Kind};
        let endpoint = Endpoint::load(if profile == "azure-openai" {
            Kind::AzureOpenai
        } else {
            Kind::OpenaiCompatible
        })
        .map_err(error)?;
        return if let Some(path) = a.response {
            emit(
                &endpoint
                    .decode(
                        &adapter,
                        &r,
                        &models::read_bounded(&path, 1024 * 1024).map_err(error)?,
                    )
                    .map_err(error)?,
                a.json,
            )
        } else {
            emit(&endpoint.mapping(&adapter, &r).map_err(error)?, a.json)
        };
    }
    if let Some(path) = a.response {
        emit(
            &adapter
                .decode(
                    &r,
                    &models::read_bounded(&path, 1024 * 1024).map_err(error)?,
                )
                .map_err(error)?,
            a.json,
        )
    } else {
        emit(
            &serde_json::json!({"schema":"saccade-provider-mapping.v1","interface_only":true,"endpoint":provider.endpoint(),"request_sha256":r.hash().map_err(error)?,"body":adapter.request(&r).map_err(error)?}),
            a.json,
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use clap::Parser;
    use saccade_core::wave7::{faces::*, quality::*, vision::*};
    fn json(path: &Path, v: &impl serde::Serialize) {
        std::fs::write(path, serde_json::to_vec(v).unwrap()).unwrap();
    }
    fn run(args: Vec<String>) -> u8 {
        let c = crate::Cli::try_parse_from(args).unwrap();
        crate::dispatch(c.command, false).unwrap()
    }
    #[test]
    fn generated_commands_render_and_assess_receipts_without_models() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("image.png");
        image::RgbImage::from_pixel(32, 24, image::Rgb([50, 100, 150]))
            .save(&p)
            .unwrap();
        let i = VisionImage::load(&p).unwrap();
        let face = FaceReport {
            schema: FACES_SCHEMA.into(),
            image_sha256: i.sha256.clone(),
            image_size: i.size(),
            faces: vec![Face {
                bbox: Rect {
                    x: 2.,
                    y: 4.,
                    width: 8.,
                    height: 8.,
                },
                score: 0.9,
                landmarks: vec![],
            }],
            provenance: Provenance::fixture("yunet-2026may"),
            limitations: FACE_LIMIT.into(),
        };
        let fp = d.path().join("faces.json");
        json(&fp, &face);
        let loc = LocateReport {
            schema: LOCATE_SCHEMA.into(),
            image_sha256: i.sha256.clone(),
            image_size: i.size(),
            phrase_sha256: models::digest(b"object"),
            detections: vec![Detection {
                bbox: face.faces[0].bbox,
                score: 0.9,
                mask: Some(Mask {
                    size: i.size(),
                    runs: vec![[130, 8]],
                }),
            }],
            detector: Provenance::fixture("generated-detector"),
            segmenter: Some(Provenance::fixture("generated-segmenter")),
        };
        let lp = d.path().join("locate.json");
        json(&lp, &loc);
        let overlay = d.path().join("overlay.png");
        let a = vec![
            "saccade".into(),
            "locate".into(),
            p.display().to_string(),
            "object".into(),
            "--segment".into(),
            "--observations".into(),
            lp.display().to_string(),
            "--overlay".into(),
            overlay.display().to_string(),
            "--json".into(),
        ];
        assert_eq!(run(a), 0);
        assert!(overlay.is_file());
        let redacted = d.path().join("redacted.png");
        assert_eq!(
            run(vec![
                "saccade".into(),
                "crop-check".into(),
                p.display().to_string(),
                "--crop".into(),
                "16:9".into(),
                "--crop".into(),
                "0,0,16,24".into(),
                "--observations".into(),
                fp.display().to_string(),
                "--blur-faces".into(),
                redacted.display().to_string(),
                "--json".into()
            ]),
            0
        );
        assert!(redacted.is_file());
        assert_eq!(
            run(vec![
                "saccade".into(),
                "faces".into(),
                p.display().to_string(),
                "--observations".into(),
                fp.display().to_string(),
                "--json".into()
            ]),
            0
        );
        let q = QualityReport {
            schema: QUALITY_SCHEMA.into(),
            image_sha256: i.sha256.clone(),
            reference_sha256: None,
            named_metrics: vec![QualityMeasurement {
                metric: LearnedMetric::MusiqTechnical,
                value: 50.,
                direction: "higher_is_better".into(),
                provenance: Provenance::fixture("musiq-technical"),
            }],
            affects_compare_verdict: false,
        };
        let qp = d.path().join("quality.json");
        json(&qp, &q);
        assert_eq!(
            run(vec![
                "saccade".into(),
                "quality-score".into(),
                p.display().to_string(),
                "--observations".into(),
                qp.display().to_string(),
                "--json".into()
            ]),
            0
        );
        assert_eq!(
            run(vec![
                "saccade".into(),
                "watermark".into(),
                p.display().to_string(),
                "--json".into()
            ]),
            0
        );
    }
    #[test]
    fn output_never_overwrites_an_original() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("original.png");
        std::fs::write(&p, b"original").unwrap();
        assert!(write_png(&p, &image::RgbImage::new(1, 1)).is_err());
        assert_eq!(std::fs::read(p).unwrap(), b"original");
    }
}

#[cfg(feature = "assist")]
#[derive(clap::Args, Default)]
pub(crate) struct Grounding {
    /// Attach advisory phrase localization to check-ui; never establish visibility by detection alone.
    #[arg(long)]
    pub locate: bool,
    #[arg(long)]
    pub locate_observations: Option<PathBuf>,
    #[arg(long)]
    pub locate_registry: Option<PathBuf>,
    #[arg(long)]
    pub locate_cache: Option<PathBuf>,
    #[arg(long)]
    pub locate_runtime_library: Option<PathBuf>,
}
#[cfg(feature = "assist")]
pub(crate) fn locate_for_check(
    image: &Path,
    phrase: &str,
    options: &Grounding,
) -> Result<serde_json::Value, CliError> {
    let report = locate_measure(LocateArgs {
        image: image.into(),
        phrase: phrase.into(),
        segment: false,
        detector: "grounding-dino-tiny".into(),
        segmenter: "sam-2.1-tiny".into(),
        observations: options.locate_observations.clone(),
        overlay: None,
        model: ModelOptions {
            registry: options.locate_registry.clone(),
            cache: options.locate_cache.clone(),
            runtime_library: options.locate_runtime_library.clone(),
            allow_download: false,
        },
        json: true,
    })?;
    Ok(
        serde_json::json!({"authority":"advisory model observation; never sufficient for a visible-condition verdict","report":report}),
    )
}
