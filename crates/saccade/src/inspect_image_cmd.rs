//! Single-image evidence without a real/fake classifier.
use crate::{agent::CliError, general_cmd};
use saccade_core::general::{assessment, hashing, input, integrity, registration};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
#[derive(clap::Args)]
pub(crate) struct Args {
    image: PathBuf,
    /// Explicitly include unsigned EXIF GPS coordinates in the report.
    #[arg(long)]
    include_gps: bool,
    /// Local saccade-hash.v1 / saccade-dedupe.v1 archive for candidate lookup.
    #[arg(long)]
    hash_index: Option<PathBuf>,
    /// Declared publication output size, WIDTHxHEIGHT; repeatable.
    #[arg(long)]
    output_size: Vec<String>,
    /// Crop x,y,width,height in raw raster pixels.
    #[arg(long, value_delimiter = ',', num_args = 1)]
    crop: Option<Vec<u32>>,
    /// Optional image-bound source/OCR observations for legibility evidence.
    #[arg(long)]
    text_source: Option<PathBuf>,
    #[arg(long, default_value = "image-inspection")]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let value = measure(&args)?;
    general_cmd::emit_document(value, Some(&args.out), args.json)
}
fn archive(path: &Path, hash: u64) -> Result<Value, CliError> {
    let bytes = input::bytes(path, 128 * 1024 * 1024)?;
    let index: Value = serde_json::from_slice(&bytes)?;
    if index["schema"] != hashing::HASH_SCHEMA && index["schema"] != hashing::DEDUPE_SCHEMA {
        return Err(CliError::new(
            "index_mismatch",
            "lookup needs a hash/dedupe document",
        ));
    }
    let entries = index["entries"]
        .as_array()
        .ok_or_else(|| CliError::usage("hash index entries missing"))?;
    if entries.len() > 100000 {
        return Err(CliError::usage("hash archive exceeds 100000 images"));
    }
    if index["preprocessing"] != "saccade-hash-white-triangle-srgb-luma-v1; phash DC bit zero" {
        return Err(CliError::new(
            "index_mismatch",
            "archive hash preprocessing differs",
        ));
    }
    let mut hits = Vec::new();
    for entry in entries {
        let text = entry["phash"]
            .as_str()
            .ok_or_else(|| CliError::usage("archive pHash missing"))?;
        if text.len() != 16
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CliError::usage("invalid archive pHash"));
        }
        let value = u64::from_str_radix(text, 16).map_err(|e| CliError::usage(e.to_string()))?;
        let d = (value ^ hash).count_ones();
        if d <= 6 {
            hits.push(
                json!({"path":entry["path"],"encoded_sha256":entry["encoded_sha256"],"hamming":d}),
            );
        }
    }
    hits.sort_by(|a, b| {
        a["hamming"]
            .as_u64()
            .cmp(&b["hamming"].as_u64())
            .then(a["path"].as_str().cmp(&b["path"].as_str()))
    });
    let matches = hits.len();
    hits.truncate(20);
    Ok(
        json!({"index_sha256":saccade_core::localized::digest(&bytes),"algorithm":"phash","threshold":6,"matches":matches,"returned":hits,"can_show":"near-duplicate candidates from the supplied archive","cannot_show":"hash collisions require review; archive paths/times do not prove an earlier original; current archive bytes not revalidated"}),
    )
}
fn measure(args: &Args) -> Result<Value, CliError> {
    let bytes = input::bytes(&args.image, input::MAX_BYTES)?;
    let image = input::decode(&bytes)?;
    let dimensions = [image.width(), image.height()];
    let crop = if let Some(values) = &args.crop {
        let crop: [u32; 4] = values
            .clone()
            .try_into()
            .map_err(|_| CliError::usage("crop needs x,y,width,height"))?;
        if crop[2] == 0
            || crop[3] == 0
            || crop[0]
                .checked_add(crop[2])
                .is_none_or(|v| v > image.width())
            || crop[1]
                .checked_add(crop[3])
                .is_none_or(|v| v > image.height())
        {
            return Err(CliError::usage("crop is empty or outside raw raster"));
        }
        crop
    } else {
        [0, 0, image.width(), image.height()]
    };
    if args.output_size.len() > 32 {
        return Err(CliError::usage("at most 32 output sizes"));
    }
    let mut outputs = Vec::new();
    for size in &args.output_size {
        let (w, h) = size
            .split_once('x')
            .ok_or_else(|| CliError::usage("output size must be WIDTHxHEIGHT"))?;
        let w: u32 = w
            .parse()
            .map_err(|_| CliError::usage("output width must be an integer"))?;
        let h: u32 = h
            .parse()
            .map_err(|_| CliError::usage("output height must be an integer"))?;
        if w == 0 || h == 0 || w > 32768 || h > 32768 {
            return Err(CliError::usage("output dimensions must be 1..32768"));
        }
        outputs.push(json!({"size":[w,h],"crop":crop,"requires_upsampling":w>crop[2]||h>crop[3],"aspect_ratio_matches":u64::from(w)*u64::from(crop[3])==u64::from(h)*u64::from(crop[2])}));
    }
    let mut headers = integrity::headers(&bytes, &image, args.include_gps)?;
    headers["compression"]["history_indicators"] = saccade_core::general::forensics::indicators(
        &image,
        &headers["compression"]["quantisation_tables"],
    )?;
    let copies = registration::copy_move(&image)
        .map_err(|e| CliError::new("invalid_geometry", e.to_string()))?;
    let hash = hashing::hash(&image);
    let lookup = args
        .hash_index
        .as_deref()
        .map(|p| archive(p, hash.phash))
        .transpose()?;
    let text = if let Some(path) = &args.text_source {
        let source: saccade_core::ui_review::Source =
            serde_json::from_slice(&input::bytes(path, 16 * 1024 * 1024)?)?;
        source.validate(&saccade_core::localized::digest(&bytes), dimensions)?;
        if source.nodes.len() > 2048 {
            return Err(CliError::usage("legibility supports <=2048 observations"));
        }
        let mut observed = Vec::new();
        for n in source.nodes.iter().take(128) {
            let Some(b) = n.bounds else { continue };
            if b[0] < 0.
                || b[1] < 0.
                || b[2] <= 0.
                || b[3] <= 0.
                || b[0] + b[2] > f64::from(image.width())
                || b[1] + b[3] > f64::from(image.height())
            {
                return Err(CliError::usage("text box outside raster"));
            }
            let mut lo = 255f64;
            let mut hi = 0f64;
            let (x, y, w, h) = (
                b[0] as u32,
                b[1] as u32,
                b[2].ceil() as u32,
                b[3].ceil() as u32,
            );
            let stride = ((f64::from(w) * f64::from(h) / 4096.).sqrt().ceil() as usize).max(1);
            for yy in (y..(y + h).min(image.height())).step_by(stride) {
                for xx in (x..(x + w).min(image.width())).step_by(stride) {
                    let p = image.get_pixel(xx, yy);
                    let alpha = f64::from(p[3]) / 255.;
                    let l = (0.2126 * f64::from(p[0])
                        + 0.7152 * f64::from(p[1])
                        + 0.0722 * f64::from(p[2]))
                        * alpha
                        + 255. * (1. - alpha);
                    lo = lo.min(l);
                    hi = hi.max(l);
                }
            }
            observed.push(json!({"text":n.text,"box":b,"ocr_confidence":n.ocr_confidence,"local_michelson_contrast":if hi+lo>0.{(hi-lo)/(hi+lo)}else{0.}}));
        }
        json!({"producer":source.producer,"observations":observed,"total":source.nodes.len(),"can_show":"observed OCR confidence and local raster luminance range","cannot_show":"confidence is not probability; min/max contrast is not foreground/background WCAG contrast or readability proof; preview capped at 128"})
    } else {
        json!({"status":"unavailable","reason":"no image-bound OCR/source observations supplied"})
    };
    let mut inputs = vec![args.image.as_path()];
    inputs.extend(args.hash_index.as_deref());
    inputs.extend(args.text_source.as_deref());
    general_cmd::prepare_out(&args.out, &inputs)?;
    let rgb = saccade_core::compare::flatten_over(&image, 255);
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90)
        .encode_image(&rgb)
        .map_err(|e| CliError::io(e.to_string()))?;
    let recompressed = saccade_core::compare::flatten_over(&input::decode(&encoded)?, 255);
    let layer = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let a = rgb.get_pixel(x, y);
        let b = recompressed.get_pixel(x, y);
        image::Rgb(std::array::from_fn(|c| {
            a[c].abs_diff(b[c]).saturating_mul(16)
        }))
    });
    layer
        .save(args.out.join("error-level-analysis.png"))
        .map_err(|e| CliError::io(e.to_string()))?;
    let credentials = saccade_core::general::credentials::inspect(&bytes)?;
    Ok(
        json!({"schema":integrity::SCHEMA,"operation":"inspect_image","verdict":"unknown","counts":{"copy_move_candidates":copies.len(),"publication_outputs":outputs.len()},"input":{"sha256":saccade_core::localized::digest(&bytes),"dimensions":dimensions,"orientation_policy":"raw raster dimensions; EXIF orientation not applied"},"headers":headers,"copy_move":{"candidates":copies,"can_show":"reciprocal FAST/oriented BRIEF self-matches clustered by translation","cannot_show":"natural repetition can match; rotated/projective copies and textureless edits may be missed; candidates do not establish manipulation"},"error_level_analysis":{"artifact":"error-level-analysis.png","quality":90,"display_gain":16,"assurance":"weak visual layer only","can_show":"local difference from one recompression with the current Rust JPEG encoder","cannot_show":"does not identify edited regions, authenticity or AI generation; depends on content and compression"},"near_duplicates":lookup,"publication":{"outputs":outputs,"quality":assessment::assess(&image)?,"text_legibility":text,"can_show":"raw pixel adequacy for declared output/crop and content-dependent quality measures","cannot_show":"no publication approval; no human readability or provenance proof"},"ai_generation":credentials["ai_generation"],"credentials":credentials,"limitations":["heuristics never produce real/fake or AI-generation verdicts","C2PA requires credentials; offline trust is distinct from visual truth; watermark evidence unavailable","extended/compressed metadata packets, encoder attribution and forensic discrimination qualification remain deferred","GPS is unsigned metadata and only emitted with --include-gps","semantic archive lookup is available separately through index query with embeddings"]}),
    )
}
#[cfg(feature = "mcp")]
pub(crate) fn schemas() -> Vec<Value> {
    vec![
        json!({"type":"object","properties":{"operation":{"const":"inspect_image","type":"string"},"image":{"type":"string"},"out":{"type":"string"},"include_gps":{"type":"boolean"},"hash_index":{"type":"string"},"text_source":{"type":"string"},"output_size":{"type":"array","maxItems":32,"items":{"type":"string"}},"crop":{"type":"array","minItems":4,"maxItems":4,"items":{"type":"integer","minimum":0}}},"required":["operation","image","out"],"additionalProperties":false}),
    ]
}
#[cfg(feature = "mcp")]
pub(crate) fn imported(
    image: PathBuf,
    out: PathBuf,
    include_gps: bool,
    hash_index: Option<PathBuf>,
    text_source: Option<PathBuf>,
    outputs: Vec<String>,
    crop: Option<Vec<u32>>,
) -> Result<Value, CliError> {
    measure(&Args {
        image,
        out,
        include_gps,
        hash_index,
        text_source,
        output_size: outputs,
        crop,
        json: true,
    })
}
