//! Optional static mesh front doors.
use crate::agent::CliError;
use clap::Args;
use saccade_core::geometry::{Document, Operation};
use std::path::PathBuf;

#[derive(Args)]
pub(crate) struct MeshPair {
    baseline: PathBuf,
    capture: PathBuf,
    /// Declared common coordinate unit; no conversion or registration is performed.
    #[arg(long)]
    unit: String,
}
#[derive(Args)]
pub(crate) struct GeometryArgs {
    #[command(flatten)]
    pair: MeshPair,
    /// Approximate area samples per direction, plus mandatory triangle/edge/vertex coverage.
    #[arg(long, default_value_t = 4096, value_parser = clap::value_parser!(u32).range(1..=100000))]
    samples: u32,
    #[arg(long)]
    json: bool,
}
#[derive(Args)]
pub(crate) struct IdentityArgs {
    #[command(flatten)]
    pair: MeshPair,
    #[arg(long)]
    json: bool,
}
pub(crate) fn compare(args: GeometryArgs) -> Result<u8, CliError> {
    let analysis = saccade_core::geometry::compare_files(
        &args.pair.baseline,
        &args.pair.capture,
        &args.pair.unit,
        args.samples as usize,
    )?;
    if args.json {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&Document {
                schema: "saccade-geometry.v1".into(),
                operation: Operation::Geometry {
                    analysis: Box::new(analysis)
                }
            })?
        ))?;
    } else if let Some(e) = analysis.evidence {
        crate::emit(&format!(
            "Geometry measurements ({}): sampled Hausdorff {:.9}, mean {:.9}, RMS {:.9}.\nOrdered geometry identical: {}. Sampled distances are not certified surface bounds. Appearance attributes are not checked.\n",
            e.unit, e.sampled_hausdorff, e.mean, e.rms, e.identity.ordered_geometry_identical
        ))?;
    }
    Ok(0)
}
pub(crate) fn identity(args: IdentityArgs) -> Result<u8, CliError> {
    let a = saccade_core::geometry::load(&args.pair.baseline)?;
    let b = saccade_core::geometry::load(&args.pair.capture)?;
    let analysis = saccade_core::geometry::identity(&a, &b, &args.pair.unit)?;
    let identical = analysis
        .evidence
        .as_ref()
        .is_some_and(|e| e.ordered_geometry_identical);
    if args.json {
        crate::emit(&format!(
            "{}\n",
            serde_json::to_string(&Document {
                schema: "saccade-geometry.v1".into(),
                operation: Operation::MeshIdentity {
                    analysis: Box::new(analysis)
                }
            })?
        ))?;
    } else {
        crate::emit(&format!(
            "Ordered geometry identity: {}. Scope: f64 world vertices, oriented triangle indices and declared units. Appearance attributes are not checked.\n",
            if identical { "identical" } else { "different" }
        ))?;
    }
    Ok(u8::from(!identical))
}
