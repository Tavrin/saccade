//! Perceptual image comparison (NVIDIA FLIP) and the report model behind the
//! `saccade` visual-regression CLI.
//!
//! See `docs/design.md` for the frozen contracts.

#[cfg(feature = "prechecks")]
pub mod a11y;
pub mod ablate;
pub mod bisect;
pub mod buffer;
pub mod compare;
pub mod config;
pub mod decision;
pub mod diagnostics;
pub mod ergonomics;
pub mod error;
pub mod evidence;
pub mod explain;
pub mod hdr;
pub mod hotspots;
pub mod inbox;
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
pub mod meta;
pub mod paths;
pub mod perf;
pub mod properties;
pub mod rank;
pub mod regions;
pub mod render;
pub mod report;
#[cfg(feature = "ai")]
pub mod review;
pub mod run;
pub mod runs;
#[cfg(feature = "prechecks")]
pub mod safety;
pub mod sequence;
#[cfg(feature = "workbench")]
pub mod serve;
pub mod snapshot;
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
