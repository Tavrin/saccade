//! Perceptual image comparison (NVIDIA FLIP) and the report model behind the
//! `flipdiff` visual-regression CLI.
//!
//! See `SPEC.md` at the repository root for the frozen contracts.

pub mod compare;
pub mod config;
pub mod error;
pub mod properties;
pub mod render;
pub mod report;
pub mod run;
pub mod view;

pub use error::{Error, Result};
pub use report::{Entry, EntryPaths, Metric, Metrics, Properties, Report, Status, Totals};
