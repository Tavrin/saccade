//! Grouped declared-case page using the report/view design system.
use super::html::escape;
use crate::{
    Result,
    coverage::{Measurement, Report},
};
use std::path::{Path, PathBuf};

fn report_cell(m: &Measurement, manifest: &Path, out: &Path) -> String {
    let label = escape(&format!(
        "{}{}{}",
        m.state,
        m.verdict
            .as_ref()
            .map_or_else(String::new, |v| format!(" / {v}")),
        m.value.map_or_else(String::new, |v| format!(" ({v:.6})"))
    ));
    let Some(path) = &m.report_path else {
        return label;
    };
    let resolved = crate::paths::resolve(path, manifest);
    // Link to the existing pair viewer only when the adjacent HTML exists.
    let page = resolved
        .parent()
        .unwrap_or(Path::new("."))
        .join("index.html");
    let target = if page.is_file() { page } else { resolved };
    let relative = crate::paths::record(&target, out, false);
    // All paths are local. Encode URL punctuation; even a colon-bearing filename
    // must never become an executable URL scheme.
    let url: String = relative
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("<a href=\"./{}\">{label}</a>", escape(&url))
}
/// Write the grouped coverage, variants and health page without filtering absences.
pub(crate) fn render(report: &Report, manifest: &Path, out: &Path) -> Result<PathBuf> {
    let mut groups = String::new();
    let rows_by_id: std::collections::BTreeMap<_, _> = report
        .rows
        .iter()
        .map(|r| (r.case.case_id.as_str(), r))
        .collect();
    for group in &report.groups {
        let name = group
            .variants
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(" / ");
        groups.push_str(&format!("<section><h3>{}</h3><p>{} expected; {} captured; {} measured; {} refused.</p><table><thead><tr><th>Case / declared variants</th><th>Baseline</th><th>Capture</th><th>Coverage</th><th>Latest vs baseline</th><th>Latest vs approved anchor (drift)</th><th>Latest vs last-good (continuity)</th><th>Reference health</th></tr></thead><tbody>",escape(&name),group.counts.expected,group.counts.captured,group.counts.measured,group.counts.refused));
        for id in &group.case_ids {
            let row = rows_by_id.get(id.as_str()).ok_or_else(|| {
                crate::Error::Config(format!("coverage group names undeclared case {id}"))
            })?;
            let variants = row
                .case
                .variants
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("; ");
            let health = if row.health.is_empty() {
                "no findings".into()
            } else {
                row.health.join("; ")
            };
            groups.push_str(&format!("<tr><th scope=\"row\">{}{}<br>{}</th><td>{}</td><td>{}</td><td><strong>{}</strong>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",escape(id),if row.case.required { " (required)" } else { " (advisory)" },escape(&variants),escape(&row.baseline_state),escape(&row.capture_state),escape(&row.coverage),row.case.refusal.as_ref().map_or_else(String::new,|r|format!("<br>{}",escape(r))),report_cell(&row.latest,manifest,out),report_cell(&row.approved_anchor,manifest,out),report_cell(&row.last_good,manifest,out),escape(&health)));
        }
        groups.push_str("</tbody></table></section>");
    }
    let health_rows = report
        .rows
        .iter()
        .filter(|r| !r.health.is_empty())
        .map(|r| {
            format!(
                "<tr><th scope=\"row\">{}</th><td>{}</td></tr>",
                escape(&r.case.case_id),
                escape(&r.health.join("; "))
            )
        })
        .collect::<String>();
    let css = super::shared::page_css(&[include_str!("../../assets/report.css")]);
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>saccade coverage and reference health</title><style>{css}</style></head><body><main><h1>Coverage and reference health</h1><nav aria-label=\"Views\"><a href=\"#coverage\">Coverage</a> · <a href=\"#variants\">Declared variants</a> · <a href=\"#health\">Baseline health</a> · <a href=\"coverage.json\">Full JSON</a></nav><section id=\"coverage\"><h2>Coverage: {}</h2><p>{} expected; {} captured; {} measured; {} refused. Every declared case remains visible, including absence on both sides.</p><p>Completeness covers declared cases only. Measured failures count as measured coverage. Passing and last-good never establish human approval. Theme/source presence, rendering and contrast remain separate findings.</p></section><section id=\"variants\"><h2>Declared variants</h2>{groups}</section><section id=\"health\"><h2>Baseline health</h2><p>Observation time: {}. Run and known approval age limit: {} seconds. Missing approval metadata means no declared approval, not proof of historical non-approval.</p><table><thead><tr><th>Case</th><th>Findings</th></tr></thead><tbody>{health_rows}</tbody></table></section></main></body></html>",
        escape(&report.coverage),
        report.counts.expected,
        report.counts.captured,
        report.counts.measured,
        report.counts.refused,
        report.now_unix,
        report.max_age_seconds
    );
    let path = out.join("index.html");
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        return Err(crate::Error::NotEmptyOutDir(
            "coverage HTML output is not a regular file".into(),
        ));
    }
    let mut temp = tempfile::NamedTempFile::new_in(out)
        .map_err(crate::run::io_err("writing coverage HTML".into()))?;
    std::io::Write::write_all(&mut temp, html.as_bytes())
        .map_err(crate::run::io_err("writing coverage HTML".into()))?;
    temp.persist(&path)
        .map_err(|e| crate::Error::Config(format!("writing coverage HTML: {}", e.error)))?;
    Ok(path)
}
