//! Timed-text transport; decoding video stays with the external frame-map producer.
use crate::agent::CliError;
use saccade_core::{
    frame_map, general::input, localized::digest, timed_text as tt, ui_review::Source,
};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Plain UTF-8 WebVTT or SRT captions.
    timed_text: PathBuf,
    /// saccade-frame-map.v1 with presentation timestamps and relative image paths.
    frame_map: PathBuf,
    /// Fixed text region x,y,width,height in every frame's capture pixels.
    #[arg(long, required=true, value_parser=rect)]
    region: [u32; 4],
    /// Directory of image-bound UI OCR sources or timed-text source wrappers named INDEX.json.
    #[arg(long, conflicts_with = "ocr")]
    sources: Option<PathBuf>,
    /// Use cached PaddleOCR; absent model/runtime explicitly skips OCR, never downloads.
    #[arg(long)]
    ocr: bool,
    #[arg(long, default_value_t = 0.3)]
    timing_tolerance_s: f64,
    #[arg(long, default_value_t = 2.0)]
    search_s: f64,
    #[arg(long, default_value_t = 0.5)]
    maximum_gap_s: f64,
    #[arg(long, default_value_t = 4.5)]
    minimum_contrast: f64,
    #[arg(long, default_value_t = 8.0)]
    minimum_x_height_px: f64,
    #[arg(long, default_value_t = 0.35)]
    minimum_sharpness: f64,
    #[arg(long, default_value_t = 1.0)]
    minimum_stroke_px: f64,
    /// New or empty output directory for the report and artifact manifest.
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}
fn rect(text: &str) -> Result<[u32; 4], String> {
    text.split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "region requires x,y,width,height integers".to_string())?
        .try_into()
        .map_err(|_| "region requires four integers".into())
}
fn region_text(
    source: &Source,
    r: [u32; 4],
    declared_empty: Option<[u32; 4]>,
) -> (Option<String>, String) {
    let x = f64::from(r[0]);
    let y = f64::from(r[1]);
    let right = x + f64::from(r[2]);
    let bottom = y + f64::from(r[3]);
    let mut nodes = Vec::new();
    for n in &source.nodes {
        let Some(b) = n.bounds else {
            return (None, "OCR node lacks geometric bounds".into());
        };
        if b[0] >= right || b[1] >= bottom || b[0] + b[2] <= x || b[1] + b[3] <= y {
            continue;
        }
        if b[0] < x
            || b[1] < y
            || b[0] + b[2] > right
            || b[1] + b[3] > bottom
            || b[2] <= 0.
            || b[3] <= 0.
            || n.text.trim().is_empty()
            || n.ocr_confidence.is_none_or(|c| c < 80.)
        {
            return (
                None,
                "intersecting OCR text is clipped, empty or below confidence 80".into(),
            );
        }
        nodes.push(n);
    }
    nodes.sort_by(|a, b| {
        let aa = a.bounds.unwrap_or([0.; 4]);
        let bb = b.bounds.unwrap_or([0.; 4]);
        aa[1]
            .total_cmp(&bb[1])
            .then(aa[0].total_cmp(&bb[0]))
            .then(a.id.cmp(&b.id))
    });
    if declared_empty == Some(r) && !nodes.is_empty() {
        return (
            None,
            "empty-region declaration conflicts with observed text".into(),
        );
    }
    if nodes.is_empty() && declared_empty != Some(r) {
        return (
            None,
            "empty OCR source cannot establish absence without an explicit producer empty-region declaration".into(),
        );
    }
    (
        Some(
            nodes
                .iter()
                .map(|n| n.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        format!(
            "{} image-bound OCR; confidence >=80, uncalibrated; producer_declared_empty_region={:?}",
            source.kind, declared_empty
        ),
    )
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let cue_bytes = input::bytes(&args.timed_text, 4 << 20)?;
    let cues = tt::parse(&cue_bytes)?;
    let map_bytes = input::bytes(&args.frame_map, 4 << 20)?;
    let map: frame_map::FrameMap = serde_json::from_slice(&map_bytes)?;
    frame_map::check(&map, None, frame_map::CheckPolicy::default(), None)?;
    if map.frames.len() > tt::LIMIT || cues.len().saturating_mul(map.frames.len()) > 4_000_000 {
        return Err(CliError::usage(
            "maximum 4096 frames and 4 million cue/frame pairs; split long sequences",
        ));
    }
    let root = args
        .frame_map
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| CliError::io(e.to_string()))?;
    let source_root = args
        .sources
        .as_deref()
        .map(|p| p.canonicalize().map_err(|e| CliError::io(e.to_string())))
        .transpose()?;
    if source_root.as_ref().is_some_and(|p| !p.is_dir()) {
        return Err(CliError::usage("sources must be a directory"));
    }
    let policy = tt::Policy {
        timing_tolerance_s: args.timing_tolerance_s,
        search_s: args.search_s,
        maximum_gap_s: args.maximum_gap_s,
        legibility: saccade_core::text_quality::Policy {
            minimum_contrast: args.minimum_contrast,
            minimum_x_height_px: args.minimum_x_height_px,
            minimum_sharpness: args.minimum_sharpness,
            minimum_stroke_px: args.minimum_stroke_px,
        },
    };
    #[cfg(feature = "ocr")]
    let (mut engine, ocr_reason) = if args.ocr {
        let contract = saccade_core::general::ocr::default_contract()?;
        let cache = saccade_core::media::default_model_dir();
        match saccade_core::general::ocr::Engine::load(&contract, &cache, false) {
            Ok(engine) => (Some(engine), "cached PaddleOCR".into()),
            Err(error) => (
                None,
                format!("OCR skipped: cached PaddleOCR/model/runtime unavailable: {error}"),
            ),
        }
    } else {
        (
            None,
            "OCR skipped: not requested; no model downloaded".to_string(),
        )
    };
    #[cfg(not(feature = "ocr"))]
    let ocr_reason = if args.ocr {
        "OCR skipped: build lacks ocr feature"
    } else {
        "OCR skipped: not requested; no model downloaded"
    }
    .to_string();
    let mut observations = Vec::new();
    let mut inputs = vec![args.timed_text.clone(), args.frame_map.clone()];
    let mut total = cue_bytes.len() + map_bytes.len();
    for frame in &map.frames {
        let path = root
            .join(&frame.file)
            .canonicalize()
            .map_err(|e| CliError::io(e.to_string()))?;
        if !path.starts_with(&root) {
            return Err(CliError::usage(
                "frame resolves outside frame-map directory",
            ));
        }
        let bytes = input::bytes(&path, input::MAX_BYTES)?;
        total = total.saturating_add(bytes.len());
        if total > 512 << 20 {
            return Err(CliError::usage(
                "sequence inputs exceed cumulative 512 MiB budget; split sequence",
            ));
        }
        let hash = digest(&bytes);
        if frame
            .sha256
            .as_ref()
            .is_some_and(|h| !h.trim_start_matches("sha256:").eq_ignore_ascii_case(&hash))
        {
            return Err(CliError::usage("frame hash differs from frame map"));
        }
        let image = input::decode(&bytes)?;
        let r = args.region;
        if r[2] == 0
            || r[3] == 0
            || u64::from(r[0]) + u64::from(r[2]) > u64::from(image.width())
            || u64::from(r[1]) + u64::from(r[3]) > u64::from(image.height())
        {
            return Err(CliError::usage(
                "region must be nonempty and inside every frame",
            ));
        }
        inputs.push(path);
        let mut source_hash = None;
        let mut declared_empty = None;
        let source = if let Some(dir) = &source_root {
            let path = dir.join(format!("{}.json", frame.index));
            match path.try_exists().map_err(|e| CliError::io(e.to_string()))? {
                false => None,
                true => {
                    let path = path
                        .canonicalize()
                        .map_err(|e| CliError::io(e.to_string()))?;
                    if !path.starts_with(dir) {
                        return Err(CliError::usage(
                            "OCR source resolves outside source directory",
                        ));
                    }
                    let bytes = input::bytes(&path, 4 << 20)?;
                    total = total.saturating_add(bytes.len());
                    if total > 512 << 20 {
                        return Err(CliError::usage(
                            "sequence inputs exceed cumulative 512 MiB budget",
                        ));
                    }
                    source_hash = Some(digest(&bytes));
                    inputs.push(path);
                    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
                    if value["schema"] == tt::SOURCE_SCHEMA {
                        let wrapper: tt::Source = serde_json::from_value(value)?;
                        if wrapper
                            .declared_empty_region_px
                            .is_some_and(|empty| empty != r)
                        {
                            return Err(CliError::usage(
                                "declared empty region must match --region",
                            ));
                        }
                        declared_empty = wrapper.declared_empty_region_px;
                        Some(wrapper.source)
                    } else {
                        Some(serde_json::from_value::<Source>(value)?)
                    }
                }
            }
        } else {
            #[cfg(feature = "ocr")]
            {
                engine.as_mut().map(|e| e.recognize(&bytes)).transpose()?
            }
            #[cfg(not(feature = "ocr"))]
            {
                None
            }
        };
        let (text, reason) = if let Some(source) = source {
            source.validate(&hash, [image.width(), image.height()])?;
            if !["paddle_ocr", "tesseract_tsv", "ocrs", "provider_ocr"]
                .contains(&source.kind.as_str())
            {
                return Err(CliError::usage(
                    "timed text accepts OCR sources only, not DOM/accessibility text",
                ));
            }
            region_text(&source, r, declared_empty)
        } else {
            (
                None,
                if source_root.is_some() {
                    "OCR skipped: no imported source for frame".into()
                } else {
                    ocr_reason.clone()
                },
            )
        };
        #[cfg(feature = "text-quality")]
        let legibility = Some(saccade_core::text_quality::legibility(
            &image,
            r,
            0,
            policy.legibility,
        )?);
        #[cfg(not(feature = "text-quality"))]
        let legibility = None;
        observations.push(tt::Observation {
            index: frame.index,
            timestamp_s: frame.timestamp_s,
            image_sha256: hash,
            source_sha256: source_hash,
            text,
            ocr_reason: reason,
            legibility,
        });
    }
    let (state, cues, extra) = tt::check(&cues, &observations, policy)?;
    let report=tt::Report {schema:tt::SCHEMA.into(),report_id:None,source_refs:Vec::new(),state:state.clone(),timed_text_sha256:digest(&cue_bytes),frame_map_sha256:digest(&map_bytes),policy,region_px:args.region,observations,cues,extra,limitations:vec![
        "Only supplied presentation-time samples are checked; video extraction, timestamp origin and unsampled frames remain producer-owned.".into(),
        "Timing/gap limits allow only floating-point roundoff at eight machine epsilons of the clock scale; zero timing tolerance stays strict and coarse clocks abstain.".into(),
        "Timing offsets are first matching sample minus cue start; last_seen_s is not disappearance time. Gaps cannot establish continuous presence or exact onset.".into(),
        "Exact Unicode with whitespace normalization only; no spelling correction, semantic inference or provider calls. Overlapping identical cues abstain; use separate text regions for simultaneous captions.".into(),
        "Missing/text-mismatch findings require known OCR coverage under maximum_gap_s. Empty regions require an explicit image-bound producer annotation in the timed-text source wrapper; empty OCR alone cannot certify absence.".into(),
        "Pixel legibility reuses text-quality thresholds; it is not human readability, glyph completeness or compliance certification. Build text-quality for pixel evidence.".into(),
    ]};
    let value = saccade_core::report_links::decorate(&serde_json::to_value(&report)?)?;
    if let Some(out) = args.out.as_deref() {
        crate::general_cmd::prepare_out(
            out,
            &inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
        )?;
        crate::general_cmd::persist_document(&value, out)?;
        saccade_core::manifest::write(out, saccade_core::manifest::Anchors::default())?;
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
    } else {
        crate::emit(&format!(
            "{}: {} cues, {} extra sampled texts\n",
            state,
            report.cues.len(),
            report.extra.len()
        ))?;
    }
    Ok(match state.as_str() {
        "aligned" => 0,
        "failed" => 1,
        _ => 4,
    })
}
