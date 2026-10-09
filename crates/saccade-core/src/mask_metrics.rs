//! Deterministic overlap and boundary metrics between two integer label images.
//!
//! The metrics are model-independent and domain-neutral: any pair of label
//! images (a predicted segmentation and a reference, an object-ID buffer and an
//! expected ID buffer, a thresholded map and a hand-drawn region) is scored the
//! same way. Nothing is resampled; images of different sizes are refused.
//!
//! # Conventions
//!
//! * Labels are native integers. Single-channel 8/16-bit images use the sample
//!   value; RGB(A) images pack `R<<16 | G<<8 | B` (alpha ignored); EXR images
//!   use the red channel, which must be a non-negative integer.
//! * A *class* is a predicate over labels, in the shared mask-spec grammar
//!   (`NAME=id=1,2`, `NAME=range=1,5`, `NAME=above=0`, `NAME=mask`). Label-glob
//!   predicates need a dictionary and are refused. With no class declared the
//!   single class `foreground` is "label is not zero"; [`ClassSelection::EachLabel`](crate::mask_metrics::ClassSelection::EachLabel)
//!   makes every distinct non-void label its own class.
//! * *Void* pixels are those whose **reference** label matches the void
//!   predicate. They are removed from every count in both images, and a mask
//!   pixel next to a void pixel is not a boundary pixel on that side.
//! * IoU is `|P∩R| / |P∪R|` and Dice is `2|P∩R| / (|P|+|R|)`. A class empty in
//!   both images has no defined score: it is reported as `absent_in_both`
//!   with `null` scores and excluded from means. A class present in only one
//!   image scores `0` (`missed` when only the reference has it, `spurious` when
//!   only the prediction has it) and **stays in the macro mean**, so a missing
//!   rare class lowers the mean and is also named in `missed_classes`.
//! * The boundary of a mask is its pixels with a 4-neighbour inside the image,
//!   not void, and outside the mask. The image edge is not a boundary. The
//!   boundary precision is the share of predicted boundary pixels within
//!   `boundary_tolerance_px` (Euclidean, in pixels, inclusive) of a reference
//!   boundary pixel, recall is the converse, and the F-score is their harmonic
//!   mean. With no boundary on either side the score is `null`; with a boundary
//!   on one side only it is `0`.
use crate::{
    Error, Result,
    evidence_quality::layers::{Predicate, parse_layer_spec},
};
use serde::Serialize;
use std::path::Path;

/// Report discriminator.
pub const SCHEMA: &str = "saccade-mask-metrics.v1";
/// Largest accepted image, in pixels.
pub const MAX_PIXELS: u64 = 100_000_000;
/// Largest accepted boundary tolerance, in pixels.
pub const MAX_TOLERANCE_PX: f64 = 64.0;
/// Largest number of classes created by [`ClassSelection::EachLabel`](crate::mask_metrics::ClassSelection::EachLabel).
pub const MAX_EACH_LABEL: usize = 256;

/// A decoded integer label image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major native labels.
    pub values: Vec<u32>,
}

impl Labels {
    /// Build from raw values; the length must equal `width * height`.
    pub fn new(width: u32, height: u32, values: Vec<u32>) -> Result<Self> {
        if u64::from(width) * u64::from(height) != values.len() as u64
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(Error::Config("label image size is inconsistent".into()));
        }
        Ok(Self {
            width,
            height,
            values,
        })
    }
}

/// Decode a label image using the native-label conventions of this module.
pub fn read_labels(path: &Path) -> Result<Labels> {
    use image::DynamicImage as D;
    let image = image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    if u64::from(image.width()) * u64::from(image.height()) > MAX_PIXELS {
        return Err(Error::Config("label image exceeds 100 megapixels".into()));
    }
    let (width, height) = (image.width(), image.height());
    let exr = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exr"));
    let values = if exr {
        image
            .to_rgb32f()
            .pixels()
            .map(|p| {
                let v = p.0[0];
                if !v.is_finite() || !(0.0..=16_777_215.0).contains(&v) || v.fract() != 0.0 {
                    Err(Error::Config(format!(
                        "EXR label image {} has a non-integer or out-of-range value",
                        path.display()
                    )))
                } else {
                    Ok(v as u32)
                }
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        match &image {
            D::ImageLuma8(b) => b.pixels().map(|p| u32::from(p.0[0])).collect(),
            D::ImageLumaA8(b) => b.pixels().map(|p| u32::from(p.0[0])).collect(),
            D::ImageLuma16(b) => b.pixels().map(|p| u32::from(p.0[0])).collect(),
            D::ImageLumaA16(b) => b.pixels().map(|p| u32::from(p.0[0])).collect(),
            other => other
                .to_rgb8()
                .pixels()
                .map(|p| (u32::from(p.0[0]) << 16) | (u32::from(p.0[1]) << 8) | u32::from(p.0[2]))
                .collect(),
        }
    };
    Labels::new(width, height, values)
}

/// A named class defined by a label predicate.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassSpec {
    /// Report name, preserved as written.
    pub name: String,
    /// Validated predicate over native labels.
    pub predicate: Predicate,
}

impl ClassSpec {
    /// Parse `NAME=PREDICATE` with the shared mask-spec grammar.
    pub fn parse(spec: &str) -> Result<Self> {
        let (name, predicate) = parse_layer_spec(spec)?;
        if matches!(predicate, Predicate::Labels { .. }) {
            return Err(Error::Config(
                "label-glob predicates need a dictionary; use id=, range=, above= or mask".into(),
            ));
        }
        Ok(Self { name, predicate })
    }
}

/// Which classes are scored.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassSelection {
    /// One class named `foreground`: label is not zero.
    Foreground,
    /// Every distinct non-void label in either image, named `id=N`.
    EachLabel,
    /// Declared classes, in report order.
    Named(Vec<ClassSpec>),
}

/// Declared scoring policy.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Classes to score.
    pub classes: ClassSelection,
    /// Reference labels excluded from all counts.
    pub void: Option<ClassSpec>,
    /// Boundary match tolerance in pixels, inclusive.
    pub boundary_tolerance_px: f64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            classes: ClassSelection::Foreground,
            void: None,
            boundary_tolerance_px: 2.0,
        }
    }
}

/// Boundary agreement for one class.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Boundary {
    /// Reference boundary pixels.
    pub reference_pixels: u64,
    /// Predicted boundary pixels.
    pub predicted_pixels: u64,
    /// Share of predicted boundary pixels near a reference boundary; null when none.
    pub precision: Option<f64>,
    /// Share of reference boundary pixels near a predicted boundary; null when none.
    pub recall: Option<f64>,
    /// Harmonic mean; null when neither side has a boundary.
    pub f_score: Option<f64>,
}

/// Scores for one class.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassResult {
    /// Class name.
    pub name: String,
    /// `scored`, `missed`, `spurious` or `absent_in_both`.
    pub status: String,
    /// Reference pixels in the class, void excluded.
    pub reference_pixels: u64,
    /// Predicted pixels in the class, void excluded.
    pub predicted_pixels: u64,
    /// Pixels in both.
    pub intersection: u64,
    /// Pixels in either.
    pub union: u64,
    /// Intersection over union; null when absent in both.
    pub iou: Option<f64>,
    /// Dice coefficient; null when absent in both.
    pub dice: Option<f64>,
    /// Share of predicted pixels that are reference pixels; null without predictions.
    pub precision: Option<f64>,
    /// Share of reference pixels that are predicted; null without reference pixels.
    pub recall: Option<f64>,
    /// Boundary agreement; null when the class is absent in both or has no boundary at all.
    pub boundary: Option<Boundary>,
}

/// Aggregate over classes; never hides a missing class.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// `scored` or `empty_empty` (no class present in either image).
    pub state: String,
    /// Mean IoU over classes present in either image; null when none.
    pub macro_iou: Option<f64>,
    /// Mean Dice over the same classes.
    pub macro_dice: Option<f64>,
    /// Mean boundary F-score over those classes that have one.
    pub macro_boundary_f: Option<f64>,
    /// Summed intersection over summed union across present classes.
    pub micro_iou: Option<f64>,
    /// Lowest class IoU among present classes.
    pub min_iou: Option<f64>,
    /// Pixels with equal labels over evaluated pixels; null when nothing was evaluated.
    pub label_agreement: Option<f64>,
    /// Classes present in the reference but not predicted.
    pub missed_classes: Vec<String>,
    /// Classes predicted but absent from the reference.
    pub spurious_classes: Vec<String>,
    /// Classes empty in both images, excluded from means.
    pub absent_classes: Vec<String>,
}

/// Echo of the declared policy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PolicyRecord {
    /// `foreground`, `each_label` or `named`.
    pub classes: String,
    /// Void predicate as `NAME=PREDICATE` summary, if any.
    pub void: Option<String>,
    /// Boundary tolerance in pixels.
    pub boundary_tolerance_px: f64,
}

/// Full metrics report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Version discriminator.
    pub schema: String,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Declared policy.
    pub policy: PolicyRecord,
    /// Pixels excluded as void.
    pub void_pixels: u64,
    /// Pixels scored.
    pub evaluated_pixels: u64,
    /// Per-class scores.
    pub classes: Vec<ClassResult>,
    /// Aggregates.
    pub summary: Summary,
}

fn ratio(n: u64, d: u64) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

fn boundary_pixels(mask: &[bool], valid: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut out = vec![false; mask.len()];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !mask[i] || !valid[i] {
                continue;
            }
            let outside = |j: usize| valid[j] && !mask[j];
            out[i] = (x > 0 && outside(i - 1))
                || (x + 1 < w && outside(i + 1))
                || (y > 0 && outside(i - w))
                || (y + 1 < h && outside(i + w));
        }
    }
    out
}

fn near(from: &[bool], to: &[bool], w: usize, h: usize, offsets: &[(i64, i64)]) -> u64 {
    let mut hits = 0;
    for y in 0..h {
        for x in 0..w {
            if !from[y * w + x] {
                continue;
            }
            let found = offsets.iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && {
                    to[ny as usize * w + nx as usize]
                }
            });
            hits += u64::from(found);
        }
    }
    hits
}

fn boundary_agreement(
    p: &[bool],
    r: &[bool],
    valid: &[bool],
    (w, h): (usize, usize),
    offsets: &[(i64, i64)],
) -> Option<Boundary> {
    let pb = boundary_pixels(p, valid, w, h);
    let rb = boundary_pixels(r, valid, w, h);
    let pn = pb.iter().filter(|b| **b).count() as u64;
    let rn = rb.iter().filter(|b| **b).count() as u64;
    if pn == 0 && rn == 0 {
        return None;
    }
    let precision = ratio(near(&pb, &rb, w, h, offsets), pn);
    let recall = ratio(near(&rb, &pb, w, h, offsets), rn);
    let f_score = match (precision, recall) {
        (Some(a), Some(b)) if a + b > 0.0 => Some(2.0 * a * b / (a + b)),
        _ => Some(0.0),
    };
    Some(Boundary {
        reference_pixels: rn,
        predicted_pixels: pn,
        precision,
        recall,
        f_score,
    })
}

fn validate_policy(policy: &Policy) -> Result<()> {
    let t = policy.boundary_tolerance_px;
    if !t.is_finite() || !(0.0..=MAX_TOLERANCE_PX).contains(&t) {
        return Err(Error::Config(
            "boundary tolerance must be finite and between 0 and 64 pixels".into(),
        ));
    }
    if let ClassSelection::Named(list) = &policy.classes {
        if list.is_empty() || list.len() > MAX_EACH_LABEL {
            return Err(Error::Config("declare between 1 and 256 classes".into()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in list {
            c.predicate.validate()?;
            if !seen.insert(&c.name) {
                return Err(Error::Config(format!("duplicate class name {:?}", c.name)));
            }
        }
    }
    if let Some(v) = &policy.void {
        v.predicate.validate()?;
    }
    Ok(())
}

/// Score `predicted` against `reference`. Sizes must match; nothing is resampled.
pub fn evaluate(predicted: &Labels, reference: &Labels, policy: &Policy) -> Result<Report> {
    validate_policy(policy)?;
    if (predicted.width, predicted.height) != (reference.width, reference.height) {
        return Err(Error::Config(format!(
            "prediction is {}x{} but reference is {}x{}; masks are never resampled",
            predicted.width, predicted.height, reference.width, reference.height
        )));
    }
    let (w, h) = (reference.width as usize, reference.height as usize);
    let valid: Vec<bool> = reference
        .values
        .iter()
        .map(|v| {
            policy
                .void
                .as_ref()
                .is_none_or(|c| !c.predicate.matches(f64::from(*v)))
        })
        .collect();
    let void_pixels = valid.iter().filter(|v| !**v).count() as u64;
    let evaluated_pixels = valid.len() as u64 - void_pixels;
    let classes: Vec<ClassSpec> = match &policy.classes {
        ClassSelection::Foreground => vec![ClassSpec {
            name: "foreground".into(),
            predicate: Predicate::Mask,
        }],
        ClassSelection::Named(list) => list.clone(),
        ClassSelection::EachLabel => {
            let mut labels = std::collections::BTreeSet::new();
            for (i, ok) in valid.iter().enumerate() {
                if *ok {
                    labels.insert(reference.values[i]);
                    labels.insert(predicted.values[i]);
                }
            }
            if labels.len() > MAX_EACH_LABEL {
                return Err(Error::Config(format!(
                    "{} distinct labels exceed the per-label limit of {MAX_EACH_LABEL}; declare classes",
                    labels.len()
                )));
            }
            labels
                .into_iter()
                .map(|v| ClassSpec {
                    name: format!("id={v}"),
                    predicate: Predicate::Ids { values: vec![v] },
                })
                .collect()
        }
    };
    let tol = policy.boundary_tolerance_px;
    let reach = tol.floor() as i64;
    let offsets: Vec<(i64, i64)> = (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
        .filter(|(dx, dy)| ((dx * dx + dy * dy) as f64) <= tol * tol)
        .collect();
    let mut results = Vec::new();
    for class in &classes {
        let inside = |values: &[u32]| -> Vec<bool> {
            values
                .iter()
                .zip(&valid)
                .map(|(v, ok)| *ok && class.predicate.matches(f64::from(*v)))
                .collect()
        };
        let (p, r) = (inside(&predicted.values), inside(&reference.values));
        let pn = p.iter().filter(|b| **b).count() as u64;
        let rn = r.iter().filter(|b| **b).count() as u64;
        let inter = p.iter().zip(&r).filter(|(a, b)| **a && **b).count() as u64;
        let union = pn + rn - inter;
        let status = match (rn > 0, pn > 0) {
            (false, false) => "absent_in_both",
            (true, false) => "missed",
            (false, true) => "spurious",
            _ => "scored",
        };
        let present = union > 0;
        results.push(ClassResult {
            name: class.name.clone(),
            status: status.into(),
            reference_pixels: rn,
            predicted_pixels: pn,
            intersection: inter,
            union,
            iou: present.then(|| inter as f64 / union as f64),
            dice: present.then(|| 2.0 * inter as f64 / (pn + rn) as f64),
            precision: ratio(inter, pn),
            recall: ratio(inter, rn),
            boundary: if present {
                boundary_agreement(&p, &r, &valid, (w, h), &offsets)
            } else {
                None
            },
        });
    }
    let present: Vec<&ClassResult> = results.iter().filter(|c| c.union > 0).collect();
    let names = |status: &str| -> Vec<String> {
        results
            .iter()
            .filter(|c| c.status == status)
            .map(|c| c.name.clone())
            .collect()
    };
    let ious: Vec<f64> = present.iter().filter_map(|c| c.iou).collect();
    let dices: Vec<f64> = present.iter().filter_map(|c| c.dice).collect();
    let bf: Vec<f64> = present
        .iter()
        .filter_map(|c| c.boundary.as_ref().and_then(|b| b.f_score))
        .collect();
    let agree = predicted
        .values
        .iter()
        .zip(&reference.values)
        .zip(&valid)
        .filter(|((a, b), ok)| **ok && a == b)
        .count() as u64;
    let summary = Summary {
        state: if present.is_empty() {
            "empty_empty"
        } else {
            "scored"
        }
        .into(),
        macro_iou: mean(&ious),
        macro_dice: mean(&dices),
        macro_boundary_f: mean(&bf),
        micro_iou: ratio(
            present.iter().map(|c| c.intersection).sum(),
            present.iter().map(|c| c.union).sum(),
        ),
        min_iou: ious.iter().copied().reduce(f64::min),
        label_agreement: ratio(agree, evaluated_pixels),
        missed_classes: names("missed"),
        spurious_classes: names("spurious"),
        absent_classes: names("absent_in_both"),
    };
    Ok(Report {
        schema: SCHEMA.into(),
        width: reference.width,
        height: reference.height,
        policy: PolicyRecord {
            classes: match &policy.classes {
                ClassSelection::Foreground => "foreground",
                ClassSelection::EachLabel => "each_label",
                ClassSelection::Named(_) => "named",
            }
            .into(),
            void: policy.void.as_ref().map(|c| c.name.clone()),
            boundary_tolerance_px: tol,
        },
        void_pixels,
        evaluated_pixels,
        classes: results,
        summary,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn grid(w: u32, h: u32, f: impl Fn(u32, u32) -> u32) -> Labels {
        let values = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y));
        Labels::new(w, h, values.collect()).unwrap()
    }

    #[test]
    fn known_overlap_matches_hand_computation() {
        // 4x4 reference block at x 0..4; prediction shifted right by 2: inter 8, union 24.
        let r = grid(8, 4, |x, _| u32::from(x < 4));
        let p = grid(8, 4, |x, _| u32::from((2..6).contains(&x)));
        let c = &evaluate(&p, &r, &Policy::default()).unwrap().classes[0];
        assert_eq!((c.intersection, c.union), (8, 24));
        assert!((c.iou.unwrap() - 1.0 / 3.0).abs() < 1e-12);
        assert!((c.dice.unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn empty_cases_are_explicit_and_not_averaged_away() {
        let empty = grid(4, 4, |_, _| 0);
        let full = grid(4, 4, |_, _| 1);
        let r = evaluate(&empty, &empty, &Policy::default()).unwrap();
        assert_eq!(r.summary.state, "empty_empty");
        assert_eq!(r.classes[0].iou, None);
        assert_eq!(r.summary.absent_classes, ["foreground"]);
        let r = evaluate(&empty, &full, &Policy::default()).unwrap();
        assert_eq!(r.classes[0].status, "missed");
        assert_eq!(r.summary.macro_iou, Some(0.0));
        // A rare class missed entirely drags the macro mean and is named.
        let reference = grid(10, 1, |x, _| if x == 9 { 2 } else { 1 });
        let predicted = grid(10, 1, |_, _| 1);
        let policy = Policy {
            classes: ClassSelection::EachLabel,
            ..Policy::default()
        };
        let r = evaluate(&predicted, &reference, &policy).unwrap();
        assert_eq!(r.summary.missed_classes, ["id=2"]);
        assert!((r.summary.macro_iou.unwrap() - 0.45).abs() < 1e-12);
        assert!(evaluate(&empty, &grid(2, 2, |_, _| 0), &Policy::default()).is_err());
    }

    #[test]
    fn void_reference_pixels_leave_every_count() {
        let reference = grid(4, 1, |x, _| if x == 3 { 255 } else { u32::from(x < 2) });
        let predicted = grid(4, 1, |x, _| u32::from(x != 1));
        let policy = Policy {
            void: Some(ClassSpec::parse("ignore=id=255").unwrap()),
            ..Policy::default()
        };
        let r = evaluate(&predicted, &reference, &policy).unwrap();
        assert_eq!((r.void_pixels, r.evaluated_pixels), (1, 3));
        // Pixel 3 (predicted foreground, void in reference) is not a false positive.
        assert_eq!((r.classes[0].predicted_pixels, r.classes[0].union), (2, 3));
    }

    #[test]
    fn shifted_thin_boundary_needs_a_declared_tolerance() {
        let line = |col: u32| grid(9, 9, move |x, _| u32::from(x == col));
        let (r, p) = (line(4), line(5));
        let at = |tol: f64| {
            let policy = Policy {
                boundary_tolerance_px: tol,
                ..Policy::default()
            };
            evaluate(&p, &r, &policy).unwrap()
        };
        let strict = at(0.0);
        assert_eq!(strict.classes[0].iou, Some(0.0));
        assert_eq!(
            strict.classes[0].boundary.as_ref().unwrap().f_score,
            Some(0.0)
        );
        let loose = at(1.0);
        assert_eq!(loose.classes[0].iou, Some(0.0));
        assert_eq!(
            loose.classes[0].boundary.as_ref().unwrap().f_score,
            Some(1.0)
        );
        assert!(
            evaluate(
                &p,
                &r,
                &Policy {
                    boundary_tolerance_px: f64::NAN,
                    ..Policy::default()
                }
            )
            .is_err()
        );
    }
}
