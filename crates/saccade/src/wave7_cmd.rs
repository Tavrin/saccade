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
