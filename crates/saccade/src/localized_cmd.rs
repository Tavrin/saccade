//! Explicit box/mask and capture-bound DOM selector localization.
use crate::agent::CliError;
use saccade_core::localized::{self, FrozenRegion};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("region-source").required(true).args(["bbox","mask","selector","region","required_effect"])))]
pub(crate) struct Args {
    #[command(flatten)]
    strict_arms: crate::arms_cmd::StrictArgs,
    #[arg(long = "intended-variable")]
    intended_variables: Vec<String>,
    #[arg(long)]
    config: Option<PathBuf>,
    /// Required-effect policy JSON; records occupancy, including an empty mask.
    #[arg(long)]
    required_effect: Option<PathBuf>,
    /// Reference screenshot, retaining the intended region if candidate content disappears.
    reference: PathBuf,
    candidate: PathBuf,
    /// Pixel box x,y,width,height.
    #[arg(long = "box", value_delimiter = ',', num_args = 1)]
    bbox: Option<Vec<u32>>,
    /// Binary grayscale inclusion PNG: 255 inside, 0 outside.
    #[arg(long)]
    mask: Option<PathBuf>,
    /// Exact selector from producer metadata; one match required.
    #[arg(long, requires = "metadata")]
    selector: Option<String>,
    /// Capture-bound DOM geometry JSON.
    #[arg(long, requires = "selector")]
    metadata: Option<PathBuf>,
    /// Previously frozen inclusion-region JSON.
    #[arg(long)]
    region: Option<PathBuf>,
    /// New directory for the frozen region and measurements.
    #[arg(long)]
    out: PathBuf,
    /// Use maximum full-frame complement FLIP instead of exact native preservation.
    #[arg(long)]
    perceptual_outside: bool,
    /// Largest FLIP score allowed outside the intended region, 0-1 (default 0.01; above it fails).
    #[arg(long, default_value_t = 0.01)]
    maximum_outside_flip: f32,
    /// Viewing condition in pixels per degree of visual angle (default 67).
    #[arg(long, default_value_t = 67.0)]
    ppd: f32,
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let mut cfg = crate::load_config(args.config.as_deref())?;
    args.strict_arms.apply(&mut cfg.meta);
    cfg.meta.intended.extend(args.intended_variables);
    saccade_core::arms::enforce(&args.reference, &args.candidate, &cfg)?;
    if cfg.meta.require_valid_arms && !args.json {
        crate::arms_cmd::emit(
            &saccade_core::arms::check_paths(&args.reference, &args.candidate, &cfg.meta)?,
            false,
        )?;
    }
    let reference_bytes = saccade_core::evidence_quality::read(&args.reference, 128 << 20)
        .map_err(|e| CliError::io(e.to_string()))?;
    let candidate_bytes = saccade_core::evidence_quality::read(&args.candidate, 128 << 20)
        .map_err(|e| CliError::io(e.to_string()))?;
    let reference =
        image::load_from_memory(&reference_bytes).map_err(|e| CliError::usage(e.to_string()))?;
    let candidate =
        image::load_from_memory(&candidate_bytes).map_err(|e| CliError::usage(e.to_string()))?;
    if let Some(path) = &args.required_effect {
        use saccade_core::evidence_quality::effect;
        let policy: effect::RequiredEffect =
            serde_json::from_slice(&saccade_core::evidence_quality::read(path, 1 << 20)?)?;
        let root = path.parent().unwrap_or(std::path::Path::new("."));
        let dimensions = (reference.width(), reference.height());
        let b = effect::select(&policy.selection, &args.reference, root, dimensions)?;
        let c = effect::select(&policy.selection, &args.candidate, root, dimensions)?;
        let comparison = saccade_core::compare::compare_rgba(
            &candidate.to_rgba8(),
            &reference.to_rgba8(),
            &saccade_core::compare::CompareOptions {
                pixels_per_degree: args.ppd,
                ..Default::default()
            },
        )?;
        let result = effect::measure(
            &policy,
            &b,
            &c,
            &reference.to_rgba8(),
            &candidate.to_rgba8(),
            &comparison.error_map,
        )?;
        std::fs::create_dir(&args.out).map_err(|e| CliError::io(e.to_string()))?;
        let mut value = serde_json::to_value(&result)?;
        crate::arms_cmd::annotate(&mut value, &cfg.meta);
        crate::local_cmd::write_value(&args.out.join("required-effect.json"), &value)?;
        if args.json {
            crate::emit(&format!("{}\n", serde_json::to_string(&value)?))?;
        } else {
            crate::emit(&format!(
                "effect {}: baseline {} candidate {}; {:?}\n",
                policy.name, result.baseline_pixels, result.candidate_pixels, result.failures
            ))?;
        }
        return Ok(u8::from(!result.failures.is_empty()));
    }
    let hash = localized::digest(&reference_bytes);
    let dimensions = [reference.width(), reference.height()];
    let region: FrozenRegion = if let Some(path) = args.region {
        crate::parse_contract(
            &std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?,
            "saccade-frozen-region.v1",
        )?
    } else if let Some(name) = args.selector {
        let path = args
            .metadata
            .ok_or_else(|| CliError::usage("selector metadata missing"))?;
        let metadata = crate::parse_contract(
            &std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?,
            "saccade-dom-regions.v1",
        )?;
        localized::selector(&metadata, &name, &hash, dimensions)?
    } else if let Some(path) = args.mask {
        let data = std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?;
        let mask = image::load_from_memory(&data).map_err(|e| CliError::usage(e.to_string()))?;
        if mask.color() != image::ColorType::L8
            || [mask.width(), mask.height()] != dimensions
            || mask.as_bytes().iter().any(|&v| v != 0 && v != 255)
        {
            return Err(CliError::usage(
                "mask must be binary L8, reference-sized; 255 includes",
            ));
        }
        localized::freeze(
            hash.clone(),
            dimensions,
            mask.as_bytes()
                .iter()
                .map(|&v| u8::from(v == 255))
                .collect(),
            BTreeMap::from([
                ("method".into(), "mask_import".into()),
                ("source_mask_sha256".into(), localized::digest(&data)),
            ]),
        )?
    } else {
        let v = args.bbox.ok_or_else(|| CliError::usage("region missing"))?;
        let b: [u32; 4] = v
            .try_into()
            .map_err(|_| CliError::usage("box needs four coordinates"))?;
        localized::boxes(
            hash.clone(),
            dimensions,
            &[b],
            BTreeMap::from([("method".into(), "box".into())]),
        )?
    };
    std::fs::create_dir(&args.out).map_err(|e| CliError::io(e.to_string()))?;
    // Freeze the chosen reference region on disk before any measurement.
    crate::local_cmd::write_value(
        &args.out.join("frozen-region.json"),
        &serde_json::to_value(&region)?,
    )?;
    let measurement = localized::measure(
        &reference,
        &candidate,
        &hash,
        localized::digest(&candidate_bytes),
        region,
        localized::Policy {
            ppd: args.ppd,
            exact_outside: !args.perceptual_outside,
            maximum_outside_flip: args.maximum_outside_flip,
        },
    )?;
    crate::local_cmd::write_value(
        &args.out.join("localized.json"),
        &serde_json::to_value(&measurement)?,
    )?;
    if args.json {
        let mut value = serde_json::json!({"schema":"saccade-localized-summary.v1","artifact":args.out.join("localized.json"),"region_id":measurement.region.region_id,"mask_sha256":measurement.region.mask_sha256,"inside":measurement.inside,"outside":measurement.outside,"boundary":measurement.boundary,"intended_change_detected":measurement.intended_change_detected,"collateral":measurement.collateral,"semantic_success":"unproven"});
        crate::arms_cmd::annotate(&mut value, &cfg.meta);
        crate::emit(&format!("{value}\n"))?;
    } else {
        crate::emit(&format!(
            "localized: {}; {} inside and {} outside changed pixels; semantic success unproven\n",
            measurement.collateral,
            measurement.inside.changed_pixels,
            measurement.outside.changed_pixels
        ))?;
    }
    Ok(u8::from(
        measurement.collateral != "preserved" || !measurement.intended_change_detected,
    ))
}
