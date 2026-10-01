//! The statistics of judge mode: calibration (accuracy, agreement, expected
//! calibration error, reliability table), the human-human ceiling
//! (Krippendorff's alpha), position bias from both-orders calls, and
//! Bradley-Terry rankings with bootstrap confidence intervals.
//!
//! Everything here is plain arithmetic over already-collected answers; it
//! makes no calls and reads no files except the decisions files handed to
//! [`calibrate`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::error::Error;
use crate::view::{Decisions, read_decisions};

/// Schema identifier of a calibration file.
pub const CALIBRATION_SCHEMA: &str = "saccade-calibration.v1";

/// Equal-width bins of the reliability table.
pub const BINS: usize = 10;

/// Answers that abstain: they are neither right nor wrong.
pub fn is_abstain(answer: &str) -> bool {
    matches!(answer, "unsure" | "needs_human")
}

/// Whether a decisions-file source names a person: `human` or `human:<name>`.
pub fn is_human_source(source: &str) -> bool {
    let s = source.trim().to_ascii_lowercase();
    s == "human" || s.starts_with("human:")
}

fn r4(v: f64) -> Value {
    if v.is_finite() {
        json!((v * 1e4).round() / 1e4)
    } else {
        Value::Null
    }
}

// ---- calibration -----------------------------------------------------------

/// One prediction with a stated confidence and whether it was right.
#[derive(Debug, Clone, Copy)]
pub struct Scored {
    /// The confidence in `[0, 1]`.
    pub conf: f64,
    /// Whether the answer matched the reference label.
    pub correct: bool,
}

/// One row of a reliability table.
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    /// Lower bound (inclusive).
    pub lo: f64,
    /// Upper bound (exclusive, except the last bin).
    pub hi: f64,
    /// Predictions in the bin.
    pub n: usize,
    /// Mean stated confidence.
    pub mean_conf: f64,
    /// Fraction that was right.
    pub accuracy: f64,
}

/// The reliability table of `items` over [`BINS`] equal-width bins (empty bins omitted).
pub fn reliability(items: &[Scored]) -> Vec<Bin> {
    let mut acc = vec![(0usize, 0.0f64, 0usize); BINS];
    for s in items {
        let i = ((s.conf.clamp(0.0, 1.0) * BINS as f64) as usize).min(BINS - 1);
        acc[i].0 += 1;
        acc[i].1 += s.conf;
        acc[i].2 += usize::from(s.correct);
    }
    acc.iter()
        .enumerate()
        .filter(|(_, (n, _, _))| *n > 0)
        .map(|(i, (n, c, k))| Bin {
            lo: i as f64 / BINS as f64,
            hi: (i + 1) as f64 / BINS as f64,
            n: *n,
            mean_conf: c / *n as f64,
            accuracy: *k as f64 / *n as f64,
        })
        .collect()
}

/// Expected calibration error: the bin-weighted mean gap between stated
/// confidence and accuracy. `None` for no predictions.
pub fn ece(items: &[Scored]) -> Option<f64> {
    if items.is_empty() {
        return None;
    }
    let n = items.len() as f64;
    Some(
        reliability(items)
            .iter()
            .map(|b| b.n as f64 / n * (b.accuracy - b.mean_conf).abs())
            .sum(),
    )
}

/// Krippendorff's alpha for nominal data. `units` holds, per unit, the labels
/// the raters gave (missing ratings are simply absent). Units with fewer than
/// two labels carry no information and are ignored. `None` when no unit has
/// two labels or every label is the same (no variation to agree about).
pub fn krippendorff_alpha(units: &[Vec<String>]) -> Option<f64> {
    let mut o: BTreeMap<(&str, &str), f64> = BTreeMap::new();
    for u in units.iter().filter(|u| u.len() >= 2) {
        let w = 1.0 / (u.len() as f64 - 1.0);
        for (i, a) in u.iter().enumerate() {
            for (j, b) in u.iter().enumerate() {
                if i != j {
                    *o.entry((a.as_str(), b.as_str())).or_insert(0.0) += w;
                }
            }
        }
    }
    let mut nc: BTreeMap<&str, f64> = BTreeMap::new();
    for (&(c, _), v) in &o {
        *nc.entry(c).or_insert(0.0) += v;
    }
    let n: f64 = nc.values().sum();
    if n <= 1.0 || nc.len() < 2 {
        return None;
    }
    let observed: f64 = o.iter().filter(|((a, b), _)| a != b).map(|(_, v)| v).sum();
    let mut expected = 0.0;
    for (a, na) in &nc {
        for (b, nb) in &nc {
            if a != b {
                expected += na * nb;
            }
        }
    }
    Some(1.0 - (n - 1.0) * observed / expected)
}

/// One prediction found in a decisions file.
#[derive(Debug, Clone)]
struct Pred {
    answer: String,
    prob: Option<f64>,
}

/// Options of [`calibrate`].
#[derive(Debug, Clone)]
pub struct CalibrateOptions {
    /// The accuracy a suggested gate threshold must reach.
    pub target_accuracy: f64,
    /// Fewest predictions at or above a threshold for it to be suggested.
    pub min_support: usize,
}

impl Default for CalibrateOptions {
    fn default() -> Self {
        Self {
            target_accuracy: 0.95,
            min_support: 10,
        }
    }
}

/// The smallest probability threshold at which a judge's non-abstaining
/// predictions reach `target` accuracy with at least `min_support` of them.
/// Returns `(threshold, accuracy, coverage_fraction)`.
pub fn suggest_threshold(
    items: &[Scored],
    target: f64,
    min_support: usize,
) -> Option<(f64, f64, f64)> {
    let mut sorted: Vec<Scored> = items.to_vec();
    sorted.sort_by(|a, b| a.conf.total_cmp(&b.conf));
    let mut best = None;
    for (i, s) in sorted.iter().enumerate() {
        if i > 0 && s.conf == sorted[i - 1].conf {
            continue;
        }
        let tail = &sorted[i..];
        if tail.len() < min_support {
            break;
        }
        let acc = tail.iter().filter(|t| t.correct).count() as f64 / tail.len() as f64;
        if acc >= target {
            best = Some((s.conf, acc, tail.len() as f64 / sorted.len() as f64));
            break;
        }
    }
    best
}

#[derive(Default)]
struct UnitData {
    humans: BTreeMap<String, String>,
    preds: BTreeMap<(String, String), Pred>,
    question: String,
}

fn unit_key(
    d: &Decisions,
    set: &crate::view::SetDecision,
    question: &str,
    hotspot: Option<u32>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        d.dirs.join(","),
        set.name,
        set.sha256
            .iter()
            .map(|h| h.as_deref().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(","),
        question,
        hotspot.map_or(String::new(), |h| h.to_string())
    )
}

fn collect(files: &[PathBuf]) -> Result<BTreeMap<String, UnitData>, Error> {
    let mut units: BTreeMap<String, UnitData> = BTreeMap::new();
    for file in files {
        let d = read_decisions(file)?;
        let stem = file
            .file_name()
            .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
        for set in &d.sets {
            let mut human_seen_accept = false;
            for p in &set.proposals {
                let key = unit_key(&d, set, &p.question, p.hotspot);
                let u = units.entry(key).or_default();
                u.question.clone_from(&p.question);
                if is_human_source(&p.source) {
                    if p.question == "accept" {
                        human_seen_accept = true;
                    }
                    u.humans
                        .insert(p.source.trim().to_owned(), p.answer.clone());
                } else {
                    u.preds
                        .entry((p.source.trim().to_owned(), p.question.clone()))
                        .or_insert(Pred {
                            answer: p.answer.clone(),
                            prob: p.prob,
                        });
                }
            }
            if let (Some(v), false) = (set.decision, human_seen_accept) {
                // An automatically promoted model prediction is not a human final.
                if set.proposals.iter().any(|p| p.promoted) {
                    continue;
                }
                let key = unit_key(&d, set, "accept", None);
                let u = units.entry(key).or_default();
                u.question = "accept".into();
                let label = serde_json::to_value(v)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                if matches!(label.as_str(), "accept" | "reject") {
                    u.humans.insert(format!("decision:{stem}"), label);
                }
            }
        }
    }
    Ok(units)
}

/// The majority label of a unit's human raters; `None` on a tie or no labels.
fn consensus(humans: &BTreeMap<String, String>) -> Option<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for v in humans.values() {
        *counts.entry(v).or_insert(0) += 1;
    }
    let max = counts.values().copied().max()?;
    let mut top = counts.iter().filter(|(_, c)| **c == max);
    let first = top.next()?;
    top.next().is_none().then(|| (*first.0).to_owned())
}

/// Builds the `saccade-calibration.v1` document from decisions files that
/// hold human final decisions (and the judges' proposals on the same sets).
///
/// `runs` are optional `saccade-judge.v1` result files; they add each
/// judge's position bias, which a decisions file does not carry.
pub fn calibrate(
    files: &[PathBuf],
    runs: &[PathBuf],
    opts: &CalibrateOptions,
) -> Result<Value, Error> {
    if !opts.target_accuracy.is_finite()
        || !(0.0..=1.0).contains(&opts.target_accuracy)
        || opts.min_support == 0
    {
        return Err(Error::Config(
            "calibration needs target_accuracy in [0, 1] and min_support above zero".into(),
        ));
    }
    let units = collect(files)?;
    type CalibrationCounts = (Vec<Scored>, usize, usize, usize, usize);
    let mut by_judge: BTreeMap<(String, String), CalibrationCounts> = BTreeMap::new();
    // (scored, abstained, agree_pairs, agree_hits, unlabelled)
    let mut ceilings: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut labelled_units = 0usize;
    for u in units.values() {
        if u.humans.len() >= 2 {
            let labels: Vec<String> = u.humans.values().cloned().collect();
            ceilings
                .entry(u.question.clone())
                .or_default()
                .push(labels.clone());
            ceilings.entry("all".into()).or_default().push(labels);
        }
        let truth = consensus(&u.humans);
        if truth.is_some() {
            labelled_units += 1;
        }
        for ((source, question), p) in &u.preds {
            let slot = by_judge
                .entry((source.clone(), question.clone()))
                .or_default();
            let Some(truth) = &truth else {
                slot.4 += 1;
                continue;
            };
            if is_abstain(&p.answer) {
                slot.1 += 1;
                continue;
            }
            if let Some(conf) = p.prob {
                slot.0.push(Scored {
                    conf,
                    correct: p.answer == *truth,
                });
            } else {
                slot.0.push(Scored {
                    conf: 1.0,
                    correct: p.answer == *truth,
                });
            }
            for label in u.humans.values() {
                slot.2 += 1;
                slot.3 += usize::from(&p.answer == label);
            }
        }
    }
    let bias = position_bias_from_runs(runs)?;
    let mut judges = Vec::new();
    for ((source, question), (scored, abstained, pairs, hits, unlabelled)) in &by_judge {
        let n = scored.len();
        let acc = (n > 0).then(|| scored.iter().filter(|s| s.correct).count() as f64 / n as f64);
        let suggestion = suggest_threshold(scored, opts.target_accuracy, opts.min_support);
        judges.push(json!({
            "judge": source,
            "question": question,
            "n": n,
            "abstained": abstained,
            "unlabelled": unlabelled,
            "accuracy": acc.map_or(Value::Null, r4),
            "agreement_with_humans": if *pairs > 0 { r4(*hits as f64 / *pairs as f64) } else { Value::Null },
            "ece": ece(scored).map_or(Value::Null, r4),
            "reliability": reliability(scored).iter().map(|b| json!({
                "lo": b.lo, "hi": b.hi, "n": b.n,
                "mean_confidence": r4(b.mean_conf), "accuracy": r4(b.accuracy),
            })).collect::<Vec<_>>(),
            "position_bias": bias.get(source).cloned().unwrap_or(Value::Null),
            "suggested_gate": match suggestion {
                Some((t, a, c)) => json!({
                    "min_prob": r4(t), "accuracy_at_threshold": r4(a), "coverage": r4(c),
                    "target_accuracy": opts.target_accuracy, "min_support": opts.min_support,
                }),
                None => Value::Null,
            },
            "note": if n < opts.min_support {
                format!("only {n} labelled predictions; no gate threshold is suggested below {}", opts.min_support)
            } else {
                String::new()
            },
        }));
    }
    let ceiling: Vec<Value> = ceilings
        .iter()
        .map(|(q, units)| {
            let raters = units.iter().map(Vec::len).max().unwrap_or(0);
            json!({
                "question": q,
                "units": units.len(),
                "max_raters": raters,
                "krippendorff_alpha": krippendorff_alpha(units).map_or(Value::Null, r4),
            })
        })
        .collect();
    Ok(json!({
        "schema": CALIBRATION_SCHEMA,
        "created_at_unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        "labels": files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>(),
        "labelled_units": labelled_units,
        "judges": judges,
        "human_ceiling": ceiling,
        "notes": [
            "accuracy is measured against the majority of the human labels; a unit with tied humans is unlabelled",
            "the human-human ceiling is nominal inter-rater agreement (Krippendorff's alpha), not a bound on objective accuracy; it is undefined without label variation",
            "ECE of a verbalised probability that was never calibrated is usually large; trust the reliability table, not the stated number",
        ],
    }))
}

// ---- position bias -----------------------------------------------------------

/// One item answered in both orders by one judge.
#[derive(Debug, Clone, Default)]
pub struct PairObs {
    /// The answer in the A/B order, in the stable answer space; `None` abstained.
    pub ab: Option<String>,
    /// The answer in the B/A order, in the stable answer space.
    pub ba: Option<String>,
    /// For pairwise questions: the raw slot answer (`P1` or `P2`) given in the A/B order.
    pub ab_slot: Option<String>,
    /// The raw slot answer given in the B/A order.
    pub ba_slot: Option<String>,
}

/// How much a judge's answers depend on the order of presentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Bias {
    /// Items answered in both orders without abstaining.
    pub items: usize,
    /// Of those, items whose answer changed with the order.
    pub flips: usize,
    /// `flips / items`.
    pub flip_rate: Option<f64>,
    /// Pairwise questions only: the share of slot answers that chose the
    /// first position (0.5 is unbiased).
    pub first_slot_rate: Option<f64>,
    /// Items with an abstention in at least one order.
    pub abstained: usize,
}

/// Computes the position bias of one judge.
pub fn position_bias(obs: &[PairObs]) -> Bias {
    let (mut items, mut flips, mut abstained) = (0, 0, 0);
    let (mut first, mut slots) = (0usize, 0usize);
    for o in obs {
        match (&o.ab, &o.ba) {
            (Some(a), Some(b)) => {
                items += 1;
                flips += usize::from(a != b);
            }
            _ => abstained += 1,
        }
        for s in [&o.ab_slot, &o.ba_slot].into_iter().flatten() {
            if s == "P1" || s == "P2" {
                slots += 1;
                first += usize::from(s == "P1");
            }
        }
    }
    Bias {
        items,
        flips,
        flip_rate: (items > 0).then(|| flips as f64 / items as f64),
        first_slot_rate: (slots > 0).then(|| first as f64 / slots as f64),
        abstained,
    }
}

impl Bias {
    /// The JSON form used in results and calibration files.
    pub fn value(&self) -> Value {
        json!({
            "items": self.items,
            "flips": self.flips,
            "flip_rate": self.flip_rate.map_or(Value::Null, r4),
            "first_slot_rate": self.first_slot_rate.map_or(Value::Null, r4),
            "abstained": self.abstained,
        })
    }
}

fn position_bias_from_runs(runs: &[PathBuf]) -> Result<BTreeMap<String, Value>, Error> {
    let mut out = BTreeMap::new();
    for path in runs {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            context: format!("reading {}", path.display()),
            source,
        })?;
        let v: Value = serde_json::from_str(&text)?;
        for b in v["position_bias"].as_array().into_iter().flatten() {
            if let Some(j) = b["judge"].as_str() {
                out.insert(j.to_owned(), b.clone());
            }
        }
    }
    Ok(out)
}

// ---- Bradley-Terry ---------------------------------------------------------------

/// One pairwise vote: `(i, j, score)` where `score` is 1 when `i` won, 0 when
/// `j` won and 0.5 for a tie.
pub type Vote = (usize, usize, f64);

/// Bradley-Terry strengths with bootstrap confidence intervals.
#[derive(Debug, Clone)]
pub struct BtResult {
    /// Log-strength per candidate, centred on 0.
    pub strength: Vec<f64>,
    /// 95% interval of each strength (bootstrap percentile).
    pub ci: Vec<(f64, f64)>,
    /// 95% interval of each candidate's rank (1 is best).
    pub rank_ci: Vec<(usize, usize)>,
    /// Comparisons each candidate took part in.
    pub comparisons: Vec<usize>,
    /// Why the ranking may not be trusted.
    pub warnings: Vec<String>,
}

/// Fewest comparisons per candidate before a ranking carries no warning.
pub const MIN_COMPARISONS: usize = 5;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Minorisation-maximisation fit with a weak prior (a tenth of a tie against
/// every other candidate), so a perfect record stays finite.
fn fit(n: usize, votes: &[Vote]) -> Vec<f64> {
    let prior = 0.1;
    let mut wins = vec![0.0f64; n];
    let mut games = vec![vec![0.0f64; n]; n];
    for &(i, j, s) in votes {
        if i >= n || j >= n || i == j {
            continue;
        }
        wins[i] += s;
        wins[j] += 1.0 - s;
        games[i][j] += 1.0;
        games[j][i] += 1.0;
    }
    for (i, row) in games.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            if i != j {
                *value += 2.0 * prior;
            }
        }
        wins[i] += prior * (n as f64 - 1.0);
    }
    let mut p = vec![1.0f64; n];
    for _ in 0..500 {
        let mut next = p.clone();
        for i in 0..n {
            let denom: f64 = (0..n)
                .filter(|&j| j != i)
                .map(|j| games[i][j] / (p[i] + p[j]))
                .sum();
            if denom > 0.0 {
                next[i] = wins[i] / denom;
            }
        }
        let log_mean = next.iter().map(|v| v.ln()).sum::<f64>() / n as f64;
        for v in &mut next {
            *v /= log_mean.exp();
        }
        let delta = p
            .iter()
            .zip(&next)
            .map(|(a, b)| (a.ln() - b.ln()).abs())
            .fold(0.0, f64::max);
        p = next;
        if delta < 1e-10 {
            break;
        }
    }
    p.iter().map(|v| v.ln()).collect()
}

fn ranks_of(strength: &[f64]) -> Vec<usize> {
    strength
        .iter()
        .map(|s| 1 + strength.iter().filter(|o| *o > s).count())
        .collect()
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Fits Bradley-Terry strengths for `n` candidates from pairwise `votes` and
/// bootstraps (`reps` resamples of the votes, deterministic from `seed`) the
/// 95% intervals of strengths and ranks.
pub fn bradley_terry(n: usize, votes: &[Vote], reps: usize, seed: u64) -> BtResult {
    let strength = fit(n, votes);
    let mut comparisons = vec![0usize; n];
    for &(i, j, _) in votes {
        if i < n && j < n && i != j {
            comparisons[i] += 1;
            comparisons[j] += 1;
        }
    }
    let mut rng = Rng(seed | 1);
    let mut s_samples: Vec<Vec<f64>> = vec![Vec::new(); n];
    let mut r_samples: Vec<Vec<f64>> = vec![Vec::new(); n];
    if !votes.is_empty() {
        for _ in 0..reps {
            let sample: Vec<Vote> = (0..votes.len())
                .map(|_| votes[(rng.next() % votes.len() as u64) as usize])
                .collect();
            let s = fit(n, &sample);
            for (i, rank) in ranks_of(&s).into_iter().enumerate() {
                s_samples[i].push(s[i]);
                r_samples[i].push(rank as f64);
            }
        }
    }
    let mut ci = Vec::new();
    let mut rank_ci = Vec::new();
    for i in 0..n {
        s_samples[i].sort_by(f64::total_cmp);
        r_samples[i].sort_by(f64::total_cmp);
        ci.push((
            percentile(&s_samples[i], 0.025),
            percentile(&s_samples[i], 0.975),
        ));
        rank_ci.push((
            percentile(&r_samples[i], 0.025) as usize,
            percentile(&r_samples[i], 0.975) as usize,
        ));
    }
    let mut warnings = Vec::new();
    let thin: Vec<usize> = (0..n)
        .filter(|&i| comparisons[i] < MIN_COMPARISONS)
        .collect();
    if !thin.is_empty() {
        warnings.push(format!(
            "too few votes: candidate(s) {} took part in fewer than {MIN_COMPARISONS} comparisons; the intervals are wide or meaningless",
            thin.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")
        ));
    }
    if !connected(n, votes) {
        warnings.push("the comparison graph is not connected: candidates in different components cannot be ordered against each other".into());
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| strength[*b].total_cmp(&strength[*a]));
    for w in order.windows(2) {
        let (hi, lo) = (w[0], w[1]);
        if !votes.is_empty() && ci[lo].1 >= ci[hi].0 {
            warnings.push(format!(
                "candidates {hi} and {lo} are statistically indistinguishable (their 95% intervals overlap)"
            ));
        }
    }
    BtResult {
        strength,
        ci,
        rank_ci,
        comparisons,
        warnings,
    }
}

fn connected(n: usize, votes: &[Vote]) -> bool {
    if n <= 1 {
        return true;
    }
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for &(i, j, _) in votes {
        if i < n && j < n {
            let (a, b) = (root(&mut parent, i), root(&mut parent, j));
            parent[a] = b;
        }
    }
    let r0 = root(&mut parent, 0);
    (1..n).all(|i| root(&mut parent, i) == r0)
}
