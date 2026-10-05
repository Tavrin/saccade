#![doc = include_str!("../README.md")]

#[cfg(feature = "prechecks")]
pub mod a11y;
pub mod ablate;
pub mod bisect;
pub mod brand;
#[cfg(feature = "ai")]
pub mod budget_ledger;
pub mod buffer;
#[path = "safety/color.rs"]
pub mod color;
pub mod compare;
pub mod config;
pub mod decision;
#[cfg(feature = "ai")]
pub mod decision_provider;
pub mod diagnostics;
pub mod ergonomics;
pub mod error;
pub mod evidence;
pub mod exclusions;
pub mod explain;
#[cfg(feature = "geometry")]
pub mod geometry;
pub mod gpu_clock;
pub mod grounded;
pub mod hdr;
pub mod hotspots;
pub mod inbox;
pub mod intent;
pub mod inventory;
#[cfg(feature = "ai")]
pub mod judge;
#[cfg(feature = "evaluation")]
pub mod judge_bench;
#[cfg(feature = "ai")]
pub mod judge_canary;
#[cfg(feature = "ai")]
pub mod judge_evidence;
#[cfg(feature = "ai")]
pub mod judge_provider;
pub mod judge_stats;
pub mod judge_vote;
pub mod labels;
pub mod local;
pub mod localized;
mod machado;
pub mod meta;
pub mod object_ids;
pub mod onset;
pub mod paths;
pub mod perf;
pub mod properties;
#[cfg(feature = "compression")]
pub mod quality;
pub mod questions;
pub mod rank;
pub mod regions;
pub mod render;
pub mod renderdoc;
pub mod report;
#[cfg(feature = "ai")]
pub mod review;
pub mod run;
pub mod runs;
#[cfg(feature = "prechecks")]
pub mod safety;
pub mod semantic;
pub mod sequence;
#[cfg(feature = "workbench")]
pub mod serve;
pub mod snapshot;
pub mod ui_review;
pub mod view;

pub use error::{Error, Result};
pub use report::{Entry, EntryPaths, Metric, Metrics, Properties, Report, Status, Totals};

/// Cargo features compiled into this library. Computational modules are optional;
/// persisted report and decision types remain available without their producers.
pub const COMPILED_FEATURES: &[&str] = &[
    #[cfg(feature = "parallel")]
    "parallel",
    #[cfg(feature = "graphics")]
    "graphics",
    #[cfg(feature = "geometry")]
    "geometry",
    #[cfg(feature = "compression")]
    "compression",
    #[cfg(feature = "semantic-regions")]
    "semantic-regions",
    #[cfg(feature = "ai")]
    "ai",
    #[cfg(feature = "workbench")]
    "workbench",
    #[cfg(feature = "evaluation")]
    "evaluation",
    #[cfg(feature = "prechecks")]
    "prechecks",
    #[cfg(feature = "schema")]
    "schema",
];

/// Shared transport root authorization.
pub mod root_policy;

/// All transitive declared roots, available to review without the evaluator feature.
#[cfg(feature = "ai")]
pub fn judge_bench_sources(case: &evidence::case::EvidenceCase) -> Vec<String> {
    if case.provenance.source_roots.is_empty()
        || case
            .inputs
            .iter()
            .any(|i| i.provenance.source_roots.is_empty())
    {
        return vec![];
    }
    case.provenance
        .source_roots
        .iter()
        .chain(case.inputs.iter().flat_map(|i| &i.provenance.source_roots))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
