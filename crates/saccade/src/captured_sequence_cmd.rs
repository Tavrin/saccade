//! External capture transition and animation front doors.
use crate::agent::CliError;
use saccade_core::captured_sequence as sequence;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Inputs {
    /// saccade-captured-sequence-plan.v1 JSON, paths relative to its directory.
    plan: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
pub(crate) struct TransitionArgs {
    #[command(flatten)]
    inputs: Inputs,
    /// Zero-based switch request; omit for paired steady levels.
    #[arg(long)]
    change_frame: Option<usize>,
    #[arg(long)]
    maximum_pop: f64,
    #[arg(long)]
    maximum_duration_ms: f64,
    #[arg(long)]
    maximum_steady_error: f64,
    #[arg(long, default_value_t = 0.02)]
    settle_threshold: f64,
    #[arg(long, default_value_t = 2)]
    consecutive: usize,
    #[arg(long, default_value_t = 2)]
    window: usize,
    #[arg(long, default_value_t = 16)]
    tile_size: u32,
}
#[derive(clap::Args)]
pub(crate) struct AnimationArgs {
    #[command(flatten)]
    inputs: Inputs,
    #[arg(long)]
    maximum_frame_error: f64,
    #[arg(long)]
    maximum_local_error: f64,
    #[arg(long)]
    maximum_flicker: f64,
    #[arg(long, default_value_t = 16)]
    tile_size: u32,
}
struct Loaded {
    plan: sequence::Plan,
    images: sequence::Images,
    sources: BTreeMap<String, String>,
    inputs: Vec<PathBuf>,
}
fn read_image(
    root: &Path,
    name: &str,
    sources: &mut BTreeMap<String, String>,
    inputs: &mut Vec<PathBuf>,
) -> Result<image::DynamicImage, CliError> {
    let path = root
        .join(name)
        .canonicalize()
        .map_err(|e| CliError::io(e.to_string()))?;
    if !path.starts_with(root) {
        return Err(CliError::usage("capture path escapes plan directory"));
    }
    let bytes = saccade_core::evidence_quality::read(&path, 64 << 20)?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(16 << 20);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| CliError::usage(e.to_string()))?;
    sources.insert(
        format!("input:{name}"),
        saccade_core::localized::digest(&bytes),
    );
    inputs.push(path);
    Ok(image)
}
fn load(args: &Inputs) -> Result<Loaded, CliError> {
    let bytes = saccade_core::evidence_quality::read(&args.plan, 1 << 20)?;
    let plan: sequence::Plan = serde_json::from_slice(&bytes)?;
    plan.validate()?;
    let root = args
        .plan
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| CliError::io(e.to_string()))?;
    let mut sources = BTreeMap::from([("plan".into(), saccade_core::localized::digest(&bytes))]);
    let mut inputs = vec![args.plan.clone()];
    let mut total = 0_u64;
    let mut decode = |frames: &[sequence::Capture]| -> Result<Vec<image::RgbaImage>, CliError> {
        let mut out = Vec::new();
        for f in frames {
            let image = read_image(&root, &f.image, &mut sources, &mut inputs)?;
            if !matches!(
                image.color(),
                image::ColorType::Rgb8
                    | image::ColorType::Rgba8
                    | image::ColorType::L8
                    | image::ColorType::La8
            ) {
                return Err(CliError::usage("captures require encoded 8-bit SDR"));
            }
            total += u64::from(image.width()) * u64::from(image.height()) * 4;
            if total > 256 << 20 {
                return Err(CliError::usage("captures exceed 256 MiB decoded"));
            }
            out.push(image.to_rgba8());
        }
        Ok(out)
    };
    let candidate = decode(&plan.candidate)?;
    let reference = plan.reference.as_deref().map(decode).transpose()?;
    let dims = candidate[0].dimensions();
    let mut scope = vec![vec![true; dims.0 as usize * dims.1 as usize]; candidate.len()];
    for (i, s) in scope.iter_mut().enumerate() {
        if let Some(masks) = &plan.masks {
            let image = read_image(&root, &masks[i], &mut sources, &mut inputs)?;
            let image::DynamicImage::ImageLuma8(mask) = image else {
                return Err(CliError::usage("inclusion masks must be L8 PNGs"));
            };
            if mask.dimensions() != dims || mask.as_raw().iter().any(|v| !matches!(*v, 0 | 255)) {
                return Err(CliError::usage(
                    "masks require equal dimensions and binary 0/255 values",
                ));
            }
            for (included, v) in s.iter_mut().zip(mask.as_raw()) {
                *included &= *v == 255;
            }
        }
        if let Some(ids) = &plan.id_buffers {
            let image = read_image(&root, &ids[i], &mut sources, &mut inputs)?;
            if image.width() != dims.0 || image.height() != dims.1 {
                return Err(CliError::usage("ID buffer dimensions differ"));
            }
            let values: Vec<u16> = match image {
                image::DynamicImage::ImageLuma8(v) => {
                    v.as_raw().iter().map(|v| u16::from(*v)).collect()
                }
                image::DynamicImage::ImageLuma16(v) => v.into_raw(),
                _ => return Err(CliError::usage("ID buffers must be L8 or L16 PNGs")),
            };
            for (included, id) in s.iter_mut().zip(values) {
                *included &= plan.include_ids.contains(&id);
            }
        }
    }
    Ok(Loaded {
        plan,
        images: sequence::Images {
            candidate,
            reference,
            scope,
        },
        sources,
        inputs,
    })
}
fn write<T: serde::Serialize>(
    args: Inputs,
    loaded: Loaded,
    report: T,
    schema: &str,
    verdict: &str,
) -> Result<u8, CliError> {
    let paths: Vec<_> = loaded.inputs.iter().map(PathBuf::as_path).collect();
    crate::general_cmd::prepare_out(&args.out, &paths)?;
    let mut value = serde_json::to_value(report)?;
    value["sources"] = serde_json::to_value(loaded.sources)?;
    let file = args.out.join(format!("{schema}.json"));
    let linked = saccade_core::report_links::write(&file, &value)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&linked)?))?;
    } else {
        crate::emit(&format!("{schema}: {verdict}; wrote {}\n", file.display()))?;
    }
    Ok(u8::from(verdict == "regression"))
}
pub(crate) fn transition(args: TransitionArgs) -> Result<u8, CliError> {
    let loaded = load(&args.inputs)?;
    let report = sequence::transition(
        loaded.plan.clone(),
        &loaded.images,
        sequence::TransitionPolicy {
            change_frame: args.change_frame,
            maximum_pop: args.maximum_pop,
            maximum_duration_ms: args.maximum_duration_ms,
            maximum_steady_error: args.maximum_steady_error,
            settle_threshold: args.settle_threshold,
            consecutive: args.consecutive,
            window: args.window,
            tile_size: args.tile_size,
        },
    )?;
    let verdict = report.verdict.clone();
    write(
        args.inputs,
        loaded,
        report,
        sequence::TRANSITION_SCHEMA,
        &verdict,
    )
}
pub(crate) fn animation(args: AnimationArgs) -> Result<u8, CliError> {
    let loaded = load(&args.inputs)?;
    let report = sequence::animation(
        loaded.plan.clone(),
        &loaded.images,
        sequence::AnimationPolicy {
            maximum_frame_error: args.maximum_frame_error,
            maximum_local_error: args.maximum_local_error,
            maximum_flicker: args.maximum_flicker,
            tile_size: args.tile_size,
        },
    )?;
    let verdict = report.verdict.clone();
    write(
        args.inputs,
        loaded,
        report,
        sequence::ANIMATION_SCHEMA,
        &verdict,
    )
}
