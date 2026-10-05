//! Explicit box/mask and capture-bound DOM selector localization.
use crate::agent::CliError;
use saccade_core::localized::{self, FrozenRegion};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("region-source").required(true).args(["bbox","mask","selector","region"])))]
pub(crate) struct Args {
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
    #[arg(long, default_value_t = 0.01)]
    maximum_outside_flip: f32,
    #[arg(long, default_value_t = 67.0)]
    ppd: f32,
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let reference_bytes =
        std::fs::read(&args.reference).map_err(|e| CliError::io(e.to_string()))?;
    let candidate_bytes =
        std::fs::read(&args.candidate).map_err(|e| CliError::io(e.to_string()))?;
    let reference =
        image::load_from_memory(&reference_bytes).map_err(|e| CliError::usage(e.to_string()))?;
    let candidate =
        image::load_from_memory(&candidate_bytes).map_err(|e| CliError::usage(e.to_string()))?;
    let hash = localized::digest(&reference_bytes);
    let dimensions = [reference.width(), reference.height()];
    let region: FrozenRegion = if let Some(path) = args.region {
        serde_json::from_slice(&std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?)?
    } else if let Some(name) = args.selector {
        let path = args
            .metadata
            .ok_or_else(|| CliError::usage("selector metadata missing"))?;
        let metadata =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?)?;
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
        crate::emit(&format!(
            "{}\n",
            serde_json::json!({"schema":"saccade-localized-summary.v1","artifact":args.out.join("localized.json"),"region_id":measurement.region.region_id,"mask_sha256":measurement.region.mask_sha256,"inside":measurement.inside,"outside":measurement.outside,"boundary":measurement.boundary,"intended_change_detected":measurement.intended_change_detected,"collateral":measurement.collateral,"semantic_success":"unproven"})
        ))?;
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
