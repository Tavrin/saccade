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
                if path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    || !source
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(path)
                        .is_file()
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

fn overlaps(change: &ExpectedChange, source: &Path, entry: &Entry, rect: [u32; 4]) -> bool {
    let Some((width, height)) = dimensions(entry) else {
        return false;
    };
    if let Some([rx, ry, rw, rh]) = change.rect_frac {
        let [x, y, w, h] = rect;
        return f64::from(x) / f64::from(width) < rx + rw
            && f64::from(x + w) / f64::from(width) > rx
            && f64::from(y) / f64::from(height) < ry + rh
            && f64::from(y + h) / f64::from(height) > ry;
    }
    let Some(mask) = &change.mask else {
        return false;
    };
    let path = source.parent().unwrap_or(Path::new(".")).join(mask);
    let Ok(image) = image::open(path) else {
        return false;
    };
    let image = image.to_luma8();
    if image.width() != width || image.height() != height {
        return false;
    }
    (rect[1]..rect[1] + rect[3])
        .any(|y| (rect[0]..rect[0] + rect[2]).any(|x| image.get_pixel(x, y).0[0] > 0))
}

fn contains(change: &ExpectedChange, source: &Path, entry: &Entry, rect: [u32; 4]) -> bool {
    let Some((width, height)) = dimensions(entry) else {
        return false;
    };
    if let Some([rx, ry, rw, rh]) = change.rect_frac {
        let [x, y, w, h] = rect;
        return f64::from(x) / f64::from(width) >= rx
            && f64::from(x + w) / f64::from(width) <= rx + rw
            && f64::from(y) / f64::from(height) >= ry
            && f64::from(y + h) / f64::from(height) <= ry + rh;
    }
    let Some(mask) = &change.mask else {
        return false;
    };
    let path = source.parent().unwrap_or(Path::new(".")).join(mask);
    let Ok(image) = image::open(path) else {
        return false;
    };
    let image = image.to_luma8();
    if image.width() != width || image.height() != height {
        return false;
    }
    (rect[1]..rect[1] + rect[3])
        .all(|y| (rect[0]..rect[0] + rect[2]).all(|x| image.get_pixel(x, y).0[0] > 0))
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
        let hits = entry
            .hotspots
            .iter()
            .filter(|h| overlaps(change, source, entry, h.rect_px))
            .count();
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
                "{hits} overlapping hotspots; tone exposure {tone:?} stops; native identical {:?}",
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
            for (index, hotspot) in entry.hotspots.iter().enumerate() {
                if !declared
                    .iter()
                    .any(|c| contains(c, source, entry, hotspot.rect_px))
                {
                    result.unexpected.push(Finding {
                        entry: entry.name.clone(),
                        kind: "hotspot_elsewhere".into(),
                        detail: format!("hotspot {} at {:?}", index + 1, hotspot.rect_px),
                    });
                }
            }
            if entry.bit_identical == Some(false)
                && entry.hotspots.is_empty()
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
    }
}
