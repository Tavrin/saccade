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
    /// Supplied finite-camera render manifest, bound to these exact mesh inputs.
    #[arg(long)]
    views: Option<PathBuf>,
    /// Write the combined geometry and optional multi-view packet.
    #[arg(long)]
    out: Option<PathBuf>,
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
    let views = if let Some(path) = args.views {
        let meta = std::fs::metadata(&path).map_err(|e| CliError::io(e.to_string()))?;
        if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
            return Err(CliError::usage(
                "asset camera manifest must be a regular file <=4 MiB",
            ));
        }
        let bytes = std::fs::read(&path).map_err(|e| CliError::io(e.to_string()))?;
        let manifest: saccade_core::asset_views::Manifest =
            crate::parse_contract(&bytes, "saccade-asset-views.v1")?;
        let evidence = analysis
            .evidence
            .as_ref()
            .ok_or_else(|| CliError::usage("geometry identity unavailable for camera binding"))?;
        let provenance = analysis
            .provenance
            .as_ref()
            .ok_or_else(|| CliError::usage("geometry input provenance unavailable"))?;
        let pin = |name: &str| -> Result<String, CliError> {
            provenance
                .resources
                .iter()
                .find(|r| r.name == name && r.role == "mesh_document")
                .and_then(|r| r.sha256.clone())
                .ok_or_else(|| CliError::usage("geometry document pin unavailable"))
        };
        Some(saccade_core::asset_views::measure(
            manifest,
            path.parent().unwrap_or(std::path::Path::new(".")),
            &[pin("baseline:document")?, pin("capture:document")?],
            &evidence.identity.geometry_sha256,
            &args.pair.unit,
        )?)
    } else {
        None
    };
    let code = u8::from(views.as_ref().is_some_and(|v| !v.passed));
    let document = Document {
        schema: "saccade-geometry.v1".into(),
        views,
        operation: Operation::Geometry {
            analysis: Box::new(analysis),
        },
    };
    if let Some(path) = args.out {
        crate::local_cmd::write_value(&path, &serde_json::to_value(&document)?)?;
    }
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&document)?))?;
    } else if let Operation::Geometry { analysis } = &document.operation {
        if let Some(e) = &analysis.evidence {
            crate::emit(&format!(
                "Geometry measurements ({}): sampled Hausdorff {:.9}, mean {:.9}, RMS {:.9}.\nOrdered geometry identical: {}. Sampled distances are not certified surface bounds. Appearance attributes are not checked by geometry.\n",
                e.unit, e.sampled_hausdorff, e.mean, e.rms, e.identity.ordered_geometry_identical
            ))?;
        }
        if let Some(v) = &document.views {
            crate::emit(&format!(
                "Supplied render views: {}/{} measured; declared thresholds passed: {}. Finite views establish sampled visibility only.\n",
                v.coverage.measured_views, v.coverage.declared_views, v.passed
            ))?;
        }
    }
    Ok(code)
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
                views: None,
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
