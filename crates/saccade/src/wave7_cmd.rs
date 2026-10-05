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
