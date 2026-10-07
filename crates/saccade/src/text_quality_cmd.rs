//! O12/O17 additive transport; optional OCR never replaces pixel evidence.
use crate::agent::CliError;
use std::path::PathBuf;
#[derive(clap::Args)]
pub(crate) struct TofuArgs {
    image: PathBuf,
    /// Binary text-region mask: nonzero red includes; dimensions must match.
    #[arg(long)]
    mask: Option<PathBuf>,
    /// Declared expected Unicode text, never interpreted as instructions.
    #[arg(long)]
    expected_text: Option<String>,
    /// Image-bound imported OCR observations (saccade-ui-source.v1).
    #[arg(long, conflicts_with = "ocr")]
    source: Option<PathBuf>,
    /// Use cached default PaddleOCR; never downloads.
    #[arg(long)]
    ocr: bool,
    /// Optional report directory; --json always emits the full versioned report.
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
pub(crate) struct LegibilityArgs {
    baseline: PathBuf,
    /// One or more variant captures, in report order.
    #[arg(required = true, num_args = 1..)]
    variants: Vec<PathBuf>,
    /// Baseline capture-pixel rectangle x,y,width,height (repeatable, max 64).
    #[arg(long, required = true, value_parser = rect)]
    region: Vec<[u32; 4]>,
    #[arg(long, default_value_t = 4.5)]
    minimum_contrast: f64,
    #[arg(long, default_value_t = 8.)]
    minimum_x_height_px: f64,
    #[arg(long, default_value_t = 0.35)]
    minimum_sharpness: f64,
    #[arg(long, default_value_t = 1.)]
    minimum_stroke_px: f64,
    /// Use cached default PaddleOCR on baseline and variants; never downloads.
    #[arg(long)]
    ocr: bool,
    /// Imported OCR for baseline, paired with one --variant-source per variant.
    #[arg(long, conflicts_with = "ocr", requires = "variant_source")]
    baseline_source: Option<PathBuf>,
    #[arg(long, requires = "baseline_source")]
    variant_source: Vec<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
fn rect(text: &str) -> Result<[u32; 4], String> {
    let values: Vec<u32> = text
        .split(',')
        .map(|v| v.parse::<u32>())
        .collect::<Result<_, _>>()
        .map_err(|_| "region requires x,y,width,height integers".to_string())?;
    values
        .try_into()
        .map_err(|_| "region requires exactly four integers".into())
}
pub(crate) fn tofu(args: TofuArgs) -> Result<u8, CliError> {
    #[cfg(feature = "text-quality")]
    {
        enabled::tofu(args)
    }
    #[cfg(not(feature = "text-quality"))]
    {
        let _ = args;
        Err(CliError::new(
            "feature_unavailable",
            "requires text-quality",
        ))
    }
}
pub(crate) fn legibility(args: LegibilityArgs) -> Result<u8, CliError> {
    #[cfg(feature = "text-quality")]
    {
        enabled::legibility(args)
    }
    #[cfg(not(feature = "text-quality"))]
    {
        let _ = args;
        Err(CliError::new(
            "feature_unavailable",
            "requires text-quality",
        ))
    }
}
#[cfg(feature = "text-quality")]
mod enabled {
    use super::*;
    use saccade_core::{
        general::input,
        localized::digest,
        text_quality::{self as tq, State},
        ui_review::Source,
    };
    use std::path::Path;
    fn load(path: &Path) -> Result<(Vec<u8>, image::RgbaImage), CliError> {
        let bytes = input::bytes(path, input::MAX_BYTES)?;
        let image = input::decode(&bytes)?;
        Ok((bytes, image))
    }
    fn observations(
        source: Option<&Path>,
        use_ocr: bool,
        bytes: &[u8],
        size: [u32; 2],
    ) -> Result<(Option<Source>, String), CliError> {
        if let Some(path) = source {
            let value: Source = serde_json::from_slice(&input::bytes(path, 16 << 20)?)?;
            value.validate(&digest(bytes), size)?;
            if !["paddle_ocr", "tesseract_tsv", "ocrs", "provider_ocr"]
                .contains(&value.kind.as_str())
            {
                return Err(CliError::usage(
                    "text-quality imported sources must be OCR observations",
                ));
            }
            return Ok((Some(value), "image-bound imported OCR".into()));
        }
        if !use_ocr {
            return Ok((None, "OCR not requested; no model downloaded".into()));
        }
        #[cfg(feature = "ocr")]
        {
            let contract = saccade_core::general::ocr::default_contract()?;
            let cache = saccade_core::media::default_model_dir();
            if let Err(error) = saccade_core::general::ocr::preflight(&contract, &cache, false) {
                return Ok((None, format!("cached PaddleOCR unavailable: {error}")));
            }
            let source = saccade_core::general::ocr::recognize(&contract, &cache, bytes, false)?;
            Ok((
                Some(source),
                "cached PaddleOCR; confidence is uncalibrated".into(),
            ))
        }
        #[cfg(not(feature = "ocr"))]
        {
            Ok((
                None,
                "PaddleOCR unavailable: build lacks ocr feature".into(),
            ))
        }
    }
    fn evidence(source: Option<&Source>, reason: &str, r: Option<[u32; 4]>) -> tq::OcrEvidence {
        let Some(source) = source else {
            return tq::unavailable_ocr(reason);
        };
        let mut nodes: Vec<_> = source
            .nodes
            .iter()
            .filter(|n| {
                r.is_none_or(|r| {
                    n.bounds.is_some_and(|b| {
                        b[0] >= r[0] as f64
                            && b[1] >= r[1] as f64
                            && b[0] + b[2] <= (r[0] + r[2]) as f64
                            && b[1] + b[3] <= (r[1] + r[3]) as f64
                    })
                })
            })
            .collect();
        nodes.sort_by(|a, b| {
            let aa = a.bounds.unwrap_or([0.; 4]);
            let bb = b.bounds.unwrap_or([0.; 4]);
            aa[1]
                .total_cmp(&bb[1])
                .then(aa[0].total_cmp(&bb[0]))
                .then(a.id.cmp(&b.id))
        });
        if nodes.is_empty()
            || nodes
                .iter()
                .any(|n| n.text.is_empty() || n.ocr_confidence.is_none_or(|v| v < 80.))
        {
            return tq::unavailable_ocr("no sufficiently confident OCR text in region");
        }
        let text = nodes
            .iter()
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let replacement = text.contains('\u{fffd}');
        tq::OcrEvidence {
            state: if replacement {
                State::Candidates
            } else {
                State::InsufficientEvidence
            },
            text: Some(text),
            agrees_with_baseline: None,
            reason: reason.into(),
        }
    }
    fn emit<T: serde::Serialize>(
        report: &T,
        state: State,
        out: Option<&Path>,
        json: bool,
        inputs: &[&Path],
    ) -> Result<u8, CliError> {
        let value = serde_json::to_value(report)?;
        if let Some(out) = out {
            crate::general_cmd::prepare_out(out, inputs)?;
            crate::general_cmd::persist_document(&value, out)?;
        }
        if json {
            crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
        } else {
            crate::emit(&format!(
                "{}: {}\n",
                value["schema"].as_str().unwrap_or("text-quality"),
                value["state"].as_str().unwrap_or("unavailable")
            ))?;
        }
        Ok(match state {
            State::Candidates | State::Illegible => 1,
            State::Legible => 0,
            _ => 4,
        })
    }
    pub(super) fn tofu(args: TofuArgs) -> Result<u8, CliError> {
        if args.expected_text.as_ref().is_some_and(|s| s.len() > 65536) {
            return Err(CliError::usage("expected text exceeds 64 KiB"));
        }
        let (bytes, image) = load(&args.image)?;
        let mask = args.mask.as_deref().map(load).transpose()?;
        let (source, reason) = observations(
            args.source.as_deref(),
            args.ocr,
            &bytes,
            [image.width(), image.height()],
        )?;
        let mut report = tq::tofu(
            &image,
            mask.as_ref().map(|(_, i)| i),
            digest(&bytes),
            mask.as_ref().map(|(b, _)| digest(b)),
            args.expected_text,
            evidence(source.as_ref(), &reason, None),
        )?;
        if report.ocr.state == State::Candidates {
            report
                .reasons
                .push("OCR observed replacement character; corroborating evidence only".into());
        }
        let mut inputs = vec![args.image.as_path()];
        inputs.extend(args.mask.as_deref());
        inputs.extend(args.source.as_deref());
        emit(
            &report,
            report.state,
            args.out.as_deref(),
            args.json,
            &inputs,
        )
    }
    fn mapped(r: [u32; 4], a: [u32; 2], b: [u32; 2]) -> [u32; 4] {
        let x = (u64::from(r[0]) * u64::from(b[0]) / u64::from(a[0])) as u32;
        let y = (u64::from(r[1]) * u64::from(b[1]) / u64::from(a[1])) as u32;
        let endx = (u64::from(r[0] + r[2]) * u64::from(b[0])).div_ceil(u64::from(a[0])) as u32;
        let endy = (u64::from(r[1] + r[3]) * u64::from(b[1])).div_ceil(u64::from(a[1])) as u32;
        [x, y, endx - x, endy - y]
    }
    pub(super) fn legibility(args: LegibilityArgs) -> Result<u8, CliError> {
        if args.variants.len() > 64 || args.region.len() > 64 {
            return Err(CliError::usage("at most 64 variants and 64 regions"));
        }
        if args.baseline_source.is_some() && args.variant_source.len() != args.variants.len() {
            return Err(CliError::usage(
                "one variant-source per variant is required",
            ));
        }
        let policy = tq::Policy {
            minimum_contrast: args.minimum_contrast,
            minimum_x_height_px: args.minimum_x_height_px,
            minimum_sharpness: args.minimum_sharpness,
            minimum_stroke_px: args.minimum_stroke_px,
        };
        let (bytes, image) = load(&args.baseline)?;
        let size = [image.width(), image.height()];
        let (source, reason) =
            observations(args.baseline_source.as_deref(), args.ocr, &bytes, size)?;
        let mut baseline = Vec::new();
        for (index, r) in args.region.iter().enumerate() {
            let mut result = tq::legibility(&image, *r, index, policy)?;
            result.ocr = evidence(source.as_ref(), &reason, Some(*r));
            baseline.push(result);
        }
        let mut variants = Vec::new();
        for (index, path) in args.variants.iter().enumerate() {
            let (bytes, image) = load(path)?;
            let dim = [image.width(), image.height()];
            let (source, reason) = observations(
                args.variant_source.get(index).map(PathBuf::as_path),
                args.ocr,
                &bytes,
                dim,
            )?;
            let mut regions = Vec::new();
            for (i, r) in args.region.iter().enumerate() {
                let r = mapped(*r, size, dim);
                let mut result = tq::legibility(&image, r, i, policy)?;
                result.ocr = evidence(source.as_ref(), &reason, Some(r));
                if let (Some(a), Some(b)) = (&baseline[i].ocr.text, &result.ocr.text) {
                    let same = a == b;
                    result.ocr.agrees_with_baseline = Some(same);
                    result.ocr.state = if same {
                        State::Legible
                    } else {
                        State::Illegible
                    };
                    if !same {
                        result.state = State::Illegible;
                        result.reasons.push("ocr_disagreement".into());
                    }
                }
                if baseline[i].state != State::Legible && result.state == State::Legible {
                    result.state = State::InsufficientEvidence;
                    result
                        .reasons
                        .push("baseline did not establish legible pixel evidence".into());
                }
                regions.push(result);
            }
            variants.push(tq::Variant {
                index,
                image_sha256: digest(&bytes),
                dimensions: dim,
                regions,
            });
        }
        let states: Vec<_> = baseline
            .iter()
            .chain(variants.iter().flat_map(|v| &v.regions))
            .map(|r| r.state)
            .collect();
        let state = if states.contains(&State::Illegible) {
            State::Illegible
        } else if states.iter().all(|s| *s == State::Legible) {
            State::Legible
        } else {
            State::InsufficientEvidence
        };
        let report=tq::LegibilityReport {report_id:None, source_refs:Vec::new(), schema:tq::LEGIBILITY_SCHEMA.into(),state,baseline_sha256:digest(&bytes),baseline_dimensions:size,policy,baseline,variants,limitations:vec!["declared text regions and proportional dimension mapping required; layout changes need separately aligned captures".into(),"x-height is a minimum connected-component body-height proxy, not font metrics; small marks may lower it".into(),"SDR sRGB luminance on opaque dominant-background pixels; not WCAG certification or human readability".into(),"OCR agreement is exact observed Unicode and optional; missing OCR never becomes agreement".into(),"square .notdef and legitimate box-shaped symbols are ambiguous; glyph coverage remains unverified".into()]};
        let mut inputs = vec![args.baseline.as_path()];
        inputs.extend(args.variants.iter().map(PathBuf::as_path));
        inputs.extend(args.baseline_source.as_deref());
        inputs.extend(args.variant_source.iter().map(PathBuf::as_path));
        emit(&report, state, args.out.as_deref(), args.json, &inputs)
    }
}
