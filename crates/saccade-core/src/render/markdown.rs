//! The Markdown summary (CI step summary / sticky PR comment).

use std::cmp::Ordering;

use super::MarkdownOptions;
use crate::report::{Entry, Metric, Mode, Report, Status};

const DEFAULT_MAX_BYTES: usize = 60_000;
const TABLE_HEAD: &str =
    "| Status | Image | Metric | Value | Threshold |\n|---|---|---|---:|---:|\n";

/// Formats `v` with 4 significant digits and no trimming, so a column of
/// values has one shape (`0.01000`, `0.5000`, `123.5`); zero is `0.0000`.
fn sig4(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v == 0.0 {
        return "0.0000".to_string();
    }
    let magnitude = v.abs().log10().floor() as i32;
    let decimals = (3 - magnitude).clamp(0, 15) as usize;
    format!("{v:.decimals$}")
}

fn metric_name(m: Metric) -> &'static str {
    match m {
        Metric::Mean => "mean",
        Metric::P95 => "p95",
        Metric::P99 => "p99",
        Metric::Max => "max",
    }
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Pass => "✅ pass",
        Status::Fail => "❌ fail",
        Status::New => "🆕 new",
        Status::Missing => "❓ missing",
        Status::Error => "⚠️ error",
    }
}

fn status_rank(s: Status) -> u8 {
    match s {
        Status::Fail => 0,
        Status::Error => 1,
        Status::Missing => 2,
        Status::New => 3,
        Status::Pass => 4,
    }
}

/// The hidden first line that identifies the sticky comment.
pub(crate) fn marker(comment_key: Option<&str>) -> String {
    match comment_key.filter(|k| !k.is_empty()) {
        Some(k) => format!("<!-- saccade-summary:{k} -->"),
        None => "<!-- saccade-summary -->".to_string(),
    }
}

/// A name as an inline code span that cannot break out of a table cell.
fn code_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    // A backtick inside a span would end it; swap for a lookalike.
    let cleaned = cleaned.replace('`', "ˋ").replace('|', "\\|");
    format!("`{cleaned}`")
}

/// Plain text made safe for one Markdown line (sidecar keys can reach it).
fn md_text(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .replace('|', "\\|")
        .replace('`', "ˋ")
}

fn row(e: &Entry) -> String {
    let value = e.value.map_or_else(|| "—".to_string(), sig4);
    let threshold = if e.value.is_some() {
        sig4(e.threshold)
    } else {
        "—".to_string()
    };
    let mut name = code_name(&e.name);
    if let Some(msg) = &e.error {
        let one_line: String = msg
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(160)
            .collect();
        name.push_str(&format!("<br>{}", code_name(&one_line)));
    }
    format!(
        "| {} | {} | {} | {} | {} |\n",
        status_label(e.status),
        name,
        e.buffer.as_ref().map_or_else(
            || metric_name(e.metric_used).to_string(),
            |b| format!("{} ({})", metric_name(b.metric), b.unit)
        ),
        value,
        threshold
    )
}

/// Rows for the failing regions of `e`, named `image › region`.
fn region_rows(e: &Entry) -> String {
    e.regions
        .iter()
        .filter(|r| r.status == Some(Status::Fail))
        .map(|r| {
            format!(
                "| {} | {} | {} | {} | {} |\n",
                status_label(Status::Fail),
                code_name(&format!("{} › {}", e.name, r.name)),
                metric_name(r.metric_used),
                sig4(r.value),
                r.threshold.map_or_else(|| "—".to_string(), sig4)
            )
        })
        .collect()
}

/// The identity-mode headline, for example `identity: ✅ 12/12 bit-identical`
/// or `identity: ❌ 2 differ (max FLIP 0.031 on bistro/cam3.png)`. `None`
/// unless the report was produced in [`Mode::Identity`].
pub fn identity_headline(report: &Report) -> Option<String> {
    if report.config.mode != Mode::Identity {
        return None;
    }
    let t = &report.totals;
    let compared = report
        .entries
        .iter()
        .filter(|e| e.bit_identical.is_some())
        .count();
    let identical = report
        .entries
        .iter()
        .filter(|e| e.bit_identical == Some(true))
        .count();
    let differ: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|e| e.status == Status::Fail && e.bit_identical != Some(true))
        .collect();
    let others: Vec<String> = [(t.error, "error"), (t.missing, "missing"), (t.new, "new")]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, label)| format!("{n} {label}"))
        .collect();
    let mut text = if differ.is_empty() && others.is_empty() {
        let within = compared - identical;
        let mut s = format!("✅ {identical}/{compared} bit-identical");
        if within > 0 {
            s.push_str(&format!(", {within} within threshold"));
        }
        s
    } else {
        let mut parts = Vec::new();
        if !differ.is_empty() {
            let worst = differ
                .iter()
                .filter_map(|e| e.metrics.as_ref().map(|m| (m.max, e.name.as_str())))
                .max_by(|a, b| a.0.total_cmp(&b.0));
            let mut s = format!("{} differ", differ.len());
            if let Some((max, name)) = worst {
                s.push_str(&format!(" (max FLIP {} on {})", sig3(max), code_name(name)));
            }
            parts.push(s);
        }
        parts.extend(others);
        let icon = if report.is_regression() { "❌" } else { "✅" };
        format!("{icon} {}", parts.join(", "))
    };
    if text.contains(['\n', '\r']) {
        text = text.replace(['\n', '\r'], " ");
    }
    Some(format!("identity: {text}"))
}

/// Formats `v` with 3 decimals (FLIP values are in `[0, 1]`).
fn sig3(v: f64) -> String {
    format!("{v:.3}")
}

fn heading(report: &Report) -> String {
    if let Some(h) = identity_headline(report) {
        return format!("### saccade {h}");
    }
    let t = &report.totals;
    let icon = if report.is_regression() { "❌" } else { "✅" };
    let parts: Vec<String> = [
        (t.fail, "failed"),
        (t.error, "errored"),
        (t.missing, "missing"),
        (t.new, "new"),
        (t.pass, "passed"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, label)| format!("{n} {label}"))
    .collect();
    if parts.is_empty() {
        format!("### saccade: {icon} no images compared")
    } else if report.is_empty_run() {
        format!(
            "### saccade: {icon} nothing compared · {}",
            parts.join(" · ")
        )
    } else {
        format!("### saccade: {icon} {}", parts.join(" · "))
    }
}

fn artifact_link(url: &str) -> String {
    let safe = url
        .replace(' ', "%20")
        .replace(')', "%29")
        .replace('(', "%28")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace(['\n', '\r'], "");
    format!("[Full report with heatmaps]({safe})")
}

/// `⚠ config differs on N images: key1, key2…` over every entry with a
/// metadata difference (not only the rows shown); `None` when there is none.
fn config_differs_line(report: &Report) -> Option<String> {
    const MAX_KEYS: usize = 10;
    let mut keys = std::collections::BTreeSet::new();
    let mut images = 0;
    for e in report.entries.iter().filter(|e| !e.meta_diff.is_empty()) {
        images += 1;
        keys.extend(e.meta_diff.iter().map(|d| d.key.as_str()));
    }
    if images == 0 {
        return None;
    }
    let shown: Vec<String> = keys
        .iter()
        .take(MAX_KEYS)
        .map(|k| code_name(&k.chars().take(60).collect::<String>()))
        .collect();
    let more = if keys.len() > MAX_KEYS { "…" } else { "" };
    let noun = if images == 1 { "image" } else { "images" };
    Some(format!(
        "⚠ config differs on {images} {noun}: {}{more}",
        shown.join(", ")
    ))
}

/// One line per entry with a warning or a local defect behind a passing
/// verdict, over every entry (not only the rows shown), capped.
fn notes_section(report: &Report) -> String {
    const MAX_NOTES: usize = 20;
    let mut lines = Vec::new();
    for e in &report.entries {
        let name = code_name(&e.name);
        if let Some(note) = e.local_hotspot_note() {
            lines.push(format!("- {name} ↳ {note}\n"));
        }
        // A passing pair is mentioned when timings come with it.
        if let (Status::Pass, Some(d)) = (e.status, &e.diagnostics)
            && (!d.perf.is_empty() || !d.perf_not_comparable.is_empty())
        {
            lines.push(format!(
                "- {name} ↳ {}\n",
                md_text(&d.verdict_line(e.bit_identical))
            ));
        }
        for w in &e.warnings {
            let one_line: String = w
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(160)
                .collect();
            lines.push(format!("- {name} ⚠ {}\n", code_name(&one_line)));
        }
    }
    let mut out: String = lines.iter().take(MAX_NOTES).map(String::as_str).collect();
    if lines.len() > MAX_NOTES {
        out.push_str(&format!("- …and {} more notes\n", lines.len() - MAX_NOTES));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

struct Parts<'a> {
    report: &'a Report,
    opts: &'a MarkdownOptions,
    nonpass: Vec<&'a Entry>,
    pass: Vec<&'a Entry>,
}

impl Parts<'_> {
    /// Assembles the document keeping the first `np` non-pass and `p` pass rows.
    fn assemble(&self, np: usize, p: usize) -> String {
        let dropped = (self.nonpass.len() - np) + (self.pass.len() - p);
        let mut out = String::new();
        out.push_str(&marker(self.opts.comment_key.as_deref()));
        out.push('\n');
        out.push_str(&heading(self.report));
        out.push_str("\n\n");
        if let Some(v) = &self.report.combined_verdict {
            out.push_str(&format!("{}\n\n", md_text(v)));
        }
        if let Some(diff) = &self.report.perf_diff {
            out.push_str(&format!("{}\n\n", md_text(&diff.summary(3))));
            for w in &diff.warnings {
                out.push_str(&format!("{}\n\n", md_text(w)));
            }
        }
        if np > 0 {
            out.push_str(TABLE_HEAD);
            for e in &self.nonpass[..np] {
                out.push_str(&row(e));
                out.push_str(&region_rows(e));
            }
            out.push('\n');
            let mut wrote = false;
            for e in self.nonpass[..np]
                .iter()
                .filter(|e| e.status == Status::Fail)
            {
                if let Some(d) = &e.diagnostics {
                    let perf = d
                        .perf_summary()
                        .map_or(String::new(), |p| format!(" · {}", md_text(&p)));
                    out.push_str(&format!(
                        "- {} ↳ {}: {}{perf}\n",
                        code_name(&e.name),
                        d.class.as_str(),
                        md_text(&d.description)
                    ));
                    wrote = true;
                }
                if let Some(line) = crate::hotspots::summary_line(&e.hotspots) {
                    out.push_str(&format!("- {} ↳ {line}\n", code_name(&e.name)));
                    wrote = true;
                }
            }
            if wrote {
                out.push('\n');
            }
        }
        out.push_str(&notes_section(self.report));
        if p > 0 {
            out.push_str(&format!(
                "<details><summary>{} passed</summary>\n\n",
                self.pass.len()
            ));
            out.push_str(TABLE_HEAD);
            for e in &self.pass[..p] {
                out.push_str(&row(e));
            }
            out.push_str("\n</details>\n\n");
        }
        if let Some(line) = config_differs_line(self.report) {
            out.push_str(&line);
            out.push_str("\n\n");
        }
        if dropped > 0 {
            out.push_str(&format!("…and {dropped} more (see full report)\n\n"));
        }
        if let Some(url) = &self.opts.artifact_url {
            out.push_str(&artifact_link(url));
            out.push_str("\n\n");
        }
        out.push_str(&format!(
            "<sub>saccade v{}</sub>\n",
            self.report.tool_version
        ));
        out
    }
}

/// Largest `n` in `0..=max` for which `fits(n)` holds, assuming `fits` is
/// monotone (true up to some point, false after). `None` if even 0 fails.
fn largest_fitting(max: usize, fits: impl Fn(usize) -> bool) -> Option<usize> {
    if !fits(0) {
        return None;
    }
    let (mut lo, mut hi) = (0, max);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(lo)
}

pub(crate) fn render_markdown(report: &Report, opts: &MarkdownOptions) -> String {
    let max = opts.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let mut nonpass: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|e| e.status != Status::Pass)
        .collect();
    nonpass.sort_by(|a, b| {
        status_rank(a.status)
            .cmp(&status_rank(b.status))
            .then_with(|| {
                b.value
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&a.value.unwrap_or(f64::NEG_INFINITY))
            })
            .then_with(|| a.name.cmp(&b.name))
    });
    let mut pass: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|e| e.status == Status::Pass)
        .collect();
    pass.sort_by(|a, b| {
        b.value
            .unwrap_or(0.0)
            .partial_cmp(&a.value.unwrap_or(0.0))
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    let parts = Parts {
        report,
        opts,
        nonpass,
        pass,
    };

    let full = parts.assemble(parts.nonpass.len(), parts.pass.len());
    if full.len() <= max {
        return full;
    }
    // Drop pass rows first, then non-pass rows.
    let np_all = parts.nonpass.len();
    if let Some(p) = largest_fitting(parts.pass.len(), |p| parts.assemble(np_all, p).len() <= max) {
        return parts.assemble(np_all, p);
    }
    if let Some(np) = largest_fitting(np_all, |np| parts.assemble(np, 0).len() <= max) {
        return parts.assemble(np, 0);
    }
    // Not even the frame fits (absurdly small cap): hard-cut on a char boundary.
    let mut s = parts.assemble(0, 0);
    let mut cut = max.min(s.len());
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s
}

#[cfg(test)]
mod tests {
    use super::sig4;

    #[test]
    fn four_significant_digits() {
        assert_eq!(sig4(0.012_345_67), "0.01235");
        assert_eq!(sig4(0.01), "0.01000");
        assert_eq!(sig4(0.5), "0.5000");
        assert_eq!(sig4(0.0), "0.0000");
        assert_eq!(sig4(123.456), "123.5");
    }
}
