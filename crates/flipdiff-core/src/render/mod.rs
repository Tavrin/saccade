//! Presentations of a [`Report`]: the self-contained HTML report and the
//! Markdown summary used for CI step summaries and PR comments.
//!
//! Both read only the [`Report`] model (see `docs/design.md`).

mod html;
mod markdown;
pub(crate) mod shared;
mod view_html;

pub use markdown::identity_headline;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::report::Report;

/// Options for [`render_markdown`].
#[derive(Debug, Clone, Default)]
pub struct MarkdownOptions {
    /// Link to the uploaded report artifact, shown in the summary when set.
    pub artifact_url: Option<String>,
    /// Distinguishes sticky comments of several runs on one pull request
    /// (matrix jobs): the marker becomes `<!-- flipdiff-summary:<key> -->`.
    /// Callers must pass a key that [`is_valid_comment_key`] accepts.
    pub comment_key: Option<String>,
    /// Hard cap on output length in bytes (GitHub comments cap at 65 536).
    /// `None` means 60 000.
    pub max_bytes: Option<usize>,
}

/// Whether `key` is safe inside the marker comment: non-empty, at most 64
/// characters of ASCII letters, digits, `.`, `_` or `-`.
pub fn is_valid_comment_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
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

/// Writes a normal report with a server-rendered sequence curve.
pub fn render_sequence_html(
    report: &Report,
    sequence: &crate::sequence::SequenceReport,
    report_dir: &Path,
) -> Result<PathBuf> {
    html::render_sequence_html(report, sequence, report_dir)
}

/// Writes the server-rendered ranking tables and links to candidate reports.
pub fn render_rank_html(rank: &crate::rank::RankReport, report_dir: &Path) -> Result<PathBuf> {
    html::render_rank_html(rank, report_dir)
}

/// Renders the Markdown summary of `report`.
///
/// The output starts with the hidden `<!-- flipdiff-summary -->` marker (or
/// `<!-- flipdiff-summary:<key> -->` with `opts.comment_key`) and is
/// never longer than `opts.max_bytes` (default 60 000): pass rows are dropped
/// first, then non-pass rows, with a "…and N more" line recording the cut.
pub fn render_markdown(report: &Report, opts: &MarkdownOptions) -> String {
    markdown::render_markdown(report, opts)
}

/// Writes the review viewer's `index.html` into `view_dir` (which already
/// holds the images the model references) and returns its path.
pub(crate) fn write_view_html(model: &crate::view::ViewModel, view_dir: &Path) -> Result<PathBuf> {
    view_html::write_view_html(model, view_dir)
}
