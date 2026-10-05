//! Standalone local vision boundaries for wave 7; observations never confer approval.
#[cfg(feature = "local-vlm")]
pub mod local_vlm;
pub mod models;
pub mod observation;
pub mod quality;
#[cfg(feature = "local-models")]
pub mod runtime;
pub mod vision;
