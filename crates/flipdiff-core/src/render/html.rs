//! The self-contained HTML report.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::report::Report;

const TEMPLATE: &str = include_str!("../../assets/report.html");
const CSS: &str = include_str!("../../assets/report.css");
const JS: &str = include_str!("../../assets/report.js");
/// Hash state, `window.flipdiff` and the proposed-decision chip, shared with the viewer.
pub(crate) const AGENT_JS: &str = include_str!("../../assets/agentapi.js");
pub(crate) const AGENT_CSS: &str = include_str!("../../assets/agentapi.css");

/// Serializes the report for embedding in a `<script type="application/json">`.
///
/// `</` becomes `<\/` (see `docs/design.md`) and `<!--` becomes `<!--`, so the payload
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
        .replace("/*__FLIPDIFF_CSS__*/", &format!("{CSS}\n{AGENT_CSS}"))
        .replace("/*__FLIPDIFF_JS__*/", &format!("{AGENT_JS}\n{JS}"))
        .replace("__FLIPDIFF_DATA__", &data))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn render_sequence_html(
    report: &Report,
    sequence: &crate::sequence::SequenceReport,
    out: &Path,
) -> Result<PathBuf> {
    let count = sequence.mean_flip_curve.len().max(2) - 1;
    let mut curves = String::new();
    let mut points = String::new();
    for (i, value) in sequence.mean_flip_curve.iter().enumerate() {
        if let Some(value) = value {
            points.push_str(&format!(
                "{:.2},{:.2} ",
                40.0 + i as f64 / count as f64 * 720.0,
                180.0 - value.clamp(0.0, 1.0) * 160.0
            ));
        } else if !points.is_empty() {
            curves.push_str(&format!("<polyline points=\"{points}\"/>"));
            points.clear();
        }
    }
    if !points.is_empty() {
        curves.push_str(&format!("<polyline points=\"{points}\"/>"));
    }
    let block = format!(
        "<section aria-label=\"Frame sequence\"><h2>Frame sequence</h2><pre>{}</pre><svg viewBox=\"0 0 800 220\" role=\"img\" aria-label=\"Mean FLIP by sorted frame index; vertical scale zero to one\" style=\"width:100%;max-width:1000px\"><path d=\"M40 20 V180 H760\" fill=\"none\" stroke=\"currentColor\"/><g fill=\"none\" stroke=\"#c05cff\" stroke-width=\"2\">{curves}</g><g fill=\"currentColor\" font-size=\"14\"><text x=\"12\" y=\"26\">1</text><text x=\"12\" y=\"184\">0</text><text x=\"40\" y=\"210\">Sorted frame index (mean FLIP)</text></g></svg></section>",
        escape(&sequence.text())
    );
    let html = build_html(report)?.replacen("<main>", &format!("<main>{block}"), 1);
    let path = out.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    Ok(path)
}

pub(crate) fn render_rank_html(rank: &crate::rank::RankReport, out: &Path) -> Result<PathBuf> {
    let number = |v: Option<f64>| v.map_or_else(|| "—".into(), |v| format!("{v:.6}"));
    let mut rows = String::new();
    for c in &rank.overall {
        rows.push_str(&format!(
            "<tr><td>{}</td><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            c.rank.map_or_else(|| "—".into(), |v| v.to_string()),
            escape(&c.report_html),
            escape(&c.label),
            number(c.mean_rank),
            number(c.mean_metric),
            c.bit_identical
        ));
    }
    let mut image_rows = String::new();
    for image in &rank.images {
        for c in &image.candidates {
            image_rows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:?}</td></tr>",
                escape(&image.name),
                escape(&c.label),
                c.rank.map_or_else(|| "—".into(), |v| v.to_string()),
                number(c.value),
                c.status
            ));
        }
    }
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>flipdiff ranking</title><style>{CSS}</style></head><body><main><section><h1>flipdiff ranking</h1><p>Metric: {:?}. Verdict: {}. {} common / {} reference images. Ties use competition ranks; incomplete comparisons have no overall winner.</p><table><thead><tr><th>Rank</th><th>Candidate report</th><th>Mean rank</th><th>Mean metric</th><th>Bit-identical</th></tr></thead><tbody>{rows}</tbody></table><h2>Per-image rankings</h2><table><thead><tr><th>Image</th><th>Candidate</th><th>Rank</th><th>Value</th><th>Status</th></tr></thead><tbody>{image_rows}</tbody></table><p><a href=\"ranking.md\">Markdown tables</a></p></section></main></body></html>",
        rank.metric,
        escape(&rank.verdict),
        rank.common_images,
        rank.reference_images
    );
    let path = out.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    Ok(path)
}

pub(crate) fn render_html(report: &Report, report_dir: &Path) -> Result<PathBuf> {
    let html = build_html(report)?;
    let path = report_dir.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    crate::decision::ensure_sidecar(report_dir);
    Ok(path)
}
