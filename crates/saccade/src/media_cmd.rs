//! Wave 8 transports delegate to the core media analyzer.
use crate::agent::CliError;
use saccade_core::media::{self, Analyzer, Options, Profile};
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct AnalyzeArgs {
    source: String,
    #[arg(long,default_value="cpu-lite",value_parser=["cpu-lite","cpu-full","gpu"])]
    profile: String,
    /// Deprecated: set SACCADE_MODELS_DIR or [models].dir (see `saccade models config`).
    #[arg(long)]
    model_dir: Option<PathBuf>,
    /// Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry.
    #[arg(long)]
    registry: Option<PathBuf>,
    /// Per-section options JSON file.
    #[arg(long)]
    options: Option<PathBuf>,
    #[arg(long)]
    strict: bool,
    /// Repeat output size WxH.
    #[arg(long)]
    output_size: Vec<String>,
    #[arg(long)]
    json: bool,
}
pub(crate) fn error(e: media::MediaError) -> CliError {
    CliError::new(
        match e.code.as_str() {
            "invalid_media_input" => "invalid_media_input",
            "invalid_media_options" => "invalid_media_options",
            "request_too_large" => "request_too_large",
            "media_section_failed" => "media_section_failed",
            "vision_unavailable" => "vision_unavailable",
            "model_integrity" => "model_integrity",
            "runtime_incompatible" => "runtime_incompatible",
            "embedding_unavailable" => "embedding_unavailable",
            "text_embedding_unavailable" => "text_embedding_unavailable",
            "video_decode_unavailable" => "video_decode_unavailable",
            "video_decode_failed" => "video_decode_failed",
            "invalid_video_input" => "invalid_video_input",
            "index_mismatch" => "index_mismatch",
            "media_fetch_unavailable" => "media_fetch_unavailable",
            "media_fetch_failed" => "media_fetch_failed",
            "invalid_json" => "invalid_json",
            "io_error" => "io_error",
            "invalid_vision_input" => "invalid_vision_input",
            "gpu_unavailable" => "gpu_unavailable",
            "description_unavailable" => "description_unavailable",
            "analyzer_poisoned" => "analyzer_poisoned",
            "unsafe_path" => "unsafe_path",
            _ => "media_error",
        },
        e.message,
    )
}
pub(crate) fn emit<T: serde::Serialize>(value: &T, json: bool) -> Result<u8, CliError> {
    let text = if json {
        serde_json::to_string(value)?
    } else {
        serde_json::to_string_pretty(value)?
    };
    crate::emit(&format!("{text}\n"))?;
    Ok(0)
}
pub(crate) fn analyze(a: AnalyzeArgs) -> Result<u8, CliError> {
    let profile = match a.profile.as_str() {
        "cpu-full" => Profile::CpuFull,
        "gpu" => Profile::Gpu,
        _ => Profile::CpuLite,
    };
    let (model_dir, registry) =
        crate::wave7_cmd::analyzer_inputs(a.model_dir.as_deref(), a.registry.as_deref())?;
    let analyzer = if let Some(registry) = registry {
        Analyzer::with_registry(profile, model_dir, false, registry)
    } else {
        Analyzer::new(profile, model_dir, false)
    }
    .map_err(error)?;
    let mut opts: Options = if let Some(p) = a.options {
        serde_json::from_slice(
            &saccade_core::wave7::models::read_bounded(&p, 65536)
                .map_err(crate::wave7_cmd::error)?,
        )?
    } else {
        Options::default()
    };
    opts.strict |= a.strict;
    for s in a.output_size {
        let (w, h) = s
            .split_once('x')
            .ok_or_else(|| CliError::usage("output size must be WxH"))?;
        opts.output_sizes.push([
            w.parse()
                .map_err(|_| CliError::usage("invalid output width"))?,
            h.parse()
                .map_err(|_| CliError::usage("invalid output height"))?,
        ]);
    }
    emit(
        &analyzer.analyze_media(&a.source, &opts).map_err(error)?,
        a.json,
    )
}
#[cfg(feature = "mcp")]
pub(crate) fn mcp(
    policy: &saccade_core::root_policy::RootPolicy,
    args: &serde_json::Value,
) -> Result<serde_json::Value, CliError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        operation: String,
        image: PathBuf,
        #[serde(default)]
        options: Options,
    }
    let a: Request = serde_json::from_value(args.clone())?;
    if a.operation != "analyze_media" {
        return Err(CliError::usage("unknown media operation"));
    }
    if a.options.description
        || a.options.faces == Some(true)
        || a.options.text == Some(true)
        || a.options.embeddings == Some(true)
        || a.options.profile.is_some_and(|p| p != Profile::CpuLite)
    {
        return Err(CliError::usage(
            "MCP media records use cpu-lite; native/provider configuration belongs to server startup",
        ));
    }
    let path = policy.read(&a.image)?;
    let bytes =
        saccade_core::wave7::models::read_bounded(&path, saccade_core::general::input::MAX_BYTES)
            .map_err(crate::wave7_cmd::error)?;
    // Operator configuration only: a request never selects a location or a download.
    let (model_dir, registry) = crate::wave7_cmd::analyzer_inputs(None, None)?;
    let analyzer = if let Some(registry) = registry {
        Analyzer::with_registry(Profile::CpuLite, model_dir, false, registry)
    } else {
        Analyzer::new(Profile::CpuLite, model_dir, false)
    }
    .map_err(error)?;
    Ok(serde_json::to_value(
        analyzer.analyze_bytes(&bytes, &a.options).map_err(error)?,
    )?)
}
#[cfg(feature = "mcp")]
pub(crate) fn mcp_schema() -> serde_json::Value {
    serde_json::json!({"type":"object","properties":{"operation":{"const":"analyze_media"},"image":{"type":"string"},"options":{"type":"object"}},"required":["operation","image"],"additionalProperties":false})
}

#[derive(clap::Args)]
pub(crate) struct KeyframesArgs {
    source: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Requested samples per second, 0.1-10 (default 1; the decoder may lower it).
    #[arg(long, default_value_t = 1.)]
    sample_fps: f64,
    /// Change-point penalty, 0.001-10 (default 0.15; higher = fewer, longer shots; content-dependent).
    #[arg(long, default_value_t = 0.15)]
    shot_penalty: f64,
    #[arg(long)]
    json: bool,
}
pub(crate) fn keyframes(a: KeyframesArgs) -> Result<u8, CliError> {
    emit(
        &media::video::keyframes(
            &a.source,
            &a.out,
            &media::video::VideoOptions {
                sample_fps: a.sample_fps,
                shot_penalty: a.shot_penalty,
                ..Default::default()
            },
        )
        .map_err(error)?,
        a.json,
    )
}

#[derive(clap::Args)]
pub(crate) struct UsageArgs {
    source: PathBuf,
    #[arg(required=true,num_args=1..)]
    targets: Vec<PathBuf>,
    #[arg(long)]
    json: bool,
}
pub(crate) fn usage(a: UsageArgs) -> Result<u8, CliError> {
    let source = media::usage::Source::load(&a.source).map_err(error)?;
    let mut targets = Vec::new();
    for path in a.targets {
        if path.is_dir() {
            for p in saccade_core::general::input::files(&path, 1000)? {
                targets.push(p.to_string_lossy().into_owned());
            }
        } else {
            targets.push(path.to_string_lossy().into_owned());
        }
    }
    let result = media::usage::find_usage(&source, &targets).map_err(error)?;
    let failed = result["targets"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|r| r["status"] == "failed"));
    emit(&result, a.json)?;
    Ok(if failed { 2 } else { 0 })
}

#[cfg(feature = "workbench")]
pub(crate) fn serve_api(
    mut roots: Vec<PathBuf>,
    port: u16,
    max: usize,
    bind: std::net::IpAddr,
    token_file: Option<PathBuf>,
    model_dir: Option<PathBuf>,
    registry: Option<PathBuf>,
) -> Result<u8, CliError> {
    if roots.is_empty() {
        roots.push(std::env::current_dir().map_err(|e| CliError::io(e.to_string()))?);
    }
    let policy = saccade_core::root_policy::RootPolicy::new(&roots, None, false, &[])?;
    let path = token_file.or_else(|| {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config/saccade/api.env"))
            .filter(|p| p.exists())
    });
    let token = if let Some(path) = path {
        let bytes = saccade_core::general::input::bytes(&path, 65536).map_err(|_| {
            CliError::new(
                "invalid_media_options",
                "API token file unavailable (redacted)",
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            CliError::new("invalid_media_options", "API token file invalid (redacted)")
        })?;
        let mut token = None;
        for line in text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            let (name, value) = line
                .strip_prefix("export ")
                .unwrap_or(line)
                .split_once('=')
                .ok_or_else(|| {
                    CliError::new("invalid_media_options", "API token file invalid (redacted)")
                })?;
            if name.trim() == "SACCADE_API_TOKEN" {
                if token.is_some() {
                    return Err(CliError::new(
                        "invalid_media_options",
                        "duplicate API token (redacted)",
                    ));
                }
                let value = value.trim();
                let value = if value.len() >= 2
                    && (value.starts_with('"') && value.ends_with('"')
                        || value.starts_with('\'') && value.ends_with('\''))
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                token = Some(value.to_owned());
            }
        }
        Some(token.ok_or_else(|| {
            CliError::new("invalid_media_options", "API token missing (redacted)")
        })?)
    } else {
        None
    };
    let (model_dir, registry) =
        crate::wave7_cmd::analyzer_inputs(model_dir.as_deref(), registry.as_deref())?;
    let a = if let Some(registry) = registry {
        Analyzer::with_registry(Profile::CpuLite, model_dir, false, registry)
    } else {
        Analyzer::new(Profile::CpuLite, model_dir, false)
    }
    .map_err(error)?;
    media::http::Api::new(a, policy, max, token)
        .map_err(error)?
        .serve_on(bind, port)
        .map_err(error)?;
    Ok(0)
}
