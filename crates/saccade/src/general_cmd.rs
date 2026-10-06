//! Wave 6 CLI and report boundary; default comparison behaviour is unchanged.
use crate::agent::CliError;
use clap::{Args, ValueEnum};
use saccade_core::general::{input, registration};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Align {
    None,
    Translation,
    Similarity,
    Affine,
    Homography,
    Auto,
}
impl From<Align> for registration::Model {
    fn from(v: Align) -> Self {
        match v {
            Align::None => Self::None,
            Align::Translation => Self::Translation,
            Align::Similarity => Self::Similarity,
            Align::Affine => Self::Affine,
            Align::Homography => Self::Homography,
            Align::Auto => Self::Auto,
        }
    }
}
#[derive(Clone, Copy, Debug, ValueEnum, Default)]
pub(crate) enum Resample {
    #[default]
    Reference,
    Common,
}
impl From<Resample> for registration::Scale {
    fn from(v: Resample) -> Self {
        match v {
            Resample::Reference => Self::Reference,
            Resample::Common => Self::Common,
        }
    }
}
#[derive(Args, Default)]
pub(crate) struct CompareArgs {
    /// Declared document raster density, 36..600 DPI (default 96).
    #[arg(long)]
    pub(crate) dpi: Option<f64>,
    /// Explicit comparison question; no automatic model fallback.
    #[arg(long, value_enum)]
    pub(crate) question: Option<crate::capability_cmd::Question>,
    /// Supplied embedding export contract for same-content.
    #[arg(long, requires = "question")]
    pub(crate) model: Option<PathBuf>,
    /// Content-addressed model cache for same-content.
    #[arg(long, requires = "question")]
    pub(crate) cache: Option<PathBuf>,
    /// Explicit ONNX Runtime library for same-content.
    #[arg(long, requires = "question")]
    pub(crate) library: Option<PathBuf>,
    /// Image-bound reference text observations for same-text.
    #[arg(long, requires = "question")]
    pub(crate) reference_source: Option<PathBuf>,
    /// Image-bound candidate text observations for same-text.
    #[arg(long, requires = "question")]
    pub(crate) capture_source: Option<PathBuf>,
    /// Existing pinned OCR contract for same-text; requires ocr feature.
    #[arg(long, requires = "question")]
    pub(crate) ocr_contract: Option<PathBuf>,

    /// Explicit registration; defaults to the existing unregistered pipeline.
    #[arg(long, value_enum)]
    pub(crate) align: Option<Align>,
    /// Explicit cross-resolution comparison scale; registration evidence records it.
    #[arg(long, value_enum, requires = "align")]
    pub(crate) resample: Option<Resample>,
}
fn registration_error(error: registration::RegistrationError) -> CliError {
    match error {
        registration::RegistrationError::InsufficientInliers { .. } => {
            CliError::new("insufficient_inliers", error.to_string())
        }
        _ => CliError::new("invalid_geometry", error.to_string()),
    }
}

pub(crate) fn write_new(path: &Path, value: &Value) -> Result<(), CliError> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}
pub(crate) fn prepare_out(out: &Path, inputs: &[&Path]) -> Result<(), CliError> {
    // Existing transport policy prevents overwrites and writing inside source directories.
    saccade_core::run::guard_output_dir(out, inputs, &[])?;
    if out.exists()
        && std::fs::read_dir(out)
            .map_err(|e| CliError::io(e.to_string()))?
            .next()
            .is_some()
    {
        return Err(CliError::new(
            "not_empty_out_dir",
            "general comparator requires an empty output directory",
        ));
    }
    std::fs::create_dir_all(out).map_err(|e| CliError::io(e.to_string()))?;
    Ok(())
}
pub(crate) fn emit_document(
    value: Value,
    out: Option<&Path>,
    json_output: bool,
) -> Result<u8, CliError> {
    if let Some(out) = out {
        let file = persist_document(&value, out)?;
        if json_output {
            let mut result = json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":value["operation"],"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":file}],"next_actions":[]});
            if let Some(commands) = value.get("related_commands") {
                result["data"]["related_commands"] = commands.clone();
            }
            crate::emit(&format!("{}\n", result))?;
        } else {
            crate::emit(&format!("evidence: {}\n", file.display()))?;
        }
    } else {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    }
    Ok(u8::from(value["verdict"] == "regression"))
}
fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn save(image: &image::RgbaImage, path: &Path) -> Result<(), CliError> {
    image.save(path).map_err(|e| CliError::io(e.to_string()))
}

pub(crate) fn compare_document(
    reference: &Path,
    capture: &Path,
    out: &Path,
    options: &CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
) -> Result<Value, CliError> {
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(CliError::usage(
            "registration FLIP threshold must be in [0,1]",
        ));
    }
    if reference.is_file() != capture.is_file() {
        return Err(CliError::usage(
            "inputs must both be files or both be directories",
        ));
    }
    prepare_out(out, &[reference, capture])?;
    let model: registration::Model = options
        .align
        .ok_or_else(|| CliError::usage("alignment required"))?
        .into();
    let scale = options.resample.unwrap_or_default().into();
    let files = |path: &Path| -> Result<std::collections::BTreeMap<String, PathBuf>, CliError> {
        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| CliError::usage("input filename must be UTF-8"))?;
            return Ok(std::collections::BTreeMap::from([(
                name.into(),
                path.into(),
            )]));
        }
        if !path.is_dir() {
            return Err(CliError::io("input is neither file nor directory"));
        }
        let mut files = std::collections::BTreeMap::new();
        for entry in input::files(path, 100000)? {
            let name = entry
                .strip_prefix(path)
                .map_err(|e| CliError::io(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(name, entry);
            if files.len() > 100000 {
                return Err(CliError::usage(
                    "registration supports at most 100000 images",
                ));
            }
        }
        Ok(files)
    };
    let a = files(reference)?;
    let b = files(capture)?;
    let file_pair = reference.is_file() && capture.is_file();
    let names = if file_pair {
        a.keys().cloned().collect()
    } else {
        a.keys()
            .chain(b.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
    };
    let mut entries = Vec::new();
    let mut failures = 0;
    let mut errors = 0;
    for (i, name) in names.into_iter().enumerate() {
        let ap = a.get(&name);
        let bp = if file_pair {
            b.values().next()
        } else {
            b.get(&name)
        };
        let (Some(ap), Some(bp)) = (ap, bp) else {
            failures += 1;
            entries.push(json!({"name":name,"status":if ap.is_none(){"new"}else{"missing"}}));
            continue;
        };
        let measured = (|| -> Result<Value, CliError> {
            let reference_bytes = input::bytes(ap, input::MAX_BYTES)?;
            let capture_bytes = input::bytes(bp, input::MAX_BYTES)?;
            let registered = registration::register(
                &input::decode(&reference_bytes)?,
                &input::decode(&capture_bytes)?,
                model,
                scale,
            )
            .map_err(registration_error)?;
            let dir = out.join(format!("pair-{i:06}"));
            std::fs::create_dir(&dir).map_err(|e| CliError::io(e.to_string()))?;
            save(&registered.reference, &dir.join("reference.png"))?;
            save(&registered.capture, &dir.join("warped.png"))?;
            let (w, h) = registered.reference.dimensions();
            // Match border values to reference before spatial filtering; these pixels are excluded below.
            let mut filtered_capture = registered.capture.clone();
            for (idx, p) in filtered_capture.pixels_mut().enumerate() {
                if registered.excluded[idx] {
                    *p = *registered
                        .reference
                        .get_pixel((idx % w as usize) as u32, (idx / w as usize) as u32);
                }
            }
            let cmp = saccade_core::compare::compare_rgba(
                &filtered_capture,
                &registered.reference,
                &saccade_core::compare::CompareOptions::default(),
            )?;
            let metrics = saccade_core::compare::masked_metrics(
                &cmp.error_map,
                Some(&registered.excluded),
                w,
                h,
                [0, 0, w, h],
            )
            .ok_or_else(|| CliError::new("invalid_geometry", "no overlapping pixels"))?;
            let deciding = match metric {
                crate::MetricArg::Mean => metrics.mean,
                crate::MetricArg::Max => metrics.max,
                crate::MetricArg::P95 => metrics.p95,
                crate::MetricArg::P99 => metrics.p99,
            };
            let mut heat = cmp.heatmap_rgb();
            saccade_core::compare::hatch_masked(&mut heat, &registered.excluded);
            heat.save(dir.join("heatmap.png"))
                .map_err(|e| CliError::io(e.to_string()))?;
            let mask = image::GrayImage::from_fn(w, h, |x, y| {
                image::Luma([
                    if registered.excluded[y as usize * w as usize + x as usize] {
                        0
                    } else {
                        255
                    },
                ])
            });
            mask.save(dir.join("geometry-inclusion.png"))
                .map_err(|e| CliError::io(e.to_string()))?;
            Ok(
                json!({"name":name,"status":if deciding>threshold{"fail"}else{"pass"},"inputs":{"reference_sha256":saccade_core::localized::digest(&reference_bytes),"capture_sha256":saccade_core::localized::digest(&capture_bytes)},"registration":registered.evidence,"metrics":metrics,"value":deciding,"threshold":threshold,"metric":match metric{crate::MetricArg::Mean=>"mean",crate::MetricArg::Max=>"max",crate::MetricArg::P95=>"p95",crate::MetricArg::P99=>"p99"},"artifacts":{"reference":format!("pair-{i:06}/reference.png"),"capture":format!("pair-{i:06}/warped.png"),"heatmap":format!("pair-{i:06}/heatmap.png"),"geometry_inclusion":format!("pair-{i:06}/geometry-inclusion.png")}}),
            )
        })();
        match measured {
            Ok(v) => {
                if v["status"] == "fail" {
                    failures += 1;
                }
                entries.push(v);
            }
            Err(e) => {
                failures += 1;
                errors += 1;
                entries.push(json!({"name":name,"status":"error","error":{"code":e.code,"message":e.message}}));
            }
        }
    }
    if entries.is_empty() {
        failures += 1;
    }
    let value = json!({"schema":registration::SCHEMA,"operation":"registered_compare","verdict":if failures>0{"regression"}else{"pass"},"counts":{"total":entries.len(),"failures":failures,"errors":errors},"pipeline":{"align":model,"resample":scale,"question":"same-render","scope":"overlapping geometry only","limitations":["registration changes the compared question; it is not native identity","FLIP boundary neighbourhoods depend on interpolation; inspect geometry-inclusion.png"]},"entries":entries});
    Ok(value)
}

pub(crate) fn plain_options(
    intent: &crate::IntentArgs,
    meta: &crate::MetaArgs,
    require: &crate::MetaRequireArgs,
    perf: &crate::perf_cmd::PerfArgs,
    hdr: &crate::HdrArgs,
) -> bool {
    intent.intent.is_none()
        && intent.intent_file.is_none()
        && intent.changes_file.is_none()
        && meta.meta_name.is_none()
        && meta.meta_ignore.is_empty()
        && !require.require_matching_meta
        && require.declare.is_empty()
        && perf.gpu_clock_map.is_none()
        && perf.perf_name.is_none()
        && !perf.gpu_clocks_not_applicable
        && !perf.perf_noise_override
        && perf.perf_noise.is_none()
        && perf.perf_noise_k.is_none()
        && perf.perf_resolution.is_none()
        && perf.perf_resolution_ticks.is_none()
        && perf.perf_min_delta_ms.is_none()
        && perf.perf_min_delta_pct.is_none()
        && hdr.hdr_tonemapper.is_none()
        && hdr.hdr_exposures.is_none()
}

pub(crate) fn compare(
    reference: &Path,
    capture: &Path,
    out: &Path,
    options: &CompareArgs,
    threshold: f64,
    metric: crate::MetricArg,
    json_output: bool,
) -> Result<u8, CliError> {
    let mut value = compare_document(reference, capture, out, options, threshold, metric)?;
    if options.question.is_some() {
        value["pipeline"]["selected_question"] = json!("same-render");
        value["pipeline"]["command"] = json!("compare --align");
        value["pipeline"]["selection_reason"] =
            json!("explicit same-render with declared registration");
    }
    emit_document(value, Some(out), json_output)
}

pub(crate) fn persist_document(value: &Value, out: &Path) -> Result<PathBuf, CliError> {
    let file = out.join(format!(
        "{}.json",
        value["schema"]
            .as_str()
            .ok_or_else(|| CliError::usage("missing schema"))?
    ));
    write_new(&file, value)?;
    let escaped = html(&serde_json::to_string_pretty(value)?);
    let mut figures = String::new();
    if let Some(entries) = value["entries"].as_array() {
        for entry in entries.iter().take(100) {
            if let Some(artifacts) = entry["artifacts"].as_object() {
                for (name, path) in artifacts {
                    if let Some(path) = path.as_str() {
                        figures.push_str(&format!("<figure><figcaption>{}: {}</figcaption><a href=\"{}\"><img style=\"max-width:600px\" src=\"{}\"></a></figure>",html(entry["name"].as_str().unwrap_or("pair")),html(name),html(path),html(path)));
                    }
                }
            }
        }
    }
    if let Some(path) = value["error_level_analysis"]["artifact"].as_str() {
        figures.push_str(&format!("<figure><figcaption>Error level analysis: weak visual evidence only</figcaption><img style=\"max-width:600px\" src=\"{}\"></figure>",html(path)));
    }
    let content = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Saccade evidence</title><h1>Saccade evidence</h1><p>Measurements are conditional on the selected pipeline. Read limitations in the evidence. Geometry exclusions are hatched in heatmaps and black in inclusion masks. Preview shows at most 100 entries; JSON contains every entry.</p>{figures}<pre>{escaped}</pre>"
    );
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out.join("index.html"))
        .map_err(|e| CliError::io(e.to_string()))?;
    std::io::Write::write_all(&mut f, content.as_bytes())
        .map_err(|e| CliError::io(e.to_string()))?;
    Ok(file)
}
#[cfg(feature = "mcp")]
pub(crate) fn tool_schema() -> Value {
    let mut tool = json!({"name":"saccade_general","description":"Explicit general comparison pipelines with versioned evidence; no approval authority.","inputSchema":{"type":"object","properties":{"operation":{"const":"registered_compare","type":"string"},"reference":{"type":"string"},"capture":{"type":"string"},"out":{"type":"string"},"align":{"type":"string","enum":["none","translation","similarity","affine","homography","auto"]},"resample":{"type":"string","enum":["reference","common"]},"threshold":{"type":"number","minimum":0,"maximum":1},"metric":{"enum":["mean","p95","p99","max"]}},"required":["operation","reference","capture","out","align"],"additionalProperties":false},"outputSchema":{"type":"object"},"annotations":{"destructiveHint":false,"openWorldHint":false}});
    let registration = tool["inputSchema"].take();
    let mut variants = vec![registration];
    variants.extend(crate::documents_cmd::schemas());
    variants.extend(crate::hash_cmd::schemas());
    variants.extend(crate::embedding_cmd::schemas());
    variants.extend(crate::text_cmd::schemas());
    variants.push(crate::document_ocr_cmd::schema());
    variants.extend(crate::assess_cmd::schemas());
    variants.extend(crate::inspect_image_cmd::schemas());
    variants.extend(crate::capability_cmd::schemas());
    tool["inputSchema"] = json!({"type":"object","oneOf":variants});
    tool
}
