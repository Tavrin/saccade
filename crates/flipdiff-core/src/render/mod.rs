//! Presentations of a [`Report`]: the self-contained HTML report and the
//! Markdown summary used for CI step summaries and PR comments.
//!
//! Both read only the [`Report`] model (see `SPEC.md` §3).

mod html;
mod markdown;

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
///
/// The page is fully self-contained: CSS, JS and the report JSON are inline,
/// and images are referenced by the report's relative paths, so the directory
/// works when opened from `file://`.
pub fn render_html(report: &Report, report_dir: &Path) -> Result<PathBuf> {
    html::render_html(report, report_dir)
}

/// Renders the Markdown summary of `report`.
///
/// The output starts with the hidden `<!-- flipdiff-summary -->` marker and is
/// never longer than `opts.max_bytes` (default 60 000): pass rows are dropped
/// first, then non-pass rows, with a "…and N more" line recording the cut.
pub fn render_markdown(report: &Report, opts: &MarkdownOptions) -> String {
    markdown::render_markdown(report, opts)
}
