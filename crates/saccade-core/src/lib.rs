#![doc = include_str!("../README.md")]

#[cfg(feature = "prechecks")]
pub mod a11y;
pub mod ablate;
/// Versioned producer fingerprints and strict arm validation.
pub mod arms;
pub mod asset_views;
// G26
#[cfg(feature = "assist")]
pub mod assist;
pub mod bisect;
pub mod brand;
#[cfg(feature = "ai")]
pub mod budget_ledger;
pub mod buffer;
pub mod captured_sequence;
#[path = "safety/color.rs"]
pub mod color;
pub mod compare;
pub mod config;
pub mod decision;
#[cfg(feature = "ai")]
pub mod decision_provider;
pub mod dense_motion;
pub mod diagnostics;
pub mod ergonomics;
pub mod error;
pub mod evidence;
pub mod exclusions;
pub mod explain;
pub mod general;
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
pub mod paired_stats;
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
    // O12/O17
    #[cfg(feature = "text-quality")]
    "text-quality",
    #[cfg(feature = "assist")]
    "assist",
    #[cfg(feature = "documents")]
    "documents",
    #[cfg(feature = "credentials")]
    "credentials",
    #[cfg(feature = "ocr")]
    "ocr",
    #[cfg(feature = "embeddings")]
    "embeddings",
    // wave7
    #[cfg(feature = "local-models")]
    "local-models",
    #[cfg(feature = "local-vlm")]
    "local-vlm",
    #[cfg(feature = "vision-providers")]
    "vision-providers",
    #[cfg(feature = "media-http")]
    "media-http",
    #[cfg(feature = "parallel")]
    "parallel",
    #[cfg(feature = "graphics")]
    "graphics",
    #[cfg(feature = "geometry")]
    "geometry",
    #[cfg(feature = "dense-motion")]
    "dense-motion",
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

// wave7
/// Standalone local vision, provenance and crop-safety interfaces.
pub mod wave7;

// wave9
pub mod evidence_quality;

// wave8
/// Versioned media analysis, saliency, usage and search primitives.
pub mod media;

// wave10
pub mod schema_catalog;

pub mod optional;

// wave11
pub mod ablation_timing;
/// Declared coverage, variant grouping and baseline health.
pub mod coverage;
pub mod manifest;
pub mod report_links;
pub mod settling;
pub mod timing;

/// Stable shared mask-spec parser for CLI, MCP and downstream measurement APIs.
pub mod mask_spec;

/// Deterministic overlap and boundary metrics between integer label images.
pub mod mask_metrics;

/// COCO and YOLO bounding-box interchange with explicit coordinate conventions.
pub mod boxes;

/// Frame-index, timestamp and file contract for externally extracted video frames.
pub mod frame_map;

/// One operator-owned model and runtime configuration shared by every surface.
pub mod model_config;

// O12/O17
/// Required region/string gates independent of global image averages.
pub mod critical_text;
pub mod text_quality;

// laneD
#[cfg(any(unix, windows))]
pub mod batch;
/// Timed subtitle/caption evidence against presentation-timestamped frames.
pub mod timed_text;
