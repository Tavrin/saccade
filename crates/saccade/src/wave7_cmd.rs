//! Standalone wave 7 commands; no ambient model downloads or provider calls.
use crate::agent::CliError;
use saccade_core::wave7::models::{self, Registry, VisionError};
use std::path::{Path, PathBuf};

pub(crate) fn error(e: VisionError) -> CliError {
    let code = match &e {
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
fn registry(path: Option<&Path>) -> Result<Registry, CliError> {
    let p = match path {
        Some(p) => p.to_path_buf(),
        None => home()?.join(".config/saccade/models.json"),
    };
    if !p.exists() && path.is_none() {
        Ok(Registry::empty())
    } else {
        Registry::load(&p).map_err(error)
    }
}
fn cache(path: Option<&Path>) -> Result<PathBuf, CliError> {
    match path {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(home()?.join(".cache/saccade/models")),
    }
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
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Explicit opt-in to download only the named model's pinned artifacts.
    Pull {
        id: String,
        #[arg(long)]
        registry: Option<PathBuf>,
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
        } => emit(&registry(p.as_deref())?.status(&cache(c.as_deref())?), json),
        ModelsOperation::Pull {
            id,
            registry: p,
            cache: c,
            json,
        } => {
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

#[derive(clap::Args)]
pub(crate) struct ModelOptions {
    #[arg(long)]
    registry: Option<PathBuf>,
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Explicit ONNX Runtime 1.22 shared library; no ambient library probing.
    #[arg(long)]
    runtime_library: Option<PathBuf>,
    #[arg(long)]
    allow_download: bool,
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
fn write_png(path: &Path, pixels: &image::RgbImage) -> Result<(), CliError> {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| CliError::io(e.to_string()))?;
    image::DynamicImage::ImageRgb8(pixels.clone())
        .write_to(&mut std::io::BufWriter::new(f), image::ImageFormat::Png)
        .map_err(|e| CliError::io(e.to_string()))
}
pub(crate) fn locate(a: LocateArgs) -> Result<u8, CliError> {
    use saccade_core::wave7::vision::{LocateReport, VisionImage};
    let image = VisionImage::load(&a.image).map_err(error)?;
    let r = if let Some(p) = &a.observations {
        if a.model.allow_download {
            return Err(CliError::usage(
                "observation replay does not download models",
            ));
        }
        LocateReport::replay(p, &image, &a.phrase, a.segment).map_err(error)?
    } else {
        let reg = registry(a.model.registry.as_deref())?;
        let _ = reg.model(&a.detector).map_err(error)?;
        if a.segment {
            let _ = reg.model(&a.segmenter).map_err(error)?;
        }
        return Err(error(VisionError::Unavailable("checkpoint-specific text tokenizer/detector/SAM adapter is not qualified; supply --observations for explicit replay".into())));
    };
    let out = a
        .overlay
        .clone()
        .unwrap_or_else(|| a.image.with_extension("locate.png"));
    write_png(
        &out,
        &saccade_core::wave7::vision::overlay(&image, &r.detections).map_err(error)?,
    )?;
    emit(&r, a.json)
}

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
            let library = a.model.runtime_library.as_deref().ok_or_else(|| {
                CliError::usage("--runtime-library is required for ONNX inference")
            })?;
            let mut runtime = saccade_core::wave7::runtime::OnnxModel::load(
                m,
                &cache(a.model.cache.as_deref())?,
                library,
                a.model.allow_download,
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
    #[arg(long)]
    json: bool,
}
fn unhex(s: &str) -> Result<Vec<u8>, CliError> {
    if s.is_empty()
        || s.len() > 128
        || s.len() % 2 != 0
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
        watermark::inspect(&image, None, legacy.as_ref()).map_err(error)?
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
        let library = a
            .model
            .runtime_library
            .as_deref()
            .ok_or_else(|| CliError::usage("--runtime-library is required"))?;
        let mut runtime = saccade_core::wave7::runtime::OnnxModel::load(
            m,
            &cache(a.model.cache.as_deref())?,
            library,
            a.model.allow_download,
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
