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
    #[serde(default)]
    pub image_effect: ImageEffect,
    #[serde(default)]
    pub frame_change: crate::perf::FrameChange,
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
    /// Complete captures used for this arm, in stable order.
    #[serde(default)]
    pub repeats: Vec<String>,
    /// Repeats excluded before measurement, with a specific reason.
    #[serde(default)]
    pub excluded_repeats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_stability: Option<RepeatStability>,
    #[serde(default)]
    pub validity_findings: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEffect {
    Identical,
    Changed,
    #[default]
    Unknown,
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
        let valid = report.capture_validity().status != crate::meta::Validity::Invalid
            && report.perf_errors.is_empty();
        let (no_effect, perf_only) = (no_effect && valid, perf_only && valid);
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
            image_effect: if identical {
                ImageEffect::Identical
            } else if flag == "IMAGE-CHANGE" {
                ImageEffect::Changed
            } else {
                ImageEffect::Unknown
            },
            frame_change: report
                .perf_diff
                .as_ref()
                .map_or(crate::perf::FrameChange::Unknown, |d| d.frame_change),
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
            repeats: Vec::new(),
            excluded_repeats: Vec::new(),
            repeat_stability: None,
            validity_findings: Vec::new(),
            next_actions: Vec::new(),
        }
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ablation {
    pub schema: String,
    pub base: String,
    pub arms: Vec<Arm>,
    #[serde(default)]
    pub base_repeats: Vec<String>,
    #[serde(default)]
    pub excluded_base_repeats: Vec<String>,
    #[serde(default)]
    pub repeat_qualification: crate::perf::Comparability,
    #[serde(default)]
    pub repeat_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_stability: Option<RepeatStability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_noise: Option<crate::perf::PerfNoise>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_noise: Option<crate::perf::PerfNoise>,
    #[serde(default)]
    pub noise_override: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepeatStability {
    /// Native samples match across every complete repeat.
    pub stable: bool,
    pub unstable_images: Vec<String>,
    /// File SHA-256 values per image, in repeat order.
    pub image_hashes: std::collections::BTreeMap<String, Vec<String>>,
    /// Largest repeat-to-first FLIP max for every image, when computable.
    #[serde(default)]
    pub max_flip_by_image: std::collections::BTreeMap<String, f64>,
}
impl Ablation {
    /// Markdown evidence summary including repeat validity and next actions.
    pub fn markdown(&self) -> String {
        let mut out = String::from(
            "# Saccade ablation\n\n| Arm | Flag | Image | Repeat validity |\n|---|---|---|---|\n",
        );
        for arm in &self.arms {
            let clean = |s: &str| crate::perf::clean(s).replace('|', "\\|");
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                clean(&arm.label),
                clean(&arm.flag),
                clean(&arm.image_verdict),
                if arm.validity_findings.is_empty() {
                    "no repeat finding"
                } else {
                    "invalid"
                }
            ));
            for finding in &arm.validity_findings {
                out.push_str(&format!(
                    "\n- **{}:** {}.\n",
                    clean(&arm.label),
                    clean(finding)
                ));
            }
            if let Some(stability) = &arm.repeat_stability {
                for name in &stability.unstable_images {
                    out.push_str(&format!(
                        "  - {}: max FLIP {:.4}; SHA-256 {}\n",
                        clean(name),
                        stability
                            .max_flip_by_image
                            .get(name)
                            .copied()
                            .unwrap_or(0.0),
                        stability
                            .image_hashes
                            .get(name)
                            .map_or(String::new(), |h| h.join(", "))
                    ));
                }
            }
            for action in &arm.next_actions {
                out.push_str(&format!("  - Next action: {}\n", clean(action)));
            }
        }
        out
    }

    pub fn text(&self) -> String {
        let mut out = format!(
            "base {}\nARM\tFLAG\tCOMBINED VERDICT\tCONFIG DIFFERS\n",
            crate::perf::clean(&self.base)
        );
        if self.base_repeats.len() > 1 {
            out.push_str(&format!(
                "base repeats: {}; qualification: {:?}\n",
                self.base_repeats.len(),
                self.repeat_qualification
            ));
            for reason in &self.repeat_reasons {
                out.push_str(&format!("  repeat: {}\n", crate::perf::clean(reason)));
            }
            if let Some(stability) = &self.base_stability {
                out.push_str(&format!(
                    "  base output stable across repeats: {}\n",
                    stability.stable
                ));
            }
        }
        for reason in &self.excluded_base_repeats {
            out.push_str(&format!(
                "  excluded base repeat: {}\n",
                crate::perf::clean(reason)
            ));
        }
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
            if a.repeats.len() > 1 {
                out.push_str(&format!("  arm repeats: {}\n", a.repeats.len()));
            }
            if let Some(stability) = &a.repeat_stability {
                out.push_str(&format!(
                    "  output stable across repeats: {}\n",
                    stability.stable
                ));
            }
            for finding in &a.validity_findings {
                out.push_str(&format!("  validity: {}\n", crate::perf::clean(finding)));
            }
            for action in &a.next_actions {
                out.push_str(&format!("  next action: {}\n", crate::perf::clean(action)));
            }
            for reason in &a.excluded_repeats {
                out.push_str(&format!(
                    "  excluded arm repeat: {}\n",
                    crate::perf::clean(reason)
                ));
            }
            if let Some(d) = &a.perf_diff {
                if a.flag == "INCONCLUSIVE" {
                    for reason in &d.qualification_reasons {
                        let action = if reason.contains("warmup_complete") {
                            "record and qualify warmup on both captures"
                        } else if reason.contains("configuration hash mismatch") {
                            "recapture both arms with matching configuration"
                        } else {
                            "recapture with complete qualified producer evidence"
                        };
                        out.push_str(&format!(
                            "  qualification: {}; action: {}.\n",
                            crate::perf::clean(reason),
                            action
                        ));
                    }
                    if d.noise_comparability != crate::perf::Comparability::Qualified {
                        out.push_str("  qualification: repeat noise unavailable; capture unchanged-build repeats and pass --perf-noise FILE.\n");
                    }
                }
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
    let groups = arms
        .iter()
        .map(|p| (String::new(), vec![p.clone()]))
        .collect::<Vec<_>>();
    run_repeats(&[base.to_path_buf()], &groups, out, cfg, top)
}

/// Compare the first complete capture in each arm while deriving noise from
/// complete base repeats. Every excluded input remains visible in the model.
#[cfg(feature = "graphics")]
pub fn run_repeats(
    bases: &[PathBuf],
    groups: &[(String, Vec<PathBuf>)],
    out: &Path,
    cfg: &crate::config::RunConfig,
    top: usize,
) -> Result<Ablation> {
    if groups.is_empty() {
        return Err(crate::Error::Config("ablate needs at least one arm".into()));
    }
    cfg.validate()?;
    cfg.perf.resolved_floor()?;
    let all_paths = bases
        .iter()
        .chain(groups.iter().flat_map(|(_, paths)| paths.iter()));
    let mut required_sidecars = BTreeSet::new();
    let all_paths = all_paths.collect::<Vec<_>>();
    let has_repeats = bases.len() > 1 || groups.iter().any(|(_, paths)| paths.len() > 1);
    if has_repeats && all_paths.iter().any(|p| p.join(&cfg.perf.name).is_file()) {
        required_sidecars.insert(cfg.perf.name.clone());
        if !cfg.perf.gpu_clocks_not_applicable {
            required_sidecars.insert(crate::gpu_clock::FILE.into());
        }
    }
    if has_repeats {
        for name in [crate::meta::DEFAULT_META_NAME, "cost-card.json"] {
            if all_paths.iter().any(|p| p.join(name).is_file()) {
                required_sidecars.insert(name.into());
            }
        }
    }
    for path in all_paths.iter().filter(|_| has_repeats) {
        if let Ok(images) = crate::run::collect_images(path) {
            for name in images.files.keys() {
                let image = Path::new(name);
                if let Some(stem) = image.file_stem() {
                    let sidecar = image.with_file_name(format!(
                        "{}.{}",
                        stem.to_string_lossy(),
                        crate::meta::DEFAULT_META_NAME
                    ));
                    if path.join(&sidecar).is_file() {
                        required_sidecars.insert(sidecar.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    let (bases, excluded_base_repeats, expected) =
        complete_repeats(bases, None, &cfg.perf.name, &required_sidecars)?;
    let base = bases
        .first()
        .ok_or_else(|| crate::Error::Config("no complete base repeat".into()))?;
    let mut accepted = Vec::new();
    for (label, paths) in groups {
        let (paths, excluded, _) =
            complete_repeats(paths, Some(&expected), &cfg.perf.name, &required_sidecars)?;
        if paths.is_empty() {
            return Err(crate::Error::Config(format!(
                "arm {label:?} has no complete repeats: {}",
                excluded.join("; ")
            )));
        }
        accepted.push((label.clone(), paths, excluded));
    }
    let arms = accepted
        .iter()
        .map(|(_, paths, _)| paths[0].clone())
        .collect::<Vec<_>>();
    let inputs: Vec<_> = std::iter::once(base)
        .chain(bases.iter().skip(1))
        .chain(accepted.iter().flat_map(|(_, paths, _)| paths.iter()))
        .map(PathBuf::as_path)
        .collect();
    crate::run::guard_output_dir(out, &inputs, &[ABLATE_FILE, crate::run::RUN_SENTINEL])?;
    std::fs::create_dir_all(out)
        .map_err(crate::run::io_err(format!("creating {}", out.display())))?;
    std::fs::write(out.join(crate::run::RUN_SENTINEL), b"incomplete ablation\n")
        .map_err(crate::run::io_err("writing ablation marker".into()))?;
    // Remove only stale summary files; every arm's report has its own ownership guard.
    for name in [ABLATE_FILE, "index.html", "ablation.txt", "ablation.md"] {
        let p = out.join(name);
        if p.is_file() {
            std::fs::remove_file(&p)
                .map_err(crate::run::io_err(format!("removing {}", p.display())))?;
        }
    }
    let labels = crate::runs::unique_labels(&arms);
    let (floor, repeat_reasons) = if bases.len() > 1 {
        crate::perf::noise_with_options(&bases, &cfg.perf)?
    } else {
        (
            None,
            vec!["at least two complete base repeats required for automatic noise".into()],
        )
    };
    let repeat_qualification = floor
        .as_ref()
        .map_or(crate::perf::Comparability::Unknown, |f| f.comparability);
    let base_stability = repeat_stability(&bases)?;
    let mut effective = cfg.clone();
    let explicit_noise = effective.perf.resolved_floor()?;
    effective.perf.floor = stricter_floor(
        explicit_noise.as_ref(),
        floor.as_ref(),
        effective.perf.noise_override,
    );
    effective.perf.noise = None;
    let mut rows = Vec::new();
    for (i, ((declared, paths, excluded), (arm, fallback))) in
        accepted.iter().zip(arms.iter().zip(labels)).enumerate()
    {
        let dir = format!("arm-{}", i + 1);
        let report = crate::run::run(base, arm, &out.join(&dir), &effective)?;
        let mut row = Arm::from_report(
            if declared.is_empty() {
                fallback
            } else {
                declared.clone()
            },
            crate::paths::record(arm, out, cfg.record_absolute_paths),
            format!("{dir}/index.html"),
            &report,
            top,
        );
        row.repeats = paths
            .iter()
            .map(|p| crate::paths::record(p, out, cfg.record_absolute_paths))
            .collect();
        row.excluded_repeats = excluded.clone();
        row.repeat_stability = repeat_stability(paths)?;
        if let Some(stability) = &row.repeat_stability {
            let base = base_stability.as_ref();
            let exceeded = stability.unstable_images.iter().any(|name| {
                let arm_max = stability
                    .max_flip_by_image
                    .get(name)
                    .copied()
                    .unwrap_or(1.0);
                let base_max = base
                    .and_then(|s| s.max_flip_by_image.get(name))
                    .copied()
                    .unwrap_or(0.0);
                arm_max > base_max + 1e-6
            });
            if exceeded {
                row.validity_findings
                    .push("arm output is not deterministic across repeats".into());
                row.next_actions.push("inspect repeat image hashes and recapture the arm under fixed rendering conditions before accepting its claim".into());
                row.no_effect = false;
                row.perf_only = false;
                row.flag = "INCONCLUSIVE".into();
                row.combined_verdict = format!(
                    "{} · repeat instability invalidates the arm claim",
                    row.image_verdict
                );
            }
        }
        rows.push(row);
    }
    let model = Ablation {
        schema: "saccade-ablate.v1".into(),
        base: crate::paths::record(base, out, cfg.record_absolute_paths),
        arms: rows,
        base_repeats: bases
            .iter()
            .map(|p| crate::paths::record(p, out, cfg.record_absolute_paths))
            .collect(),
        excluded_base_repeats,
        repeat_qualification,
        repeat_reasons,
        base_stability,
        derived_noise: floor,
        explicit_noise,
        noise_override: cfg.perf.noise_override,
    };
    for (name, text) in [
        (ABLATE_FILE, serde_json::to_string_pretty(&model)?),
        ("index.html", model.html()?),
        ("ablation.txt", model.text()),
        ("ablation.md", model.markdown()),
    ] {
        std::fs::write(out.join(name), text)
            .map_err(crate::run::io_err(format!("writing {name}")))?;
    }
    std::fs::remove_file(out.join(crate::run::RUN_SENTINEL))
        .map_err(crate::run::io_err("removing ablation marker".into()))?;
    Ok(model)
}

#[cfg(feature = "graphics")]
fn stricter_floor(
    explicit: Option<&crate::perf::PerfNoise>,
    derived: Option<&crate::perf::PerfNoise>,
    override_derived: bool,
) -> Option<crate::perf::PerfNoise> {
    match (explicit, derived) {
        (Some(explicit), Some(derived)) if !override_derived => {
            let mut combined = explicit.clone();
            combined.frame = combined.frame.max(derived.frame);
            for (term, spread) in &derived.terms {
                let entry = combined.terms.entry(term.clone()).or_default();
                *entry = (*entry).max(*spread);
            }
            if derived.comparability != crate::perf::Comparability::Qualified {
                combined.comparability = derived.comparability;
            }
            Some(combined)
        }
        (Some(explicit), _) => Some(explicit.clone()),
        (None, derived) => derived.cloned(),
    }
}

#[cfg(feature = "graphics")]
fn repeat_stability(paths: &[PathBuf]) -> Result<Option<RepeatStability>> {
    if paths.len() < 2 {
        return Ok(None);
    }
    let sets = paths
        .iter()
        .map(|p| crate::run::collect_images(p))
        .collect::<Result<Vec<_>>>()?;
    let mut unstable_images = Vec::new();
    let mut image_hashes = std::collections::BTreeMap::new();
    let mut max_flip_by_image = std::collections::BTreeMap::new();
    for name in sets[0].files.keys() {
        let first = &sets[0].files[name];
        let mut hashes = Vec::new();
        let mut max_flip = 0.0_f64;
        for set in &sets {
            let path = &set.files[name];
            hashes.push(crate::run::sha256_file(path)?);
            let differs = !crate::compare::native_samples_identical(first, path);
            if differs && !unstable_images.contains(name) {
                unstable_images.push(name.clone());
            }
            if differs {
                let a = crate::run::decode(first)?;
                let b = crate::run::decode(path)?;
                if a.dimensions() == b.dimensions() {
                    let score = crate::compare::compare_rgba(
                        &b,
                        &a,
                        &crate::compare::CompareOptions::default(),
                    )?
                    .metrics
                    .max;
                    max_flip = max_flip.max(score);
                } else {
                    max_flip = 1.0;
                }
            }
        }
        image_hashes.insert(name.clone(), hashes);
        max_flip_by_image.insert(name.clone(), max_flip);
    }
    Ok(Some(RepeatStability {
        stable: unstable_images.is_empty(),
        unstable_images,
        image_hashes,
        max_flip_by_image,
    }))
}

#[cfg(feature = "graphics")]
fn complete_repeats(
    paths: &[PathBuf],
    expected: Option<&std::collections::BTreeSet<String>>,
    perf_name: &str,
    required_sidecars: &BTreeSet<String>,
) -> Result<(
    Vec<PathBuf>,
    Vec<String>,
    std::collections::BTreeSet<String>,
)> {
    let mut accepted = Vec::new();
    let mut excluded = Vec::new();
    let mut names = expected.cloned().unwrap_or_default();
    let require_perf = paths.len() > 1 && paths.iter().any(|p| p.join(perf_name).is_file());
    for path in paths {
        let found = match crate::run::collect_images(path) {
            Ok(v) => v,
            Err(e) => {
                excluded.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let keys = found
            .files
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let problem = if !found.problems.is_empty() {
            Some(format!(
                "unreadable images: {:?}",
                found.problems.keys().collect::<Vec<_>>()
            ))
        } else if keys.is_empty() {
            Some("no images".to_string())
        } else if !names.is_empty() && keys != names {
            Some(format!(
                "image set differs: missing {:?}, extra {:?}",
                names.difference(&keys).collect::<Vec<_>>(),
                keys.difference(&names).collect::<Vec<_>>()
            ))
        } else if found.files.values().any(|p| !crate::run::is_decodable(p)) {
            Some("image cannot be decoded".into())
        } else if let Some(missing) = required_sidecars
            .iter()
            .find(|name| !path.join(name).is_file())
        {
            Some(format!("missing {missing}"))
        } else if require_perf && !path.join(perf_name).is_file() {
            Some(format!("missing {perf_name}"))
        } else {
            None
        };
        if let Some(reason) = problem {
            excluded.push(format!("{}: {reason}", path.display()));
        } else {
            if names.is_empty() {
                names = keys;
            }
            accepted.push(path.clone());
        }
    }
    Ok((accepted, excluded, names))
}

#[cfg(all(test, feature = "graphics"))]
#[allow(clippy::unwrap_used)]
mod repeat_tests {
    use super::*;

    #[test]
    fn explicit_noise_cannot_hide_higher_derived_floor_without_recorded_override() {
        let explicit = crate::perf::PerfNoise {
            frame: 0.1,
            ..Default::default()
        };
        let derived = crate::perf::PerfNoise {
            frame: 0.5,
            ..Default::default()
        };
        assert_eq!(
            stricter_floor(Some(&explicit), Some(&derived), false)
                .unwrap()
                .frame,
            0.5
        );
        assert_eq!(
            stricter_floor(Some(&explicit), Some(&derived), true)
                .unwrap()
                .frame,
            0.1
        );
    }

    #[test]
    fn missing_clock_sidecar_is_listed_as_incomplete_repeat() {
        let temp = tempfile::tempdir().unwrap();
        let dirs: Vec<_> = (0..3).map(|i| temp.path().join(format!("r{i}"))).collect();
        for dir in &dirs {
            std::fs::create_dir_all(dir).unwrap();
            image::RgbaImage::new(2, 2)
                .save(dir.join("frame.png"))
                .unwrap();
            std::fs::write(dir.join("saccade-perf.json"), b"{}").unwrap();
            std::fs::write(dir.join("gpu_clock.json"), b"{}").unwrap();
        }
        std::fs::remove_file(dirs[2].join("gpu_clock.json")).unwrap();
        let required = BTreeSet::from(["saccade-perf.json".into(), "gpu_clock.json".into()]);
        let (accepted, excluded, _) =
            complete_repeats(&dirs, None, "saccade-perf.json", &required).unwrap();
        assert_eq!(accepted.len(), 2);
        assert!(excluded[0].contains("missing gpu_clock.json"));
    }

    #[test]
    fn incomplete_repeat_is_named_and_excluded() {
        let temp = tempfile::tempdir().unwrap();
        let complete = temp.path().join("r1");
        let incomplete = temp.path().join("r2");
        std::fs::create_dir_all(&complete).unwrap();
        std::fs::create_dir_all(&incomplete).unwrap();
        image::RgbaImage::new(2, 2)
            .save(complete.join("frame.png"))
            .unwrap();
        std::fs::write(complete.join("saccade-perf.json"), "{}").unwrap();
        let (accepted, excluded, names) = complete_repeats(
            &[complete.clone(), incomplete.clone()],
            None,
            "saccade-perf.json",
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(accepted, vec![complete]);
        assert!(names.contains("frame.png"));
        assert_eq!(excluded.len(), 1);
        assert!(excluded[0].contains("no images"));
        assert!(excluded[0].contains("r2"));
    }

    #[test]
    fn unstable_arm_produces_validity_finding_and_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = ["base1", "base2", "arm1", "arm2"];
        for (index, name) in dirs.iter().enumerate() {
            let dir = temp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let mut image = image::RgbaImage::from_pixel(32, 32, image::Rgba([20, 20, 20, 255]));
            if index == 3 {
                image.put_pixel(16, 16, image::Rgba([255, 255, 255, 255]));
            }
            image.save(dir.join("image.png")).unwrap();
        }
        let bases = dirs[..2]
            .iter()
            .map(|n| temp.path().join(n))
            .collect::<Vec<_>>();
        let arms = vec![(
            "arm".into(),
            dirs[2..].iter().map(|n| temp.path().join(n)).collect(),
        )];
        let model = run_repeats(
            &bases,
            &arms,
            &temp.path().join("out"),
            &crate::config::RunConfig::default(),
            1,
        )
        .unwrap();
        assert!(model.base_stability.as_ref().unwrap().stable);
        assert_eq!(
            model.arms[0].validity_findings,
            ["arm output is not deterministic across repeats"]
        );
        assert_eq!(model.arms[0].flag, "INCONCLUSIVE");
        assert!(!model.arms[0].perf_only && !model.arms[0].no_effect);
        assert!(
            model.arms[0]
                .combined_verdict
                .contains("repeat instability")
        );
        assert!(
            model.arms[0]
                .repeat_stability
                .as_ref()
                .unwrap()
                .max_flip_by_image["image.png"]
                > 0.0
        );
        assert!(
            model
                .markdown()
                .contains("arm output is not deterministic across repeats")
        );
    }
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
