//! Transport for the first-party print extension.
use crate::{agent::CliError, general_cmd};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Reference CMYK TIFF/JPEG or supported single-image PDF raster.
    reference: PathBuf,
    /// Candidate CMYK TIFF/JPEG or supported single-image PDF raster.
    candidate: PathBuf,
    /// Empty artifact output directory.
    #[arg(long)]
    out: PathBuf,
    /// Override both embedded input ICC profiles with this CMYK ICC.
    #[arg(long)]
    input_profile: Option<PathBuf>,
    /// Target CMYK output ICC for gamut diagnostics.
    #[arg(long)]
    output_profile: Option<PathBuf>,
    /// Declared total area coverage limit, percent (0..400).
    #[arg(long)]
    tac_limit: f64,
    /// Declared physical raster density for small-mark detection.
    #[arg(long)]
    dpi: f64,
    /// Maximum text-like component height, points.
    #[arg(long, default_value_t = 12.)]
    small_text_points: f64,
    /// Emit the versioned, linked measurement as JSON.
    #[arg(long)]
    json: bool,
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let mut inputs = vec![args.reference.as_path(), args.candidate.as_path()];
    inputs.extend(args.input_profile.as_deref());
    inputs.extend(args.output_profile.as_deref());
    general_cmd::prepare_out(&args.out, &inputs)?;
    let options = saccade_print::Options {
        input_profile: args.input_profile,
        output_profile: args.output_profile,
        tac_limit: args.tac_limit,
        dpi: args.dpi,
        small_text_points: args.small_text_points,
    };
    let result = saccade_print::compare(&args.reference, &args.candidate, &args.out, &options)
        .map_err(|e| CliError::new(e.code(), e.to_string()))?;
    let path = args.out.join("saccade-print.v1.json");
    saccade_core::report_links::write(&path, &result)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&result)?))?;
    } else {
        crate::emit(&format!("print evidence: {}\n", path.display()))?;
    }
    Ok(0)
}

#[cfg(feature = "mcp")]
pub(crate) fn schema() -> serde_json::Value {
    serde_json::json!({"type":"object","properties":{"operation":{"const":"print_compare"},"reference":{"type":"string"},"capture":{"type":"string"},"out":{"type":"string"},"input_profile":{"type":"string"},"output_profile":{"type":"string"},"tac_limit":{"type":"number","minimum":0,"maximum":400},"dpi":{"type":"number","minimum":36,"maximum":2400},"small_text_points":{"type":"number","minimum":1,"maximum":72}},"required":["operation","reference","capture","out","tac_limit","dpi"],"additionalProperties":false})
}
