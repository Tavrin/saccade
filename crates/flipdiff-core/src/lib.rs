//! Perceptual image comparison (NVIDIA FLIP) and the report model behind the
//! `flipdiff` visual-regression CLI.
//!
//! See `docs/design.md` for the frozen contracts.

pub mod bisect;
pub mod buffer;
pub mod compare;
pub mod config;
pub mod decision;
pub mod diagnostics;
pub mod error;
pub mod explain;
pub mod hdr;
pub mod hotspots;
pub mod inbox;
pub mod judge;
pub mod judge_canary;
pub mod judge_evidence;
pub mod judge_provider;
pub mod judge_stats;
pub mod judge_vote;
pub mod meta;
pub mod properties;
pub mod rank;
pub mod regions;
pub mod render;
pub mod report;
pub mod run;
pub mod runs;
pub mod sequence;
pub mod serve;
pub mod snapshot;
pub mod view;
pub mod watch;

pub use error::{Error, Result};
pub use report::{Entry, EntryPaths, Metric, Metrics, Properties, Report, Status, Totals};
