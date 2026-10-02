//! Ablation arms compared against one common base, with image and perf evidence.
#![allow(missing_docs)]
use crate::perf::{PerfDiff, PerfError, TermDiff};
use crate::{Entry, Report, Result, Status};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const ABLATE_FILE: &str = "saccade-ablate.v1.json";

pub fn image_verdict(entries: &[Entry]) -> String {
    if entries.is_empty()
        || entries
            .iter()
            .any(|e| !matches!(e.status, Status::Pass | Status::Fail) || e.bit_identical.is_none())
    {
        return "image not comparable".into();
    }
    if entries.iter().all(|e| e.bit_identical == Some(true)) {
        return "image bit-identical".into();
    }
    let classes: BTreeSet<_> = entries
        .iter()
        .filter(|e| e.bit_identical == Some(false))
        .filter_map(|e| e.diagnostics.as_ref().map(|d| d.class.as_str()))
        .collect();
    if classes.is_empty() {
        "image changed (FLIP class unavailable)".into()
    } else {
        format!(
            "image changed (FLIP {})",
            classes.into_iter().collect::<Vec<_>>().join(", ")
        )
    }
}
pub fn combined(entries: &[Entry], perf: &PerfDiff) -> String {
    format!("{} · {}", image_verdict(entries), perf.verdict())
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arm {
    pub label: String,
    pub path: String,
    pub image_verdict: String,
    pub frame_delta: Option<f64>,
    pub top_deltas: Vec<TermDiff>,
    pub config_differs: Vec<String>,
    pub no_effect: bool,
    pub perf_only: bool,
    pub flag: String,
    pub combined_verdict: String,
    pub perf_diff: Option<PerfDiff>,
    pub errors: Vec<String>,
    pub perf_errors: Vec<PerfError>,
    pub report_html: String,
}
impl Arm {
    pub fn from_report(
        label: String,
        path: String,
        report_html: String,
        report: &Report,
        top: usize,
    ) -> Self {
        let image = image_verdict(&report.entries);
        let identical = image == "image bit-identical";
        let (no_effect, perf_only) = report
            .perf_diff
            .as_ref()
            .map_or((false, false), |d| d.flags(identical));
        let flag = if no_effect {
            "NO-EFFECT"
        } else if perf_only {
            "PERF-ONLY"
        } else if image == "image not comparable" {
            "INCONCLUSIVE"
        } else if !identical {
            "IMAGE-CHANGE"
        } else {
            "INCONCLUSIVE"
        };
        let config_differs = report
            .entries
            .iter()
            .flat_map(|e| e.meta_diff.iter().map(|d| d.key.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            label,
            path,
            report_html,
            frame_delta: report.perf_diff.as_ref().and_then(|d| d.frame.delta),
            top_deltas: report.perf_diff.as_ref().map_or_else(Vec::new, |d| {
                d.top(top, true).into_iter().cloned().collect()
            }),
            combined_verdict: report
                .combined_verdict
                .clone()
                .unwrap_or_else(|| format!("{image} · performance unavailable")),
            image_verdict: image,
            config_differs,
            no_effect,
            perf_only,
            flag: flag.into(),
            perf_diff: report.perf_diff.clone(),
            perf_errors: report.perf_errors.clone(),
            errors: report
                .entries
                .iter()
                .filter_map(|e| e.error.clone())
                .collect(),
        }
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ablation {
    pub schema: String,
    pub base: String,
    pub arms: Vec<Arm>,
}
impl Ablation {
    pub fn text(&self) -> String {
        let mut out = format!(
            "base {}\nARM\tFLAG\tCOMBINED VERDICT\tCONFIG DIFFERS\n",
            crate::perf::clean(&self.base)
        );
        for a in &self.arms {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                crate::perf::clean(&a.label),
                a.flag,
                crate::perf::clean(&a.combined_verdict),
                a.config_differs
                    .iter()
                    .map(|s| crate::perf::clean(s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Some(d) = &a.perf_diff {
                for t in &a.top_deltas {
                    out.push_str(&format!(
                        "  {} {:+.6} ms (beyond noise; threshold {:.6} ms; k={})\n",
                        crate::perf::clean(&t.id),
                        t.change.delta.unwrap_or(0.0),
                        t.change.noise_threshold.unwrap_or(0.0),
                        d.noise_k
                    ));
                }
                if !d.not_comparable(1).is_empty() {
                    out.push_str("  terms differ (not comparable):\n");
                    for t in d.not_comparable(usize::MAX) {
                        out.push_str(&format!("    {}\n", d.describe_unpaired(t)));
                    }
                }
                for w in &d.warnings {
                    out.push_str(&format!("  warning: {}\n", crate::perf::clean(w)));
                }
            }
            for e in &a.errors {
                out.push_str(&format!("  error: {}\n", crate::perf::clean(e)));
            }
        }
        out
    }
    pub fn html(&self) -> Result<String> {
        let data = serde_json::to_string(self)?
            .replace("</", "<\\/")
            .replace("<!--", "<\\u0021--");
        Ok(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>saccade ablation</title><style>{}</style></head><body><main class=\"perf-page\"><h1>saccade ablation</h1><p><a href=\"{ABLATE_FILE}\">JSON evidence</a></p><div id=\"ablation\"></div></main><script type=\"application/json\" id=\"ablation-data\">{data}</script><script>{}\nwindow.saccadePerf.ablation(document.getElementById('ablation'), JSON.parse(document.getElementById('ablation-data').textContent));</script></body></html>",
            crate::render::shared::page_css(&[]),
            crate::render::shared::page_js(&[])
        ))
    }
}
#[cfg(feature = "graphics")]
pub fn run(
    base: &Path,
    arms: &[PathBuf],
    out: &Path,
    cfg: &crate::config::RunConfig,
    top: usize,
) -> Result<Ablation> {
    if arms.is_empty() {
        return Err(crate::Error::Config("ablate needs at least one arm".into()));
    }
    cfg.validate()?;
    cfg.perf.resolved_floor()?;
    let inputs: Vec<_> = std::iter::once(base)
        .chain(arms.iter().map(PathBuf::as_path))
        .collect();
    crate::run::guard_output_dir(out, &inputs, &[ABLATE_FILE, crate::run::RUN_SENTINEL])?;
    std::fs::create_dir_all(out)
        .map_err(crate::run::io_err(format!("creating {}", out.display())))?;
    std::fs::write(out.join(crate::run::RUN_SENTINEL), b"incomplete ablation\n")
        .map_err(crate::run::io_err("writing ablation marker".into()))?;
    // Remove only stale summary files; every arm's report has its own ownership guard.
    for name in [ABLATE_FILE, "index.html", "ablation.txt"] {
        let p = out.join(name);
        if p.is_file() {
            std::fs::remove_file(&p)
                .map_err(crate::run::io_err(format!("removing {}", p.display())))?;
        }
    }
    let labels = crate::runs::unique_labels(arms);
    let mut rows = Vec::new();
    for (i, (arm, label)) in arms.iter().zip(labels).enumerate() {
        let dir = format!("arm-{}", i + 1);
        let report = crate::run::run(base, arm, &out.join(&dir), cfg)?;
        rows.push(Arm::from_report(
            label,
            crate::paths::record(arm, out, cfg.record_absolute_paths),
            format!("{dir}/index.html"),
            &report,
            top,
        ));
    }
    let model = Ablation {
        schema: "saccade-ablate.v1".into(),
        base: crate::paths::record(base, out, cfg.record_absolute_paths),
        arms: rows,
    };
    for (name, text) in [
        (ABLATE_FILE, serde_json::to_string_pretty(&model)?),
        ("index.html", model.html()?),
        ("ablation.txt", model.text()),
    ] {
        std::fs::write(out.join(name), text)
            .map_err(crate::run::io_err(format!("writing {name}")))?;
    }
    std::fs::remove_file(out.join(crate::run::RUN_SENTINEL))
        .map_err(crate::run::io_err("removing ablation marker".into()))?;
    Ok(model)
}

/// Returns `feature_unavailable` when graphics computation is not compiled.
#[cfg(not(feature = "graphics"))]
pub fn run(
    _base: &Path,
    _arms: &[PathBuf],
    _out: &Path,
    _cfg: &crate::config::RunConfig,
    _top: usize,
) -> Result<Ablation> {
    Err(crate::Error::FeatureUnavailable {
        feature: "graphics",
    })
}
