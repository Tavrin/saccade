//! Local snapshot rendering shared by inspect/export and MCP evidence.
use crate::agent::CliError;
use saccade_core::snapshot::{ViewState, snapshot as draw, write_frames};
use std::path::{Path, PathBuf};
/// What a snapshot wrote.
pub struct Snap {
    /// The PNG files, one per frame.
    pub paths: Vec<PathBuf>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// Draws `target` in state `state` and writes the PNG(s) to `out`.
pub fn render_snapshot(
    target: &Path,
    entry: Option<&str>,
    state: Option<&str>,
    width: u32,
    out: &Path,
) -> Result<Snap, CliError> {
    if !target.is_dir() && !target.is_file() {
        return Err(CliError::io(format!("{} does not exist", target.display())));
    }
    let st = ViewState::parse(state.unwrap_or(""));
    let frames = draw(target, entry, &st, width)?;
    let (width, height) = frames.first().map_or((0, 0), |f| f.dimensions());
    let paths = write_frames(&frames, out)?;
    Ok(Snap {
        paths,
        width,
        height,
    })
}
