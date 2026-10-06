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
    #[command(flatten)]
    provider: crate::document_ocr_cmd::Options,
    a: PathBuf,
    b: PathBuf,
    /// Image-bound imported saccade-ui-source.v1 observations for the reference.
    #[arg(long)]
    a_source: Option<PathBuf>,
    /// Image-bound imported observations for the candidate.
    #[arg(long)]
    b_source: Option<PathBuf>,
    /// Override the default pinned PP-OCRv5 contract (or select external Tesseract).
    #[arg(long)]
    ocr_contract: Option<PathBuf>,
    /// Explicitly fetch SHA-pinned Rust OCR models into the contract cache.
    #[arg(long)]
    download_model: bool,
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
pub(crate) fn source(
    path: Option<&Path>,
    contract: Option<&Path>,
    bytes: &[u8],
    size: [u32; 2],
    download: bool,
) -> Result<ui_review::Source, CliError> {
    if let Some(path) = path {
        let source: ui_review::Source =
            serde_json::from_slice(&input::bytes(path, 16 * 1024 * 1024)?)?;
        source.validate(&saccade_core::localized::digest(bytes), size)?;
        return Ok(source);
    }
    if contract.is_none() {
        #[cfg(feature = "ocr")]
        {
            return Ok(saccade_core::general::ocr::recognize(
                &saccade_core::general::ocr::default_contract()?,
                &saccade_core::media::default_model_dir(),
                bytes,
                download,
            )?);
        }
        #[cfg(not(feature = "ocr"))]
        {
            return Err(CliError::new(
                "feature_unavailable",
                "default PP-OCRv5 OCR requires ocr; imported sources remain available",
            ));
        }
    }
    let contract = contract.ok_or_else(|| CliError::usage("OCR contract unavailable"))?;
    let contract_bytes = input::bytes(contract, 2 * 1024 * 1024)?;
    let raw: Value = serde_json::from_slice(&contract_bytes)?;
    let value = if raw["schema"] == saccade_core::wave7::models::REGISTRY_SCHEMA {
        let registry = saccade_core::wave7::models::Registry::load(contract)
            .map_err(crate::wave7_cmd::error)?;
        let contracts: Vec<_> = registry
            .contracts
            .values()
            .filter(|v| {
                v["schema"] == saccade_core::general::ocr::SCHEMA
                    || v["schema"] == "saccade-tesseract.v1"
            })
            .collect();
        if contracts.len() != 1 {
            return Err(CliError::usage(
                "shared registry needs one unambiguous OCR contract",
            ));
        }
        contracts[0].clone()
    } else {
        raw
    };
    if value["schema"] == saccade_core::general::ocr::SCHEMA {
        #[cfg(feature = "ocr")]
        {
            let c: saccade_core::general::ocr::Contract = serde_json::from_value(value)?;
            let cache = contract.parent().unwrap_or(Path::new(".")).join(&c.cache);
            return Ok(saccade_core::general::ocr::recognize(
                &c, &cache, bytes, download,
            )?);
        }
        #[cfg(not(feature = "ocr"))]
        {
            return Err(CliError::new(
                "feature_unavailable",
                "Rust OCR requires ocr",
            ));
        }
    }
    if download {
        return Err(CliError::usage(
            "--download-model applies only to a Rust OCR contract",
        ));
    }
    crate::ui_review_cmd::recognize_text(contract, bytes, size)
}
fn preflight(contract: Option<&Path>, download: bool) -> Result<(), CliError> {
    #[cfg(not(feature = "ocr"))]
    {
        let _ = (contract, download);
        Err(CliError::new(
            "feature_unavailable",
            "optional OCR runtime missing; fix: install an ocr build, then saccade models pull runtime and saccade text A B --download-model; or import image-bound sources",
        ))
    }
    #[cfg(feature = "ocr")]
    {
        let (value, dir) = if let Some(path) = contract {
            let raw: Value = serde_json::from_slice(&input::bytes(path, 2 << 20)?)?;
            let value = if raw["schema"] == saccade_core::wave7::models::REGISTRY_SCHEMA {
                let r = saccade_core::wave7::models::Registry::load(path)
                    .map_err(crate::wave7_cmd::error)?;
                let c = r
                    .contracts
                    .values()
                    .filter(|v| {
                        v["schema"] == saccade_core::general::ocr::SCHEMA
                            || v["schema"] == "saccade-tesseract.v1"
                    })
                    .collect::<Vec<_>>();
                if c.len() != 1 {
                    return Err(CliError::usage(
                        "shared registry needs one unambiguous OCR contract",
                    ));
                }
                c[0].clone()
            } else {
                raw
            };
            (value, path.parent().unwrap_or(Path::new(".")).to_path_buf())
        } else {
            (
                serde_json::to_value(saccade_core::general::ocr::default_contract()?)?,
                saccade_core::media::default_model_dir(),
            )
        };
        if value["schema"] == saccade_core::general::ocr::SCHEMA {
            let c: saccade_core::general::ocr::Contract = serde_json::from_value(value)?;
            let cache = if contract.is_some() {
                dir.join(&c.cache)
            } else {
                dir
            };
            saccade_core::general::ocr::preflight(&c, &cache, download)?;
        } else if let Some(executable) = value["executable"].as_str()
            && !dir.join(executable).is_file()
        {
            return Err(CliError::usage(
                "optional Tesseract executable missing; fix: install tesseract-ocr and supply its pinned contract",
            ));
        }
        Ok(())
    }
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let value = measure(&args)?;
    general_cmd::emit_document(value, Some(&args.out), args.json)
}
fn measure(args: &Args) -> Result<Value, CliError> {
    if args.provider.ocr_provider.is_some() {
        if args.a_source.is_some()
            || args.b_source.is_some()
            || args.ocr_contract.is_some()
            || args.download_model
        {
            return Err(CliError::usage(
                "document OCR provider conflicts with imported/local OCR options",
            ));
        }
        let value = crate::document_ocr_cmd::measure(
            &args.provider,
            [&args.a, &args.b],
            &args.expect_text,
        )?;
        general_cmd::prepare_out(&args.out, &[&args.a, &args.b])?;
        return Ok(value);
    }
    if args.a_source.is_none() || args.b_source.is_none() {
        preflight(args.ocr_contract.as_deref(), args.download_model)?;
    }
    let aa = input::bytes(&args.a, input::MAX_BYTES)?;
    let bb = input::bytes(&args.b, input::MAX_BYTES)?;
    let ai = input::decode(&aa)?;
    let bi = input::decode(&bb)?;
    let a = source(
        args.a_source.as_deref(),
        args.ocr_contract.as_deref(),
        &aa,
        [ai.width(), ai.height()],
        args.download_model,
    )?;
    let b = source(
        args.b_source.as_deref(),
        args.ocr_contract.as_deref(),
        &bb,
        [bi.width(), bi.height()],
        args.download_model,
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
        provider: Default::default(),
        a,
        b,
        a_source: Some(a_source),
        b_source: Some(b_source),
        ocr_contract: None,
        download_model: false,
        expect_text: expected,
        readable_confidence: confidence,
        moved_px: moved,
        out,
        json: true,
    })
}

pub(crate) fn routed(
    a: &Path,
    b: &Path,
    sa: Option<&Path>,
    sb: Option<&Path>,
    contract: Option<&Path>,
    out: &Path,
) -> Result<Value, CliError> {
    measure(&Args {
        provider: Default::default(),
        a: a.into(),
        b: b.into(),
        a_source: sa.map(Path::to_path_buf),
        b_source: sb.map(Path::to_path_buf),
        ocr_contract: contract.map(Path::to_path_buf),
        download_model: false,
        expect_text: vec![],
        readable_confidence: 80.,
        moved_px: 3.,
        out: out.into(),
        json: true,
    })
}
