//! Standalone local vision boundaries for wave 7; observations never confer approval.
pub mod faces;
#[cfg(all(test, feature = "local-models"))]
mod heavy_tests;
#[cfg(feature = "local-vlm")]
pub mod local_vlm;
pub mod models;
pub mod observation;
#[cfg(feature = "vision-providers")]
pub mod providers;
pub mod quality;
#[cfg(feature = "local-models")]
pub mod runtime;
#[cfg(feature = "schema")]
pub mod schemas;
pub mod vision;
pub mod watermark;
