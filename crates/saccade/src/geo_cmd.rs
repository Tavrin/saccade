//! CLI transport for the optional raster extension.
use crate::{agent::CliError, general_cmd};
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand)]
enum Command {
    /// Measure same-grid multichannel TIFFs in native units.
    Compare(Compare),
    /// Compare z/x/y PNG/JPEG/WebP trees and report coverage by zoom.
    Tiles(Pair),
    /// Score single-band class TIFFs with overlap and boundary metrics.
    MaskMetrics(Mask),
}
#[derive(clap::Args)]
struct Pair {
    /// Reference input file or tile directory.
    reference: PathBuf,
    /// Candidate input file or tile directory.
    candidate: PathBuf,
    /// Empty artifact output directory, outside inputs.
    #[arg(long)]
    out: PathBuf,
    /// Emit versioned measurement JSON.
    #[arg(long)]
    json: bool,
}
#[derive(clap::Args)]
struct Compare {
    #[command(flatten)]
    pair: Pair,
    /// Three one-based band indices for R,G,B. Requires declared ranges.
    #[arg(long,value_delimiter=',',num_args=1,requires_all=["rgb_min","rgb_max"])]
    rgb_bands: Option<Vec<usize>>,
    /// Three native-unit lower bounds, shared by both rasters.
    #[arg(
        long,
        value_delimiter = ',',
        num_args = 1,
        allow_hyphen_values = true,
        requires = "rgb_bands"
    )]
    rgb_min: Option<Vec<f64>>,
    /// Three native-unit upper bounds, shared by both rasters.
    #[arg(
        long,
        value_delimiter = ',',
        num_args = 1,
        allow_hyphen_values = true,
        requires = "rgb_bands"
    )]
    rgb_max: Option<Vec<f64>>,
}
#[derive(clap::Args)]
struct Mask {
    #[command(flatten)]
    pair: Pair,
    /// Class predicate NAME=id=1,2, NAME=range=1,5, NAME=above=0 or NAME=mask; repeatable.
    #[arg(long = "class", conflicts_with = "each_label")]
    classes: Vec<String>,
    /// Score each non-void native label independently.
    #[arg(long)]
    each_label: bool,
    /// Reference void predicate; nodata on either side is also excluded.
    #[arg(long)]
    void: Option<String>,
    /// Euclidean boundary match tolerance in pixels.
    #[arg(long, default_value_t = 1.)]
    boundary_tolerance_px: f64,
}
fn error(e: saccade_geo::Error) -> CliError {
    CliError::new(e.code(), e.to_string())
}
fn triple<T>(values: Vec<T>) -> Result<[T; 3], CliError> {
    values
        .try_into()
        .map_err(|_| CliError::usage("RGB mapping needs exactly three values"))
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let (pair, value) = match args.command {
        Command::Compare(args) => {
            let mapping = match (args.rgb_bands, args.rgb_min, args.rgb_max) {
                (Some(bands), Some(min), Some(max)) => Some(saccade_geo::RgbMapping {
                    bands: triple(bands)?,
                    min: triple(min)?,
                    max: triple(max)?,
                }),
                (None, None, None) => None,
                _ => {
                    return Err(CliError::usage(
                        "RGB mapping needs explicit bands, min and max",
                    ));
                }
            };
            let value = saccade_geo::compare(
                &args.pair.reference,
                &args.pair.candidate,
                &args.pair.out,
                mapping.as_ref(),
            )
            .map_err(error)?;
            (args.pair, value)
        }
        Command::Tiles(pair) => {
            let value = saccade_geo::tiles::compare(&pair.reference, &pair.candidate, &pair.out)
                .map_err(error)?;
            (pair, value)
        }
        Command::MaskMetrics(args) => {
            use saccade_core::mask_metrics::{ClassSelection, ClassSpec, Policy};
            let classes = if args.each_label {
                ClassSelection::EachLabel
            } else if args.classes.is_empty() {
                ClassSelection::Foreground
            } else {
                ClassSelection::Named(
                    args.classes
                        .iter()
                        .map(|s| ClassSpec::parse(s))
                        .collect::<saccade_core::Result<_>>()?,
                )
            };
            let policy = Policy {
                classes,
                void: args.void.as_deref().map(ClassSpec::parse).transpose()?,
                boundary_tolerance_px: args.boundary_tolerance_px,
            };
            let value =
                saccade_geo::mask_metrics(&args.pair.reference, &args.pair.candidate, &policy)
                    .map_err(error)?;
            general_cmd::prepare_out(
                &args.pair.out,
                &[&args.pair.reference, &args.pair.candidate],
            )?;
            (args.pair, value)
        }
    };
    let exit = u8::from(value["verdict"] == "changed");
    let path = pair.out.join(format!(
        "{}.json",
        value["schema"].as_str().unwrap_or("raster")
    ));
    saccade_core::report_links::write(&path, &value)?;
    if pair.json {
        crate::emit(&format!("{}\n", serde_json::to_string_pretty(&value)?))?;
    } else {
        crate::emit(&format!("raster evidence: {}\n", path.display()))?;
    }
    Ok(exit)
}
