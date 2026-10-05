//! Wave 8 transports delegate to the core media analyzer.
use crate::agent::CliError;
use saccade_core::media::{self, Analyzer, Options, Profile};
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct AnalyzeArgs {
    source: String,
    #[arg(long,default_value="cpu-lite",value_parser=["cpu-lite","cpu-full","gpu"])]
    profile: String,
    #[arg(long, default_value = "/mnt/linux-extra/saccade-models")]
    model_dir: PathBuf,
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
    let analyzer = if let Some(p) = a.registry {
        Analyzer::with_registry(
            profile,
            a.model_dir,
            false,
            saccade_core::wave7::models::Registry::load(&p).map_err(crate::wave7_cmd::error)?,
        )
    } else {
        Analyzer::new(profile, a.model_dir, false)
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
    let analyzer = Analyzer::new(
        Profile::CpuLite,
        "/mnt/linux-extra/saccade-models".into(),
        false,
    )
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
    #[arg(long, default_value_t = 1.)]
    sample_fps: f64,
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
