//! The self-contained HTML report.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::report::Report;

const TEMPLATE: &str = include_str!("../../assets/report.html");
const CSS: &str = include_str!("../../assets/report.css");
const JS: &str = include_str!("../../assets/report.js");

/// Serializes the report for embedding in a `<script type="application/json">`.
///
/// `</` becomes `<\/` (SPEC §5) and `<!--` becomes `<!--`, so the payload
/// can neither close the script element nor enter the "script data escaped"
/// state. Both are valid JSON string escapes and decode to the original text.
pub(crate) fn embed_json(report: &Report) -> Result<String> {
    let json = serde_json::to_string(report)?;
    Ok(json.replace("</", "<\\/").replace("<!--", "<\\u0021--"))
}

pub(crate) fn build_html(report: &Report) -> Result<String> {
    let data = embed_json(report)?;
    // The user-controlled payload is substituted last so that nothing in it can
    // be mistaken for another placeholder.
    Ok(TEMPLATE
        .replace("/*__FLIPDIFF_CSS__*/", CSS)
        .replace("/*__FLIPDIFF_JS__*/", JS)
        .replace("__FLIPDIFF_DATA__", &data))
}

pub(crate) fn render_html(report: &Report, report_dir: &Path) -> Result<PathBuf> {
    let html = build_html(report)?;
    let path = report_dir.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    Ok(path)
}
