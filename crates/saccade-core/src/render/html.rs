//! The self-contained HTML report.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::report::Report;

const TEMPLATE: &str = include_str!("../../assets/report.html");
const CSS: &str = include_str!("../../assets/report.css");
const JS: &str = include_str!("../../assets/report.js");
/// Hash state, `window.saccade` and the proposed-decision chip, shared with the viewer.
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

pub(crate) fn build_html(
    report: &Report,
    case: Option<&crate::evidence::case::EvidenceCase>,
) -> Result<String> {
    let data = embed_json(report)?;
    // Partition the template before inserting either user-controlled payload.
    // A literal placeholder in an intent/name must never trigger substitution.
    let template = TEMPLATE
        .replace(
            "/*__SACCADE_CSS__*/",
            &super::shared::page_css(&[AGENT_CSS, CSS]),
        )
        .replace(
            "/*__SACCADE_JS__*/",
            &super::shared::page_js(&[AGENT_JS, JS]),
        );
    let (before, tail) = template
        .split_once("__SACCADE_SUMMARY__")
        .ok_or_else(|| Error::Config("report template lacks summary slot".into()))?;
    let (middle, after) = tail
        .split_once("__SACCADE_DATA__")
        .ok_or_else(|| Error::Config("report template lacks data slot".into()))?;
    let motion = report
        .entries
        .iter()
        .filter_map(|e| {
            e.diagnostics
                .as_ref()
                .and_then(|d| d.motion.as_ref())
                .map(|m| (e, m))
        })
        .map(|(e, m)| {
            Ok(format!(
                "<h3>{}</h3><pre>{}</pre>",
                escape(&e.name),
                escape(&serde_json::to_string_pretty(m)?)
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join("");
    let summary = format!(
        "{}<details><summary>Exclusion audit</summary><pre>{}</pre></details>",
        super::bundle::summary(report, case)?,
        escape(&crate::exclusions::text(report))
    );
    let summary = if let Some(check) = &report.config.meta.arm_validation {
        format!(
            "<p><strong>Arm comparison mode:</strong> {}. Unmapped: {} keys; outcomes: {} keys.</p><p><strong>Allowed unreached (intentionally unconverged):</strong> {}</p>{summary}",
            escape(&serde_json::to_string(&check.compare)?),
            check.unmapped.count,
            check.outcomes.count,
            escape(&serde_json::to_string(&check.allowed_unreached)?)
        )
    } else {
        summary
    };
    let summary = format!(
        "{summary}<details><summary>Motion diagnostics (raw FLIP remains authoritative)</summary>{motion}</details>"
    );
    let mut scope_notice = String::new();
    let mut id_tables = String::new();
    for entry in &report.entries {
        if let Some(field) = &entry.field_evidence {
            scope_notice.push_str(&format!(
                "<p><strong>Scope: {}</strong> — {} ({})</p>",
                escape(&field.scope),
                escape(&entry.name),
                escape(&field.class)
            ));
            if !field.per_id.is_empty() {
                id_tables.push_str(&format!("<h3>Per-ID shifts: {}</h3><p>{} IDs measured; top {} shown. {}</p><table><thead><tr><th>ID / name</th><th>Pixels</th><th>Unit</th><th>Signed mean</th><th>Mean absolute</th><th>p95</th><th>Threshold</th><th>% over</th><th>Contribution %</th><th>Relative depth mean / p95</th></tr></thead><tbody>",escape(&entry.name),field.ids_measured,field.per_id.len(),escape(&field.provenance.join("; "))));
                for row in &field.per_id {
                    id_tables.push_str(&format!("<tr><td>{}: {}</td><td>{}</td><td>{}</td><td>{:+.6}</td><td>{:.6}</td><td>{:.6}</td><td>{:.6}</td><td>{:.2}</td><td>{:.2}</td><td>{}</td></tr>",row.id,escape(&row.name),row.pixels,escape(&row.unit),row.shift.signed_mean_shift,row.shift.mean_absolute,row.shift.p95,row.shift.threshold,100.0*row.shift.share_over_threshold,100.0*row.contribution_share,row.relative.as_ref().map_or_else(||"—".into(),|r|format!("{:.6} / {:.6}",r.mean_absolute,r.p95))));
                }
                id_tables.push_str("</tbody></table>");
                for row in &field.per_id {
                    if let Some(crop) = &row.crop
                        && crop.starts_with("regions/")
                        && !crop.contains("..")
                    {
                        id_tables.push_str(&format!("<figure><figcaption>Top ID {}: {} — base | candidate | error (crops downsampled to at most 512 px per panel)</figcaption><img loading=\"lazy\" style=\"max-width:100%\" src=\"{}\" alt=\"ID comparison crop\"></figure>",row.id,escape(&row.name),escape(crop)));
                    }
                }
            }
            if let Some(noise) = &field.noise {
                id_tables.push_str(&format!("<p>Repeat noise: {}; signed mean {:+.6}; RMS {:.6}; beyond-envelope tiles {}. {}</p>",escape(&noise.class),noise.signed_mean,noise.rms,noise.beyond_tiles.len(),escape(&noise.method)));
            }
        }
    }
    let summary = format!("{scope_notice}{summary}{id_tables}");
    let mut gallery = String::new();
    for entry in &report.entries {
        if entry.gallery.is_empty() {
            continue;
        }
        gallery.push_str(&format!("<h3>{}</h3>", escape(&entry.name)));
        for region in &entry.gallery {
            if !region.strip.starts_with("regions/")
                || region.strip.contains("..")
                || !region.zoom_2x.starts_with("regions/")
                || region.zoom_2x.contains("..")
            {
                continue;
            }
            gallery.push_str(&format!("<figure><figcaption>{:?}: {} | FLIP mean {:.5}, max {:.5}; signed luminance {:+.5}; detail {:?}; gap {:?}. Base | candidate | heatmap</figcaption><img loading=\"lazy\" style=\"max-width:100%\" src=\"{}\" alt=\"Region comparison strip\"><details><summary>2× nearest-neighbour zoom</summary><img loading=\"lazy\" style=\"image-rendering:pixelated;max-width:100%\" src=\"{}\" alt=\"Two times crop zoom\"></details></figure>", region.rect_px,escape(&region.reasons.join(", ")),region.mean_flip,region.max_flip,region.signed_shift,region.detail_energy_ratio,region.gap_change,escape(&region.strip),escape(&region.zoom_2x)));
        }
    }
    let summary = format!(
        "{summary}<details><summary>Automatic regions of interest</summary>{gallery}</details>"
    );
    let (head, tail) = before
        .split_once("__SACCADE_META__")
        .ok_or_else(|| Error::Config("report template lacks metadata slot".into()))?;
    let metadata = header_metadata(report);
    Ok(format!(
        "{head}{metadata}{tail}{summary}{middle}{data}{after}"
    ))
}

fn header_metadata(report: &Report) -> String {
    let cfg = &report.config;
    let mut fields = vec![
        ("version", format!("v{}", report.tool_version)),
        (
            "mode",
            format!(
                "{:?} ({} vs {})",
                cfg.mode, cfg.labels.baseline, cfg.labels.capture
            ),
        ),
    ];
    if cfg.mode == crate::report::Mode::Identity {
        fields.push((
            "proof",
            "Exact native decoded samples, dimensions, channel interpretation and sample type"
                .into(),
        ));
        fields.push(("scope", format!("Supplied captures: {}; selection: {}; exclusions: {}. Capture comparability and performance remain separate.",
            report.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", "),
            if cfg.entries.is_empty() { "all names".into() } else { cfg.entries.join(", ") },
            if cfg.ignore.is_empty() { "none".into() } else { cfg.ignore.join(", ") })));
    } else {
        fields.extend([
            ("threshold", cfg.default_threshold.to_string()),
            ("metric", format!("{:?}", cfg.default_metric)),
            ("ppd", cfg.pixels_per_degree.to_string()),
        ]);
    }
    fields
        .into_iter()
        .map(|(key, value)| format!("<div><dt>{key}</dt><dd>{}</dd></div>", escape(&value)))
        .collect()
}

pub(super) fn escape(text: &str) -> String {
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
    write_pixel_data(report, out)?;
    let case = super::bundle::prepare(report, out)?;
    let html = build_html(report, case.as_ref())?.replacen("<main>", &format!("<main>{block}"), 1);
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
    let css = super::shared::page_css(&[CSS]);
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>saccade ranking</title><style>{css}</style></head><body><main><section><h1>saccade ranking</h1><p>Metric: {:?}. Verdict: {}. {} common / {} reference images. Ties use competition ranks; incomplete comparisons have no overall winner.</p><table><thead><tr><th>Rank</th><th>Candidate report</th><th>Mean rank</th><th>Mean metric</th><th>Bit-identical</th></tr></thead><tbody>{rows}</tbody></table><h2>Per-image rankings</h2><table><thead><tr><th>Image</th><th>Candidate</th><th>Rank</th><th>Value</th><th>Status</th></tr></thead><tbody>{image_rows}</tbody></table><p><a href=\"ranking.md\">Markdown tables</a></p></section></main></body></html>",
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

fn write_pixel_data(report: &Report, report_dir: &Path) -> Result<()> {
    let mut data = serde_json::Map::new();
    for e in &report.entries {
        let paths = [&e.paths.baseline, &e.paths.capture, &e.paths.heatmap];
        let uris: Vec<Option<String>> = paths
            .iter()
            .map(|path| {
                path.as_ref()
                    .filter(|p| report_dir.join(p).is_file())
                    .cloned()
            })
            .collect();
        data.insert(e.name.clone(), serde_json::to_value(uris)?);
    }
    let json = serde_json::to_string(&data)?.replace("</", "<\\/");
    let path = report_dir.join("report-pixels.js");
    std::fs::write(&path, format!("window.__saccadeReportPixels={json};\n")).map_err(|source| {
        Error::Io {
            context: format!("writing {}", path.display()),
            source,
        }
    })
}

pub(crate) fn render_html(report: &Report, report_dir: &Path) -> Result<PathBuf> {
    let case = super::bundle::prepare(report, report_dir)?;
    let html = build_html(report, case.as_ref())?;
    write_pixel_data(report, report_dir)?;
    let path = report_dir.join("index.html");
    std::fs::write(&path, html).map_err(|source| Error::Io {
        context: format!("writing {}", path.display()),
        source,
    })?;
    crate::decision::ensure_sidecar(report_dir);
    Ok(path)
}
