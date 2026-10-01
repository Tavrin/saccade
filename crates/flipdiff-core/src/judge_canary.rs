//! Gold canaries: a small built-in set of generated image pairs with known
//! truths, mixed into judge runs so a judge that gets the easy cases wrong is
//! flagged and down-weighted instead of trusted.
//!
//! The pairs are generated, never stored: a tiny scene (sky, ground, a lit
//! sphere with a shadow) and four edits of it, so the set is public data by
//! construction and can be sent to any provider.

use image::{Rgb, RgbImage};

use crate::decision::Question;

/// Side of a canary image, in pixels.
const SIZE: u32 = 128;

/// One known-answer item.
#[derive(Debug, Clone)]
pub struct Canary {
    /// What the pair is.
    pub label: &'static str,
    /// A neutral file name the judge sees (it must not give the truth away).
    pub name: String,
    /// The question asked.
    pub question: Question,
    /// The correct answer.
    pub truth: &'static str,
    /// The intent text shown with the pair, when the question uses one.
    pub intent: Option<&'static str>,
    /// The reference image.
    pub baseline: RgbImage,
    /// The candidate image.
    pub capture: RgbImage,
}

/// A small deterministic generator (no dependency, stable across platforms).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
}

fn scene(shadow: f32) -> RgbImage {
    let mut img = RgbImage::new(SIZE, SIZE);
    let horizon = SIZE * 11 / 20;
    let (cx, cy, r) = (64.0f32, 52.0f32, 22.0f32);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fy) = (x as f32, y as f32);
            let mut c = if y < horizon {
                let t = fy / horizon as f32;
                [90.0 + 90.0 * t, 140.0 + 70.0 * t, 220.0 + 20.0 * t]
            } else {
                [96.0, 128.0 - (fy - horizon as f32) * 0.3, 78.0]
            };
            let (ex, ey) = ((fx - (cx + 20.0)) / 34.0, (fy - (cy + 36.0)) / 8.0);
            if ex * ex + ey * ey < 1.0 && y >= horizon {
                for v in &mut c {
                    *v *= 1.0 - shadow * (1.0 - (ex * ex + ey * ey).sqrt() * 0.5);
                }
            }
            let (dx, dy) = (fx - cx, fy - cy);
            if dx * dx + dy * dy < r * r {
                let light = (1.0 - ((dx + 8.0).powi(2) + (dy + 8.0).powi(2)).sqrt() / (r * 1.5))
                    .clamp(0.15, 1.0);
                c = [230.0 * light, 80.0 * light, 70.0 * light];
            }
            img.put_pixel(x, y, Rgb(c.map(|v| v.clamp(0.0, 255.0) as u8)));
        }
    }
    img
}

/// The built-in gold set, in a fixed order: identical, an obvious shadow
/// change, pure noise and a global tone shift.
pub fn gold_set() -> Vec<Canary> {
    let base = scene(0.55);
    let no_shadow = scene(0.0);
    let mut rng = Lcg(0x5EED);
    let mut noisy = base.clone();
    for p in noisy.pixels_mut() {
        for v in &mut p.0 {
            let n = (rng.next() % 3) as i32 - 1;
            *v = (i32::from(*v) + n).clamp(0, 255) as u8;
        }
    }
    let mut darker = base.clone();
    for p in darker.pixels_mut() {
        for v in &mut p.0 {
            *v = (f32::from(*v) * 0.72) as u8;
        }
    }
    let refactor = Some("Refactor of the asset loader. No visual change is intended.");
    vec![
        Canary {
            label: "identical",
            name: "view_0412.png".into(),
            question: Question::Accept,
            truth: "accept",
            intent: refactor,
            baseline: base.clone(),
            capture: base.clone(),
        },
        Canary {
            label: "obvious shadow change",
            name: "view_0873.png".into(),
            question: Question::Accept,
            truth: "reject",
            intent: refactor,
            baseline: base.clone(),
            capture: no_shadow,
        },
        Canary {
            label: "pure noise",
            name: "view_0159.png".into(),
            question: Question::Triage,
            truth: "noise",
            intent: None,
            baseline: base.clone(),
            capture: noisy,
        },
        Canary {
            label: "tone shift",
            name: "view_0634.png".into(),
            question: Question::Triage,
            truth: "global_shift",
            intent: None,
            baseline: base,
            capture: darker,
        },
    ]
}

/// How many canaries to mix into a run of `real_items` items at `rate`
/// (a fraction of the real items, rounded up, at least one when `rate > 0`,
/// never more than the built-in set).
pub fn canary_count(real_items: usize, rate: f64) -> usize {
    if rate <= 0.0 || real_items == 0 {
        return 0;
    }
    ((real_items as f64 * rate).ceil() as usize).clamp(1, 4)
}

/// A judge's record on the canaries it answered.
#[derive(Debug, Clone, PartialEq)]
pub struct CanaryScore {
    /// Right answers.
    pub correct: usize,
    /// Wrong, non-abstaining answers.
    pub wrong: usize,
    /// Abstentions (not counted for or against).
    pub abstained: usize,
    /// `correct / (correct + wrong)`; `None` when the judge never committed.
    pub accuracy: Option<f64>,
    /// Whether the judge failed the canaries.
    pub flagged: bool,
    /// The factor applied to the judge's weight (1 unless flagged).
    pub weight_factor: f64,
}

/// Scores a judge from its `(answer, truth)` pairs; an answer of `None` or an
/// abstention answer is an abstention. A judge is flagged when its accuracy
/// on committed answers is below `pass` after at least two of them, or is 0
/// after one; its weight is then multiplied by `accuracy^2` (at least 0.05).
pub fn score(answers: &[(Option<&str>, &str)], pass: f64) -> CanaryScore {
    let (mut correct, mut wrong, mut abstained) = (0, 0, 0);
    for (a, truth) in answers {
        match a {
            Some(a) if !crate::judge_stats::is_abstain(a) => {
                if a == truth {
                    correct += 1;
                } else {
                    wrong += 1;
                }
            }
            _ => abstained += 1,
        }
    }
    let n = correct + wrong;
    let accuracy = (n > 0).then(|| correct as f64 / n as f64);
    let flagged = accuracy.is_some_and(|a| a < pass && (n >= 2 || a == 0.0));
    let weight_factor = match (flagged, accuracy) {
        (true, Some(a)) => (a * a).max(0.05),
        _ => 1.0,
    };
    CanaryScore {
        correct,
        wrong,
        abstained,
        accuracy,
        flagged,
        weight_factor,
    }
}
