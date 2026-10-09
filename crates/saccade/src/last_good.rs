//! CLI error adapter for accepted history snapshots.
use crate::agent::CliError;
use std::path::Path;
pub(crate) fn resolve(store: &Path) -> Result<tempfile::TempDir, CliError> {
    Ok(saccade_core::workflows::last_good::resolve(store)?)
}
#[cfg(all(feature = "products", feature = "mcp"))]
pub(crate) fn resolve_checked(
    store: &Path,
    check: impl Fn(&Path) -> Result<(), CliError>,
) -> Result<tempfile::TempDir, CliError> {
    Ok(saccade_core::workflows::last_good::resolve_checked(
        store,
        |p| check(p).map_err(|e| saccade_core::workflows::CommandError::new(e.code, e.message)),
    )?)
}
