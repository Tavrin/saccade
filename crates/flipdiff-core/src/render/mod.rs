//! Presentations of a [`Report`]: the self-contained HTML report and the
//! Markdown summary used for CI step summaries and PR comments.
//!
//! Both read only the [`Report`] model (see `SPEC.md` §3).
//!
//! SCAFFOLD STUB — lane B replaces the bodies; the signatures are frozen.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::report::Report;

/// Options for [`render_markdown`].
#[derive(Debug, Clone, Default)]
pub struct MarkdownOptions {
    /// Link to the uploaded report artifact, shown in the summary when set.
    pub artifact_url: Option<String>,
    /// Hard cap on output length in bytes (GitHub comments cap at 65 536).
    /// `None` means 60 000.
    pub max_bytes: Option<usize>,
}

/// Writes `index.html` into `report_dir` (which already holds the report JSON
/// and the images it references) and returns its path.
pub fn render_html(report: &Report, report_dir: &Path) -> Result<PathBuf> {
    let _ = report;
    Ok(report_dir.join("index.html"))
}

/// Renders the Markdown summary of `report`.
pub fn render_markdown(report: &Report, opts: &MarkdownOptions) -> String {
    let _ = (report, opts);
    String::new()
}
