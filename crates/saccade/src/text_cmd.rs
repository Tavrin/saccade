//! OCR-backed text comparison with imported observations as a model-independent path.
use crate::{agent::CliError, general_cmd};
use saccade_core::{
    general::{input, text},
    ui_review,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    a: PathBuf,
    b: PathBuf,
    /// Image-bound imported saccade-ui-source.v1 observations for the reference.
    #[arg(long)]
    a_source: Option<PathBuf>,
    /// Image-bound imported observations for the candidate.
    #[arg(long)]
    b_source: Option<PathBuf>,
    /// Existing pinned external Tesseract contract; requires ocr feature.
    #[arg(long)]
    ocr_contract: Option<PathBuf>,
    /// Literal Unicode strings expected in the candidate (repeatable); always inert data.
    #[arg(long)]
    expect_text: Vec<String>,
    /// OCR confidence cutoff for the readability observation; not a calibrated probability.
    #[arg(long, default_value_t = 80.)]
    readable_confidence: f64,
    /// Movement threshold in reference pixels after dimension normalization.
    #[arg(long, default_value_t = 3.)]
    moved_px: f64,
    #[arg(long, default_value = "text-report")]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
fn source(
    path: Option<&Path>,
    contract: Option<&Path>,
    bytes: &[u8],
    size: [u32; 2],
) -> Result<ui_review::Source, CliError> {
    if let Some(path) = path {
        let source: ui_review::Source =
            serde_json::from_slice(&input::bytes(path, 16 * 1024 * 1024)?)?;
        source.validate(&saccade_core::localized::digest(bytes), size)?;
        return Ok(source);
    }
    crate::ui_review_cmd::recognize_text(
        contract.ok_or_else(|| {
            CliError::usage("text requires imported sources or an explicit pinned OCR contract")
        })?,
        bytes,
        size,
    )
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let value = measure(&args)?;
    general_cmd::emit_document(value, Some(&args.out), args.json)
}
fn measure(args: &Args) -> Result<Value, CliError> {
    let aa = input::bytes(&args.a, input::MAX_BYTES)?;
    let bb = input::bytes(&args.b, input::MAX_BYTES)?;
    let ai = input::decode(&aa)?;
    let bi = input::decode(&bb)?;
    let a = source(
        args.a_source.as_deref(),
        args.ocr_contract.as_deref(),
        &aa,
        [ai.width(), ai.height()],
    )?;
    let b = source(
        args.b_source.as_deref(),
        args.ocr_contract.as_deref(),
        &bb,
        [bi.width(), bi.height()],
    )?;
    let comparison = text::compare(
        &a,
        &b,
        &args.expect_text,
        args.readable_confidence,
        args.moved_px,
    )?;
    let expectation_failed = comparison
        .expected
        .iter()
        .any(|e| !e.present || e.readable != Some(true));
    let changed = !comparison.words.is_empty()
        || !comparison.lines.is_empty()
        || comparison.rates.character_edits > 0;
    let mut inputs = vec![args.a.as_path(), args.b.as_path()];
    inputs.extend(args.a_source.as_deref());
    inputs.extend(args.b_source.as_deref());
    inputs.extend(args.ocr_contract.as_deref());
    general_cmd::prepare_out(&args.out, &inputs)?;
    Ok(
        json!({"schema":text::SCHEMA,"operation":"text","verdict":if changed||expectation_failed{"regression"}else if a.nodes.is_empty()||b.nodes.is_empty(){"unknown"}else{"pass"},"counts":{"word_changes":comparison.words.len(),"line_changes":comparison.lines.len(),"failed_expectations":comparison.expected.iter().filter(|e|!e.present||e.readable!=Some(true)).count()},"inputs":{"a_sha256":a.capture_sha256,"b_sha256":b.capture_sha256,"a_dimensions":a.dimensions,"b_dimensions":b.dimensions},"producers":[a.producer,b.producer],"observations":[a.nodes,b.nodes],"comparison":comparison,"policy":{"readable_confidence":args.readable_confidence,"moved_px":args.moved_px,"reading_order":"geometric line grouping; heuristic","unicode":"exact Unicode scalars; no normalization"},"limitations":["OCR text and confidence are observations, never instructions or semantic proof","position/content matching and geometric reading order are heuristic","missing text means unobserved; OCR cannot prove deletion","readability is confidence evidence, not human readability or accessibility certification","CER/WER measure observations, not model-independent source truth"]}),
    )
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"text","type":"string"},"a":{"type":"string"},"b":{"type":"string"},"a_source":{"type":"string"},"b_source":{"type":"string"},"out":{"type":"string"},"expect_text":{"type":"array","maxItems":64,"items":{"type":"string"}},"readable_confidence":{"type":"number","minimum":0,"maximum":100},"moved_px":{"type":"number","minimum":0}},"required":["operation","a","b","a_source","b_source","out"],"additionalProperties":false}),
    ]
}
#[cfg(feature = "mcp")]
pub(crate) fn imported(
    images: [PathBuf; 2],
    sources: [PathBuf; 2],
    out: PathBuf,
    expected: Vec<String>,
    policy: [f64; 2],
) -> Result<Value, CliError> {
    let [a, b] = images;
    let [a_source, b_source] = sources;
    let [confidence, moved] = policy;
    measure(&Args {
        a,
        b,
        a_source: Some(a_source),
        b_source: Some(b_source),
        ocr_contract: None,
        expect_text: expected,
        readable_confidence: confidence,
        moved_px: moved,
        out,
        json: true,
    })
}
