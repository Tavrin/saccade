//! Deterministic checks of a predeclared visual change against measured evidence.
use crate::report::{Entry, Report, Status};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Versioned declaration schema.
pub const SCHEMA: &str = "saccade-visual-intent.v1";
/// Verification artifact written beside a pair report.
pub const RESULT_FILE: &str = "intent-verification.v1.json";

/// Visual change declared before comparing captures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualIntent {
    /// Required occupancy in addition to expected visual changes (wave9).
    #[serde(default)]
    pub required_effects: Vec<crate::evidence_quality::effect::RequiredEffect>,
    /// Must equal [`SCHEMA`].
    pub schema: String,
    /// Intended outcome in the author's words.
    pub objective: String,
    /// Required assertion that change outside declared regions is unexpected.
    pub no_change_elsewhere: bool,
    /// Expected per-entry changes.
    pub changes: Vec<ExpectedChange>,
}

/// One expected kind of change in an image region.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedChange {
    /// Exact entry path in the pair report.
    pub entry: String,
    /// Expected measured change.
    pub kind: ChangeKind,
    /// Fractional box `[x,y,w,h]`, exclusive far edge.
    #[serde(default)]
    pub rect_frac: Option<[f64; 4]>,
    /// Relative white-pixel mask, same dimensions as the image.
    #[serde(default)]
    pub mask: Option<String>,
}

/// Deterministically verifiable visual intent categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Full-frame positive exposure shift.
    ToneUp,
    /// Full-frame negative exposure shift.
    ToneDown,
    /// Local hotspot.
    Structure,
    /// No native sample change.
    None,
}

/// One declaration-to-measurement finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Report entry name.
    pub entry: String,
    /// Declared or observed change kind.
    pub kind: String,
    /// Measured reason.
    pub detail: String,
}

/// Full deterministic outcome, kept separate from review authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    /// Versioned artifact discriminator.
    pub schema: String,
    /// Observations that support a declaration.
    pub matched: Vec<Finding>,
    /// Measured changes outside the declaration or forbidden by `none`.
    pub unexpected: Vec<Finding>,
    /// Declared changes with no matching measurement.
    pub missing: Vec<Finding>,
    /// Scope or input problems that prevent a check.
    pub unmeasurable: Vec<Finding>,
}

impl VisualIntent {
    /// Check declaration shape and relative mask provenance before comparison.
    pub fn validate(&self, source: &Path) -> crate::Result<()> {
        use crate::error::Error;
        if self.schema != SCHEMA || self.objective.trim().is_empty() || !self.no_change_elsewhere {
            return Err(Error::Config(format!(
                "intent needs schema {SCHEMA}, objective and no_change_elsewhere: true"
            )));
        }
        for effect in &self.required_effects {
            effect.validate()?;
        }
        for change in &self.changes {
            if change.entry.trim().is_empty() || change.rect_frac.is_some() == change.mask.is_some()
            {
                return Err(Error::Config(
                    "each expected change needs an entry and exactly one of rect_frac or mask"
                        .into(),
                ));
            }
            if let Some([x, y, w, h]) = change.rect_frac
                && (![x, y, w, h]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    || w <= 0.0
                    || h <= 0.0
                    || x + w > 1.0
                    || y + h > 1.0)
            {
                return Err(Error::Config(
                    "intent rect_frac must fit within the image".into(),
                ));
            }
            if let Some(mask) = &change.mask {
                let path = Path::new(mask);
                let root = source.parent().unwrap_or(Path::new("."));
                let candidate = root.join(path);
                if path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    || !candidate.is_file()
                    || !std::fs::canonicalize(&candidate)
                        .ok()
                        .zip(std::fs::canonicalize(root).ok())
                        .is_some_and(|(file, root)| file.starts_with(root))
                {
                    return Err(Error::Config(
                        "intent mask must be an existing relative file beside the declaration"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn dimensions(entry: &Entry) -> Option<(u32, u32)> {
    entry.metrics.as_ref().map(|m| (m.width, m.height))
}

fn pixel_match(
    change: &ExpectedChange,
    source: &Path,
    entry: &Entry,
    runs: &[[u32; 2]],
    all: bool,
) -> bool {
    let Some((width, height)) = dimensions(entry) else {
        return false;
    };
    if runs.is_empty() {
        return false;
    }
    let image = change.mask.as_ref().and_then(|mask| {
        image::open(source.parent().unwrap_or(Path::new(".")).join(mask))
            .ok()
            .map(|i| i.to_luma8())
    });
    if change.mask.is_some()
        && image
            .as_ref()
            .is_none_or(|i| i.width() != width || i.height() != height)
    {
        return false;
    }
    let hit = |index: u32| pixel_in_change(change, image.as_ref(), width, height, index);
    let pixels = runs
        .iter()
        .flat_map(|[start, len]| *start..start.saturating_add(*len));
    if all {
        pixels.into_iter().all(hit)
    } else {
        pixels.into_iter().any(hit)
    }
}

fn pixel_in_change(
    change: &ExpectedChange,
    image: Option<&image::GrayImage>,
    width: u32,
    height: u32,
    index: u32,
) -> bool {
    let x = index % width;
    let y = index / width;
    if let Some([rx, ry, rw, rh]) = change.rect_frac {
        let fx = (f64::from(x) + 0.5) / f64::from(width);
        let fy = (f64::from(y) + 0.5) / f64::from(height);
        fx >= rx && fx < rx + rw && fy >= ry && fy < ry + rh
    } else {
        image.is_some_and(|i| i.get_pixel(x, y).0[0] > 0)
    }
}

/// Match a validated declaration against the report's hotspots and diagnostics.
pub fn verify(intent: &VisualIntent, source: &Path, report: &Report) -> Verification {
    let mut result = Verification {
        schema: "saccade-intent-verification.v1".into(),
        matched: vec![],
        unexpected: vec![],
        missing: vec![],
        unmeasurable: vec![],
    };
    for effect in &intent.required_effects {
        let entries: Vec<_> = report
            .entries
            .iter()
            .filter(|e| {
                crate::config::compile_glob(&effect.glob).is_ok_and(|g| g.is_match(&e.name))
            })
            .collect();
        if entries.is_empty() {
            result.unmeasurable.push(Finding {
                entry: effect.glob.clone(),
                kind: "required_effect".into(),
                detail: "effect has no measured entries".into(),
            });
        }
        for entry in entries {
            let measured = entry.required_effects.iter().find(|r| r.policy == *effect);
            let finding = Finding {
                entry: entry.name.clone(),
                kind: "required_effect".into(),
                detail: measured.map_or_else(
                    || "required occupancy not measured".into(),
                    |r| {
                        format!(
                            "baseline {} candidate {}; failures {:?}",
                            r.baseline_pixels, r.candidate_pixels, r.failures
                        )
                    },
                ),
            };
            match measured {
                Some(r) if r.failures.is_empty() => result.matched.push(finding),
                Some(_) => result.missing.push(finding),
                None => result.unmeasurable.push(finding),
            }
        }
    }
    for change in &intent.changes {
        let Some(entry) = report.entries.iter().find(|e| e.name == change.entry) else {
            result.unmeasurable.push(Finding {
                entry: change.entry.clone(),
                kind: format!("{:?}", change.kind),
                detail: "entry absent from measured scope".into(),
            });
            continue;
        };
        if !matches!(entry.status, Status::Pass | Status::Fail) || dimensions(entry).is_none() {
            result.unmeasurable.push(Finding {
                entry: change.entry.clone(),
                kind: format!("{:?}", change.kind),
                detail: "entry has no complete image comparison".into(),
            });
            continue;
        }
        if let Some(mask) = &change.mask {
            let dimensions = dimensions(entry).unwrap_or((0, 0));
            let mask_path = source.parent().unwrap_or(Path::new(".")).join(mask);
            if image::image_dimensions(mask_path).ok() != Some(dimensions) {
                result.unmeasurable.push(Finding {
                    entry: change.entry.clone(),
                    kind: format!("{:?}", change.kind),
                    detail: "mask dimensions differ from measured image".into(),
                });
                continue;
            }
        }
        let hits = usize::from(pixel_match(
            change,
            source,
            entry,
            &entry.changed_pixel_runs,
            false,
        ));
        let tone = entry
            .diagnostics
            .as_ref()
            .and_then(|d| d.tone.as_ref())
            .map(|t| t.exposure_stops);
        let full_frame = change.rect_frac == Some([0.0, 0.0, 1.0, 1.0]);
        let observed = match change.kind {
            ChangeKind::Structure => hits > 0,
            ChangeKind::ToneUp => full_frame && tone.is_some_and(|v| v > 0.0),
            ChangeKind::ToneDown => full_frame && tone.is_some_and(|v| v < 0.0),
            ChangeKind::None => hits == 0 && entry.bit_identical == Some(true),
        };
        let finding = Finding {
            entry: change.entry.clone(),
            kind: format!("{:?}", change.kind).to_lowercase(),
            detail: format!(
                "{hits} changed-pixel region match; tone exposure {tone:?} stops; native identical {:?}",
                entry.bit_identical
            ),
        };
        if observed {
            result.matched.push(finding);
        } else if change.kind == ChangeKind::None {
            result.unexpected.push(finding);
        } else {
            result.missing.push(finding);
        }
    }
    if intent.no_change_elsewhere {
        for entry in &report.entries {
            if !matches!(entry.status, Status::Pass | Status::Fail) {
                continue;
            }
            let declared: Vec<_> = intent
                .changes
                .iter()
                .filter(|c| c.entry == entry.name && c.kind != ChangeKind::None)
                .collect();
            let masks: Vec<_> = declared
                .iter()
                .map(|c| {
                    c.mask.as_ref().and_then(|mask| {
                        image::open(source.parent().unwrap_or(Path::new(".")).join(mask))
                            .ok()
                            .map(|image| image.to_luma8())
                    })
                })
                .collect();
            let (width, height) = dimensions(entry).unwrap_or((1, 1));
            let uncovered = entry
                .changed_pixel_runs
                .iter()
                .flat_map(|[start, len]| *start..start.saturating_add(*len))
                .filter(|index| {
                    !declared
                        .iter()
                        .zip(&masks)
                        .any(|(c, image)| pixel_in_change(c, image.as_ref(), width, height, *index))
                })
                .count();
            if uncovered > 0 {
                result.unexpected.push(Finding {
                    entry: entry.name.clone(),
                    kind: "hotspot_elsewhere".into(),
                    detail: format!("{uncovered} changed pixels outside declared regions"),
                });
            }
            if entry.bit_identical == Some(false)
                && entry.changed_pixel_runs.is_empty()
                && declared.is_empty()
            {
                result.unexpected.push(Finding {
                    entry: entry.name.clone(),
                    kind: "change_elsewhere".into(),
                    detail: "native samples differ without a declared change".into(),
                });
            }
            if entry
                .diagnostics
                .as_ref()
                .is_some_and(|d| d.class == crate::diagnostics::ChangeClass::GlobalTone)
                && !declared.iter().any(|c| {
                    matches!(c.kind, ChangeKind::ToneUp | ChangeKind::ToneDown)
                        && c.rect_frac == Some([0.0, 0.0, 1.0, 1.0])
                })
            {
                result.unexpected.push(Finding {
                    entry: entry.name.clone(),
                    kind: "tone_elsewhere".into(),
                    detail: "undeclared global tone change".into(),
                });
            }
            if entry
                .diagnostics
                .as_ref()
                .is_some_and(|d| d.class == crate::diagnostics::ChangeClass::Misaligned)
                && declared.is_empty()
            {
                result.unexpected.push(Finding {
                    entry: entry.name.clone(),
                    kind: "shift_elsewhere".into(),
                    detail: "undeclared image shift".into(),
                });
            }
        }
    }
    result
}

/// Attach predeclared required effects to the normal comparison policy.
pub fn apply_effects(
    intent: &VisualIntent,
    source: &Path,
    config: &mut crate::config::RunConfig,
) -> crate::Result<()> {
    for effect in &intent.required_effects {
        if config.required_effect.iter().any(|e| e.name == effect.name) {
            return Err(crate::Error::Config(
                "duplicate required-effect name".into(),
            ));
        }
        config.effect_roots.insert(
            effect.name.clone(),
            source.parent().unwrap_or(Path::new(".")).to_path_buf(),
        );
        config.required_effect.push(effect.clone());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn checks_declared_patch_and_unexpected_elsewhere() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("base");
        let cap = tmp.path().join("cap");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&cap).unwrap();
        let baseline = RgbImage::from_pixel(64, 64, Rgb([80, 80, 80]));
        let mut capture = baseline.clone();
        for y in 8..24 {
            for x in 8..24 {
                capture.put_pixel(x, y, Rgb([240, 240, 240]));
            }
        }
        baseline.save(base.join("a.png")).unwrap();
        capture.save(cap.join("a.png")).unwrap();
        let report = crate::run::run(
            &base,
            &cap,
            &tmp.path().join("out"),
            &crate::config::RunConfig::default(),
        )
        .unwrap();
        assert!(!report.entries[0].hotspots.is_empty());
        let source = tmp.path().join("intent.json");
        let mut intent = VisualIntent {
            required_effects: Vec::new(),
            schema: SCHEMA.into(),
            objective: "brighten patch".into(),
            no_change_elsewhere: true,
            changes: vec![ExpectedChange {
                entry: "a.png".into(),
                kind: ChangeKind::Structure,
                rect_frac: Some([0.0, 0.0, 0.5, 0.5]),
                mask: None,
            }],
        };
        intent.validate(&source).unwrap();
        let found = verify(&intent, &source, &report);
        assert_eq!(found.matched.len(), 1);
        assert!(found.unexpected.is_empty(), "{:?}", found.unexpected);
        intent.changes[0].rect_frac = Some([0.5, 0.5, 0.5, 0.5]);
        let found = verify(&intent, &source, &report);
        assert_eq!(found.missing.len(), 1);
        assert_eq!(found.unexpected.len(), 1);

        let mut thin = report.clone();
        thin.entries[0].hotspots[0].rect_px = [0, 0, 64, 64];
        thin.entries[0].hotspots[0].pixel_runs = vec![[0, 1], [63 * 64 + 63, 1]];
        thin.entries[0].changed_pixel_runs = thin.entries[0].hotspots[0].pixel_runs.clone();
        intent.changes[0].rect_frac = Some([0.4, 0.4, 0.2, 0.2]);
        let found = verify(&intent, &source, &thin);
        assert!(
            found.matched.is_empty(),
            "box overlap cannot establish a pixel match"
        );
        assert_eq!(found.missing.len(), 1);
    }
}
