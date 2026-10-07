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
    general_cmd::emit_document_saved(
        &value,
        &out.join(format!("{}.json", documents::SCHEMA)),
        json_output,
    )
}
#[cfg(feature = "documents")]
pub(crate) mod operation;

pub(crate) fn measure(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
) -> Result<Value, CliError> {
    #[cfg(feature = "documents")]
    {
        operation::measure(a, b, out, options, threshold, metric)
    }
    #[cfg(not(feature = "documents"))]
    {
        measure_local(a, b, out, options, threshold, metric, out)
    }
}

pub(super) fn measure_local(
    a: &Path,
    b: &Path,
    out: &Path,
    options: &general_cmd::CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
    scratch: &Path,
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
    let a_hash = saccade_core::localized::digest(&aa);
    let b_hash = saccade_core::localized::digest(&bb);
    let (map, map_source) = if let Some(path) = &options.page_map {
        let map: documents::page_map::Map =
            serde_json::from_slice(&input::bytes(path, 256 * 1024)?).map_err(|_| {
                CliError::new("document_page_map_invalid", "invalid declared page map")
            })?;
        map.validate(ac, bc, &a_hash, &b_hash)?;
        (map, "declared")
    } else if ac == 1 && bc == 1 {
        (
            documents::page_map::Map {
                schema: documents::page_map::SCHEMA.into(),
                reference_sha256: a_hash.clone(),
                candidate_sha256: b_hash.clone(),
                pairs: vec![documents::page_map::Pair {
                    reference: Some(1),
                    candidate: Some(1),
                }],
            },
            "single_page",
        )
    } else {
        return Err(CliError::new(
            "document_page_map_required",
            "multipage comparison requires --page-map with complete, input-bound correspondence",
        ));
    };
    general_cmd::prepare_out(out, &[a, b])?;
    let mut pages = Vec::new();
    let mut failures = 0;
    for (page, pair) in map.pairs.iter().enumerate() {
        let (Some(reference), Some(candidate)) = (pair.reference, pair.candidate) else {
            failures += 1;
            pages.push(json!({"page":page+1,"reference_page":pair.reference,"candidate_page":pair.candidate,"status":if pair.reference.is_none(){"new"}else{"missing"},"measurement":null}));
            continue;
        };
        let load = |bytes: &[u8], page: usize| -> Result<image::RgbaImage, CliError> {
            let image = if documents::format(bytes).is_some() {
                documents::page(bytes, dpi, page)?
            } else {
                input::decode(bytes)?
            };
            documents::worker::charge_pixels(u64::from(image.width()) * u64::from(image.height()))?;
            Ok(image)
        };
        let result = (|| -> Result<Value, CliError> {
            let ap = scratch.join("a.png");
            let bp = scratch.join("b.png");
            save(&load(&aa, reference - 1)?, &ap)?;
            save(&load(&bb, candidate - 1)?, &bp)?;
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
                pages.push(json!({"page":page+1,"reference_page":reference,"candidate_page":candidate,"status":status,"measurement":value,"artifact":format!("page-{:04}/saccade-registration.v1.json",page+1)}));
            }
            Err(error) => {
                if error.code == "document_total_pixel_limit"
                    || error.code == "document_output_byte_limit"
                {
                    return Err(error);
                }
                failures += 1;
                pages.push(json!({"page":page+1,"reference_page":reference,"candidate_page":candidate,"status":"error","error":{"code":error.code,"message":error.message}}));
            }
        }
    }
    let value = json!({"schema":documents::SCHEMA,"operation":"documents_compare","verdict":if failures==0{"pass"}else{"regression"},"counts":{"total":pages.len(),"failures":failures,"reference_pages":ac,"candidate_pages":bc},"inputs":{"a_sha256":a_hash,"b_sha256":b_hash},"page_map":{"source":map_source,"map":map},"caps":documents::worker::CAPS,"rendering":{"dpi":dpi,"svg":"resvg/usvg 0.48.1; CSS dimensions at 96 DPI","pdf":"hayro 0.3.0; PDF points at 72 DPI","alpha":"straight RGBA; white comparison background","resources":"offline; SVG text/images/active content refused; PDF interpreter warnings fail page"},"pages":pages,"limitations":["declared correspondence is user-supplied evidence, not semantic matching","PDF renderer does not implement every PDF feature; errors remain visible","raster equality establishes equality only at the declared density; originals are bound by encoded SHA-256"]});
    Ok(value)
}

#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"documents_compare","type":"string"},"reference":{"type":"string"},"capture":{"type":"string"},"out":{"type":"string"},"dpi":{"type":"number","minimum":36,"maximum":600},"threshold":{"type":"number","minimum":0,"maximum":1}},"required":["operation","reference","capture","out"],"additionalProperties":false}),
    ]
}

pub(crate) fn save(image: &impl PngSource, path: &Path) -> Result<(), CliError> {
    if !documents::worker::operation_is_active() {
        return image.save_unbounded(path);
    }
    let bytes = image.png()?;
    documents::worker::charge_output(bytes.len() as u64)?;
    std::fs::write(path, bytes).map_err(|e| CliError::io(e.to_string()))
}
pub(crate) trait PngSource {
    fn save_unbounded(&self, path: &Path) -> Result<(), CliError>;
    fn png(&self) -> Result<Vec<u8>, CliError>;
}
impl PngSource for image::RgbaImage {
    fn save_unbounded(&self, path: &Path) -> Result<(), CliError> {
        self.save(path).map_err(|e| CliError::io(e.to_string()))
    }
    fn png(&self) -> Result<Vec<u8>, CliError> {
        use image::ImageEncoder;
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                self.as_raw(),
                self.width(),
                self.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| CliError::io(e.to_string()))?;
        Ok(bytes)
    }
}
impl PngSource for image::DynamicImage {
    fn save_unbounded(&self, path: &Path) -> Result<(), CliError> {
        self.save(path).map_err(|e| CliError::io(e.to_string()))
    }
    fn png(&self) -> Result<Vec<u8>, CliError> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        self.write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(|e| CliError::io(e.to_string()))?;
        Ok(bytes.into_inner())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod budget_tests {
    use super::*;
    #[test]
    fn parent_output_quota_refuses_before_png_and_report_index_write() {
        let dir = tempfile::tempdir().unwrap();
        let _budget = documents::worker::operation_budget();
        documents::worker::charge_output(documents::worker::CAPS.output_bytes - 1).unwrap();
        let image = image::RgbaImage::new(8, 8);
        let path = dir.path().join("refused.png");
        assert_eq!(
            save(&image, &path).unwrap_err().code,
            "document_output_byte_limit"
        );
        assert!(!path.exists());
        let report =
            saccade_core::report_links::decorate(&json!({"schema":documents::SCHEMA})).unwrap();
        let error = saccade_core::report_links::index(&dir.path().join("report.json"), &report)
            .unwrap_err();
        assert!(matches!(
            error,
            saccade_core::Error::Document {
                code: "document_output_byte_limit"
            }
        ));
        // Capability-bound append refuses before creating an index inode.
        let index = dir.path().join("reports/index.jsonl");
        assert!(!index.exists());
        std::fs::create_dir_all(index.parent().unwrap()).unwrap();
        std::fs::write(&index, b"retained index bytes\n").unwrap();
        let error = saccade_core::report_links::index(&dir.path().join("report.json"), &report)
            .unwrap_err();
        assert!(matches!(
            error,
            saccade_core::Error::Document {
                code: "document_output_byte_limit"
            }
        ));
        assert_eq!(std::fs::read(&index).unwrap(), b"retained index bytes\n");
    }
}
