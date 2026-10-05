//! No-reference quality CLI; reports measurements without a content-independent quality gate.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{assessment, input};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    image: PathBuf,
    /// Reference quality measurements; deltas are image minus compare-to.
    #[arg(long)]
    compare_to: Option<PathBuf>,
    #[arg(long, default_value = "assessment-report")]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let value = measure(&args.image, args.compare_to.as_deref(), &args.out)?;
    general_cmd::emit_document(value, Some(&args.out), args.json)
}
fn one(path: &Path) -> Result<Value, CliError> {
    let bytes = input::bytes(path, input::MAX_BYTES)?;
    let image = input::decode(&bytes)?;
    Ok(
        json!({"encoded_sha256":saccade_core::localized::digest(&bytes),"dimensions":[image.width(),image.height()],"measures":assessment::assess(&image)?}),
    )
}
pub(crate) fn measure(
    image: &Path,
    compare_to: Option<&Path>,
    out: &Path,
) -> Result<Value, CliError> {
    let current = one(image)?;
    let before = compare_to.map(one).transpose()?;
    let mut delta = serde_json::Map::new();
    if let Some(before) = &before
        && let (Some(a), Some(b)) = (
            current["measures"].as_object(),
            before["measures"].as_object(),
        )
    {
        for (key, value) in a {
            delta.insert(
                key.clone(),
                match (value.as_f64(), b.get(key).and_then(Value::as_f64)) {
                    (Some(a), Some(b)) => json!(a - b),
                    _ => Value::Null,
                },
            );
        }
    }
    let mut inputs = vec![image];
    inputs.extend(compare_to);
    general_cmd::prepare_out(out, &inputs)?;
    Ok(
        json!({"schema":assessment::SCHEMA,"operation":"assess","verdict":"unknown","counts":{"images":if before.is_some(){2}else{1}},"image":current,"compare_to":before,"deltas":delta,"meanings":{"laplacian_variance":"luma-squared; lower can mean blur or flat content","edge_width_px":"contrast/local slope in pixels; larger can mean broader edges","noise_sigma":"MAD high-pass estimate in low-gradient areas, luma units; texture can contribute","block_boundary_excess":"8-pixel boundary difference excess, luma units; content can mimic blocks","occupied_luma_fraction":"occupied 8-bit levels/256; fewer can mean posterisation or flat content","shallow_plateau_step_fraction":"shallow jumps between plateaus; banding candidate, not a verdict","black_clip_fraction":"all-black RGB pixels after white compositing","white_clip_fraction":"all-white RGB pixels after white compositing","low_channel_fraction":"RGB samples at 0","high_channel_fraction":"RGB samples at 255","transparent_fraction":"alpha below 255; white compositing affects all measures"},"learned_score":{"status":"unavailable","reason":"no reviewed permissive model/export and pinned artifact supplied"},"limitations":["thresholds are content-dependent; no good/bad quality or authenticity verdict","noise, blockiness and banding indicators can respond to intended image structure","edge-width sampling can miss small features; absent edges are unavailable","8-bit SDR metrics; use HDR-FLIP for HDR pair comparison","paired deltas are image minus compare-to; resolution differences affect interpretation"]}),
    )
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"assess","type":"string"},"image":{"type":"string"},"compare_to":{"type":"string"},"out":{"type":"string"}},"required":["operation","image","out"],"additionalProperties":false}),
    ]
}
