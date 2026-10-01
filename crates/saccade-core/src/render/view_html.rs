//! The self-contained review viewer page.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::view::ViewModel;

const TEMPLATE: &str = include_str!("../../assets/view.html");
const CSS: &str = include_str!("../../assets/view.css");
const JS: &str = include_str!("../../assets/view.js");

/// Serializes the model for a `<script type="application/json">`: `</` becomes
/// `<\/` and `<!--` becomes `<!--`, so the payload cannot close the
/// script element or enter the "script data escaped" state.
fn embed_json(model: &ViewModel) -> Result<String> {
    let json = serde_json::to_string(model)?;
    Ok(json.replace("</", "<\\/").replace("<!--", "<\\u0021--"))
}

pub(crate) fn write_view_html(model: &ViewModel, view_dir: &Path) -> Result<PathBuf> {
    let data = embed_json(model)?;
    // The payload is substituted last so nothing in it can pose as a placeholder.
    let html = TEMPLATE
        .replace(
            "/*__SACCADE_CSS__*/",
            &super::shared::page_css(&[super::html::AGENT_CSS, CSS]),
        )
        .replace(
            "/*__SACCADE_JS__*/",
            &super::shared::page_js(&[super::html::AGENT_JS, JS]),
        )
        .replace("__SACCADE_DATA__", &data);
    let path = view_dir.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    crate::decision::ensure_sidecar(view_dir);
    Ok(path)
}
