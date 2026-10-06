//! Deterministic grounded explanations and optional offline atomic proposals.
use crate::agent::CliError;
use saccade_core::grounded::{self, Explanation, Proposal};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Immutable comparison or localized measurement JSON.
    #[arg(long)]
    report: PathBuf,
    /// Optional JSON array of atomic proposals; no provider calls are made.
    #[arg(long)]
    proposals: Option<PathBuf>,
    /// New explanation JSON file.
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    json: bool,
}
pub(crate) fn explanation(path: &Path, proposals: Option<&Path>) -> Result<Explanation, CliError> {
    let bytes = std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let digest = saccade_core::localized::digest(&bytes);
    let catalog = match value["schema"].as_str() {
        Some(schema) if schema.starts_with("saccade-report.v") => {
            grounded::report_catalog(&crate::parse_contract(&bytes, "saccade-report.v1")?, digest)
        }
        Some(schema) if schema.starts_with("saccade-localized.v") => grounded::localized_catalog(
            &crate::parse_contract(&bytes, "saccade-localized.v1")?,
            digest,
        ),
        _ => {
            return Err(CliError::usage(
                "grounded explanation needs a comparison or localized report",
            ));
        }
    };
    let proposed: Vec<Proposal> = if let Some(path) = proposals {
        serde_json::from_slice(&std::fs::read(path).map_err(|e| CliError::io(e.to_string()))?)?
    } else {
        grounded::deterministic(&catalog)
    };
    Ok(grounded::verify(catalog, &proposed)?)
}
pub(crate) fn page(
    path: &Path,
    limit: usize,
    cursor: Option<&str>,
) -> Result<serde_json::Value, CliError> {
    let explanation = explanation(path, None)?;
    let offset = cursor
        .map(|s| {
            s.parse::<usize>()
                .map_err(|_| CliError::usage("cursor must be an integer"))
        })
        .transpose()?
        .unwrap_or(0);
    if !(1..=5).contains(&limit) {
        return Err(CliError::usage("grounded page limit must be 1..5"));
    }
    let claims: Vec<_> = explanation.claims.iter().skip(offset).take(limit).collect();
    let facts: Vec<_> = explanation
        .catalog
        .facts
        .iter()
        .filter(|f| {
            claims
                .iter()
                .any(|c| c.observation.evidence_ids.contains(&f.id))
        })
        .collect();
    let regions: Vec<_> = explanation
        .catalog
        .regions
        .iter()
        .filter(|r| {
            claims
                .iter()
                .any(|c| c.observation.region_ids.contains(&r.id))
        })
        .collect();
    let next = offset.saturating_add(claims.len());
    let mut result = crate::local_cmd::base_result("inspect.grounded");
    result["artifact"] = crate::local_cmd::reference(path)?;
    result["data"] = serde_json::json!({"source_sha256":explanation.catalog.source_sha256,"catalog_sha256":explanation.catalog_sha256,"claims":claims,"facts":facts,"regions":regions,"limits":explanation.limits});
    result["page"] = serde_json::json!({"total":explanation.claims.len(),"shown":claims.len(),"omitted":explanation.claims.len().saturating_sub(claims.len()),"next_cursor":(next<explanation.claims.len()).then(||next.to_string())});
    crate::local_cmd::bounded(result, 4096)
}
pub(crate) fn run(args: Args) -> Result<u8, CliError> {
    let explanation = explanation(&args.report, args.proposals.as_deref())?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.out)
        .map_err(|e| CliError::io(e.to_string()))?;
    let linked = saccade_core::report_links::decorate(&serde_json::to_value(&explanation)?)?;
    serde_json::to_writer_pretty(file, &linked)?;
    saccade_core::report_links::index(&args.out, &linked)?;
    if args.json {
        crate::emit(&format!("{}\n", serde_json::to_string(&explanation)?))?;
    } else {
        crate::emit(&format!(
            "grounded explanation: {} supported numerical observations; {} proposals dropped; semantic and causal claims unproven\n",
            explanation.claims.len(),
            explanation.dropped.len()
        ))?;
    }
    Ok(0)
}
