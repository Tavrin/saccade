//! First-party accessibility policy extension. Pixel pre-checks, never certification.
pub mod a11y;
pub mod auto;
mod thresholds;
pub use auto::{Options, Report, run};
/// Typed input, configuration and artifact error from the shared measurement layer.
pub use saccade_core::{Error, Result};
fn io_err(context: String) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io { context, source }
}
fn compile_glob(pattern: &str) -> Result<globset::GlobMatcher> {
    globset::Glob::new(pattern)
        .map(|g| g.compile_matcher())
        .map_err(|e| Error::Config(format!("region glob: {e}")))
}
