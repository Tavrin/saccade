//! Page-streamed document comparison using the existing registered measurement boundary.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{documents, input};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn is_document(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("svg") || s.eq_ignore_ascii_case("pdf"))
}
pub(crate) fn compare(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
    json_output: bool,
) -> Result<u8, CliError> {
    let value = measure(a, b, out, options, threshold, metric)?;
    general_cmd::emit_document(value, Some(out), json_output)
}
pub(crate) fn measure(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
) -> Result<Value, CliError> {
    if !cfg!(feature = "documents") {
        return Err(CliError::new(
            "feature_unavailable",
            "document input requires documents",
        ));
    }
    if !threshold.is_finite() || !(0. ..=1.).contains(&threshold) {
        return Err(CliError::usage("document FLIP threshold must be 0..1"));
    }
    let aa = input::bytes(a, input::MAX_BYTES)?;
    let bb = input::bytes(b, input::MAX_BYTES)?;
    let ac = documents::page_count(&aa)?;
    let bc = documents::page_count(&bb)?;
    let dpi = options.dpi.unwrap_or(documents::DEFAULT_DPI);
    if !dpi.is_finite() || !(36. ..=600.).contains(&dpi) {
        return Err(CliError::usage("document DPI must be 36..600"));
    }
    general_cmd::prepare_out(out, &[a, b])?;
    let scratch = tempfile::tempdir().map_err(|e| CliError::io(e.to_string()))?;
    let mut pages = Vec::new();
    let mut failures = 0;
    for page in 0..ac.max(bc) {
        if page >= ac || page >= bc {
            failures += 1;
            pages.push(json!({"page":page+1,"status":if page>=ac{"new"}else{"missing"},"measurement":null}));
            continue;
        }
        let load = |bytes: &[u8]| -> Result<image::RgbaImage, CliError> {
            Ok(if documents::format(bytes).is_some() {
                documents::page(bytes, dpi, page)?
            } else {
                input::decode(bytes)?
            })
        };
        let result = (|| -> Result<Value, CliError> {
            let ap = scratch.path().join("a.png");
            let bp = scratch.path().join("b.png");
            load(&aa)?
                .save(&ap)
                .map_err(|e| CliError::io(e.to_string()))?;
            load(&bb)?
                .save(&bp)
                .map_err(|e| CliError::io(e.to_string()))?;
            let page_options = general_cmd::CompareArgs {
                align: Some(options.align.unwrap_or(general_cmd::Align::None)),
                resample: options.resample,
                ..Default::default()
            };
            let sub = out.join(format!("page-{:04}", page + 1));
            let value =
                general_cmd::compare_document(&ap, &bp, &sub, &page_options, threshold, metric)?;
            general_cmd::persist_document(&value, &sub)?;
            Ok(value)
        })();
        match result {
            Ok(value) => {
                let status = value["verdict"].as_str().unwrap_or("error");
                if status != "pass" {
                    failures += 1;
                }
                pages.push(json!({"page":page+1,"status":status,"measurement":value,"artifact":format!("page-{:04}/saccade-registration.v1.json",page+1)}));
            }
            Err(error) => {
                failures += 1;
                pages.push(json!({"page":page+1,"status":"error","error":{"code":error.code,"message":error.message}}));
            }
        }
    }
    let value = json!({"schema":documents::SCHEMA,"operation":"documents_compare","verdict":if failures==0{"pass"}else{"regression"},"counts":{"total":pages.len(),"failures":failures,"reference_pages":ac,"candidate_pages":bc},"inputs":{"a_sha256":saccade_core::localized::digest(&aa),"b_sha256":saccade_core::localized::digest(&bb)},"rendering":{"dpi":dpi,"svg":"resvg/usvg 0.48.1; CSS dimensions at 96 DPI","pdf":"hayro 0.3.0; PDF points at 72 DPI","alpha":"straight RGBA; white comparison background","resources":"offline; SVG text/images/active content refused; PDF interpreter warnings fail page"},"pages":pages,"limitations":["page index is correspondence; inserted pages are not semantically aligned","PDF renderer does not implement every PDF feature; errors remain visible","raster equality establishes equality only at the declared density; originals are bound by encoded SHA-256"]});
    Ok(value)
}

#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"documents_compare","type":"string"},"reference":{"type":"string"},"capture":{"type":"string"},"out":{"type":"string"},"dpi":{"type":"number","minimum":36,"maximum":600},"threshold":{"type":"number","minimum":0,"maximum":1}},"required":["operation","reference","capture","out"],"additionalProperties":false}),
    ]
}
