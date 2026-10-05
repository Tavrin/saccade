//! Exact penalised segmentation of short, qualified performance histories.
use crate::evidence::analysis::{Analysis, Capability, Provenance, Resource};
use serde::{Deserialize, Serialize};

/// One independent qualified observation of a declared timing statistic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Observation {
    /// Content hash of the recorded report, identifying the run.
    pub run: String,
    /// Source revision if unambiguously supplied by capture metadata.
    pub commit: Option<String>,
    /// Observation time from the producer; gaps are retained, never interpolated.
    pub time: u64,
    /// Qualified performance comparison identity (hardware, timer, configuration, statistic).
    pub identity: String,
    /// Exact sample-window identity; duplicates are not independent repeats.
    pub window: String,
    /// Timing statistic in milliseconds, strictly positive.
    pub milliseconds: f64,
    /// Qualified repeat range in milliseconds.
    pub repeat_range_ms: f64,
    /// Effective absolute/noise/resolution floor from the comparison.
    pub floor_ms: f64,
    /// Relative materiality floor, in percent.
    pub min_delta_pct: f64,
}

/// Retrospective onset candidate; confirmation requires fresh qualified repeats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Candidate {
    /// Always candidate; segmentation cannot prove commit causality.
    pub status: String,
    /// Last observation in the preceding segment, not proof of correctness.
    pub last_before: Observation,
    /// First observation in the sustained higher segment.
    pub first_changed: Observation,
    /// Number of observations in the preceding segment.
    pub before_count: usize,
    /// Number of observations in the changed segment.
    pub after_count: usize,
    /// Segment median before the onset.
    pub before_ms: f64,
    /// Segment median after the onset.
    pub after_ms: f64,
    /// Increase in milliseconds.
    pub effect_ms: f64,
    /// Increase as percent of the preceding median.
    pub effect_pct: f64,
    /// Maximum applicable materiality bound across the two segments.
    pub materiality_ms: f64,
    /// Whether the recommended 10-before/5-after count is present; not confirmation.
    pub retrospective_counts_met: bool,
}

/// Complete numerical witness for exact short-history segmentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Onset {
    /// Ordered observations used, including timestamps and input identities.
    pub observations: Vec<Observation>,
    /// End-exclusive segment boundaries, including the series end.
    pub segment_ends: Vec<usize>,
    /// Minimum observations per segment (five).
    pub minimum_segment: usize,
    /// Fixed robust scale applied to all log timings.
    pub log_noise_scale: f64,
    /// Estimated positive lag-one correlation of pilot segment residuals.
    pub lag_one_correlation: f64,
    /// Conservative serial dependence inflation, bounded by n / minimum_segment.
    pub serial_dependence_factor: f64,
    /// Penalty per additional segment: 3 ln(n) times serial dependence inflation.
    pub penalty: f64,
    /// Sum of median absolute deviations plus penalty per change.
    pub objective: f64,
    /// Material sustained increases only; decreases remain in the segmentation.
    pub candidates: Vec<Candidate>,
}
/// Portable `history onset --json` contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Always `saccade-onset.v1`.
    pub schema: String,
    /// Always `onset`.
    pub operation: String,
    /// Independent comparable histories, with their measurement chronology.
    pub partitions: Vec<Partition>,
    /// Unqualified or duplicate input observations.
    pub excluded_observations: Vec<ExcludedObservation>,
    /// Interpretation and qualification limits.
    pub limits: Vec<String>,
}

/// One history partition; insufficient histories cannot manufacture analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Partition {
    /// Fewer than ten distinct qualified observations.
    InsufficientHistory {
        /// Qualified comparison identity.
        identity: String,
        /// Capture timestamps or recorded sequence, never report creation time.
        chronology: String,
        /// Available independent observations in measurement order.
        observations: Vec<Observation>,
        /// Qualified observations outside the requested recent window.
        older_observations_omitted: usize,
    },
    /// Exact segmentation and material candidates, subject to the recorded limits.
    CandidateAnalysis {
        /// Qualified comparison identity.
        identity: String,
        /// Capture timestamps or recorded sequence, never report creation time.
        chronology: String,
        /// Numerical witness and provenance.
        analysis: Box<Analysis<Onset>>,
        /// Qualified observations outside the requested recent window.
        older_observations_omitted: usize,
    },
}

/// A supplied observation omitted from segmentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExcludedObservation {
    /// Content hash of the report object.
    pub run: String,
    /// Explicit exclusion reason.
    pub reason: String,
}

fn median(values: &[f64]) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    if v.len().is_multiple_of(2) {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}
fn costs(values: &[f64]) -> Vec<Vec<f64>> {
    let n = values.len();
    let mut cost = vec![vec![0.0; n + 1]; n];
    for start in 0..n {
        for end in start + 1..=n {
            let centre = median(&values[start..end]);
            cost[start][end] = values[start..end].iter().map(|v| (v - centre).abs()).sum();
        }
    }
    cost
}
fn segment(cost: &[Vec<f64>], minimum: usize, penalty: f64) -> (Vec<usize>, f64) {
    let n = cost.len();
    let mut best = vec![f64::INFINITY; n + 1];
    let mut previous = vec![0; n + 1];
    best[0] = -penalty;
    for end in minimum..=n {
        for start in 0..=end - minimum {
            let candidate = best[start] + cost[start][end] + penalty;
            if candidate < best[end] {
                best[end] = candidate;
                previous[end] = start;
            }
        }
    }
    let mut ends = Vec::new();
    let mut at = n;
    while at > 0 {
        ends.push(at);
        at = previous[at];
    }
    ends.reverse();
    (ends, best[n])
}

/// Detects candidates in 10–120 qualified observations of one identity.
/// Caller qualification is required; malformed values, mixed identities, duplicate
/// windows and unsorted time are rejected, never silently pooled.
pub fn detect(observations: &[Observation]) -> crate::Result<Analysis<Onset>> {
    if !(10..=120).contains(&observations.len()) {
        return Err(crate::Error::Config(
            "onset needs 10..120 distinct qualified observations in one partition".into(),
        ));
    }
    let mut windows = std::collections::BTreeSet::new();
    for (i, o) in observations.iter().enumerate() {
        if o.identity.is_empty()
            || o.identity != observations[0].identity
            || o.window.is_empty()
            || !windows.insert(&o.window)
            || !o.milliseconds.is_finite()
            || o.milliseconds <= 0.0
            || [o.repeat_range_ms, o.floor_ms, o.min_delta_pct]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
            || (i > 0 && observations[i - 1].time > o.time)
        {
            return Err(crate::Error::Config(
                "onset inputs are invalid, incomparable, repeated or unordered".into(),
            ));
        }
    }
    let logs: Vec<_> = observations.iter().map(|o| o.milliseconds.ln()).collect();
    let diffs: Vec<_> = logs.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let repeats: Vec<_> = observations
        .iter()
        .map(|o| (1.0 + o.repeat_range_ms / o.milliseconds).ln())
        .collect();
    // Fixed for the entire series. Conservative repeat range protects noisier
    // producers; the floor avoids division by zero for exact synthetic timings.
    let scale = (median(&diffs) * 1.048_358).max(median(&repeats)).max(1e-6);
    let centre = median(&logs);
    let values: Vec<_> = logs.iter().map(|v| (v - centre) / scale).collect();
    let cost = costs(&values);
    let seed_penalty = 3.0 * (values.len() as f64).ln();
    let (pilot_ends, _) = segment(&cost, 5, seed_penalty);
    // Remove pilot levels so a planted step does not itself inflate serial
    // dependence. Adjacent residual pairs never cross a pilot boundary.
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    let mut start = 0;
    for &end in &pilot_ends {
        let centre = median(&values[start..end]);
        for pair in values[start..end].windows(2) {
            let (a, b) = (pair[0] - centre, pair[1] - centre);
            xx += a * a;
            yy += b * b;
            xy += a * b;
        }
        start = end;
    }
    let lag_one_correlation = if xx * yy > 0.0 {
        (xy / (xx * yy).sqrt()).clamp(0.0, 0.99)
    } else {
        0.0
    };
    // AR(1) long-run variance / effective-sample-size inflation. This is
    // conservative short-history regularization, not a calibrated p-value.
    let serial_dependence_factor = ((1.0 + lag_one_correlation) / (1.0 - lag_one_correlation))
        .clamp(1.0, values.len() as f64 / 5.0);
    let penalty = seed_penalty * serial_dependence_factor;
    let (ends, objective) = segment(&cost, 5, penalty);
    let mut candidates = Vec::new();
    let mut before_start = 0;
    for pair in ends.windows(2) {
        let (split, end) = (pair[0], pair[1]);
        let before = median(
            &observations[before_start..split]
                .iter()
                .map(|o| o.milliseconds)
                .collect::<Vec<_>>(),
        );
        let after = median(
            &observations[split..end]
                .iter()
                .map(|o| o.milliseconds)
                .collect::<Vec<_>>(),
        );
        let materiality = observations[before_start..end]
            .iter()
            .map(|o| o.floor_ms.max(before * o.min_delta_pct / 100.0))
            .fold(0.0, f64::max);
        if after - before > materiality
            && observations[split..end]
                .iter()
                .filter(|o| o.milliseconds - before > materiality)
                .count()
                >= 5
        {
            candidates.push(Candidate {
                status: "candidate".into(),
                last_before: observations[split - 1].clone(),
                first_changed: observations[split].clone(),
                before_count: split - before_start,
                after_count: end - split,
                before_ms: before,
                after_ms: after,
                effect_ms: after - before,
                effect_pct: (after / before - 1.0) * 100.0,
                materiality_ms: materiality,
                retrospective_counts_met: split - before_start >= 10 && end - split >= 5,
            });
        }
        before_start = split;
    }
    let mut provenance = Provenance::native("log-l1-exact-dp/2");
    provenance.settings.extend([("cost".into(), "absolute deviation from segment median of fixed-scale log timings".into()), ("penalty".into(), "3 * ln(n) * serial_dependence_factor".into()), ("minimum_segment".into(), "5".into()), ("normalization".into(), "max(1.048358 * median absolute adjacent log difference, median log1p(repeat range / timing), 1e-6)".into())]);
    provenance.settings.extend([
        ("lag_one_correlation".into(), lag_one_correlation.to_string()),
        ("serial_dependence_factor".into(), serial_dependence_factor.to_string()),
        ("dependence_estimator".into(), "positive lag-one pilot-segment residual correlation; inflation (1+rho)/(1-rho), capped at n/5".into()),
    ]);
    provenance.resources = observations
        .iter()
        .map(|o| Resource {
            name: o.run.clone(),
            role: "history_report".into(),
            sha256: Some(o.run.clone()),
            license: None,
        })
        .collect();
    Ok(Analysis { capability: Capability::Available, provenance: Some(provenance), evidence: Some(Onset { observations: observations.to_vec(), segment_ends: ends, minimum_segment: 5, log_noise_scale: scale, lag_one_correlation, serial_dependence_factor, penalty, objective, candidates }), limitations: vec!["Candidates only. Fresh qualified repeats are required; no commit causality or statistical significance is established.".into(), "The 3 ln(n) penalty is an uncalibrated seed, not a false-alert guarantee. Short gaps and intervening revisions widen the onset interval.".into(), "This detects sustained timing-level increases, not variance changes, periodicity or general gradual drift.".into()] })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn series(values: Vec<f64>) -> Vec<Observation> {
        values
            .into_iter()
            .enumerate()
            .map(|(i, milliseconds)| Observation {
                run: format!("run{i}"),
                commit: Some(format!("commit{i}")),
                time: i as u64 * 86400,
                identity: "hardware-timer-config".into(),
                window: format!("window{i}"),
                milliseconds,
                repeat_range_ms: 0.04,
                floor_ms: 0.12,
                min_delta_pct: 0.5,
            })
            .collect()
    }
    #[test]
    fn planted_steps_are_candidates_and_noise_only_is_not() {
        let noise: Vec<_> = (0..60)
            .map(|i| 10.0 + ((i * 17 % 11) as f64 - 5.0) * 0.004)
            .collect();
        assert!(
            detect(&series(noise.clone()))
                .unwrap()
                .evidence
                .unwrap()
                .candidates
                .is_empty()
        );
        let mut stepped = noise;
        for v in &mut stepped[30..] {
            *v += 2.0;
        }
        let evidence = detect(&series(stepped)).unwrap().evidence.unwrap();
        assert_eq!(evidence.segment_ends, [30, 60]);
        assert_eq!(evidence.candidates[0].first_changed.run, "run30");
        assert_eq!(evidence.candidates[0].status, "candidate");
        assert!(evidence.candidates[0].retrospective_counts_met);
    }
    #[test]
    fn correlated_noise_has_no_material_onset() {
        // Python Random(123): state = .95*state + gauss(0,.08), no change.
        let input = series(vec![
            10.032338274657974,
            10.041762272336939,
            10.007787380920604,
            10.028417850996702,
            10.045147882339776,
            10.029910836866256,
            9.958377111290636,
            9.944246514154521,
            9.975267902598782,
            9.93869660692871,
            9.904197554326242,
            9.970532590746853,
            9.972146525222788,
            9.978163180826238,
            9.947705972134928,
            9.963673940859445,
            9.940588410563782,
            9.926034428777994,
            9.894849056055145,
            9.980084384724325,
            10.010326093123119,
            9.990041183568772,
            10.079144090997252,
            10.185322397997252,
            10.350521156130325,
            10.3564130095208,
            10.282335613695595,
            10.240259800545441,
            10.300030736392555,
            10.168444378037869,
            10.080311885098416,
            10.196059471739101,
            10.215605664712466,
            10.310443782641517,
            10.151377601289164,
            10.119151134440363,
            10.081049630793585,
            10.150941747668362,
            10.154986913697392,
            10.052008405688628,
            10.003986491987101,
            9.89911813277036,
            9.82009357427966,
            9.683496808442474,
            9.580895929909799,
            9.61219305557619,
            9.61723577713948,
            9.650808122844094,
            9.669595036926019,
            9.635506020444495,
            9.729311537111936,
            9.582725477858075,
            9.576913256837226,
            9.557136317235624,
            9.537886040898067,
            9.629099291732205,
            9.726816591526264,
            9.779750833070086,
            9.725870917813063,
            9.707448236280465,
        ]);
        let result = detect(&input).unwrap().evidence.unwrap();
        assert!(result.candidates.is_empty(), "{result:?}");
        assert!(result.serial_dependence_factor > 1.0);
        let mut stepped = input;
        for observation in &mut stepped[30..] {
            observation.milliseconds += 2.0;
        }
        let result = detect(&stepped).unwrap().evidence.unwrap();
        assert!(
            result.candidates.iter().any(
                |candidate| candidate.first_changed.run == "run30" && candidate.effect_ms > 1.0
            ),
            "{result:?}"
        );
    }
    #[test]
    fn exact_objective_matches_exhaustive_partition_oracle() {
        let values: Vec<_> = (0..20)
            .map(|i| if i < 10 { (i % 2) as f64 } else { 7.0 })
            .collect();
        let c = costs(&values);
        fn oracle(cost: &[Vec<f64>], start: usize, penalty: f64) -> f64 {
            let n = cost.len();
            let mut best = cost[start][n];
            for split in start + 5..=n.saturating_sub(5) {
                best = best.min(cost[start][split] + penalty + oracle(cost, split, penalty));
            }
            best
        }
        assert!((segment(&c, 5, 3.0).1 - oracle(&c, 0, 3.0)).abs() < 1e-10);
    }
    #[test]
    fn gaps_survive_but_mixed_and_repeated_observations_are_rejected() {
        let mut input = series(vec![10.0; 20]);
        input[10..].iter_mut().for_each(|o| o.time += 10 * 86400);
        assert_eq!(
            detect(&input).unwrap().evidence.unwrap().observations,
            input
        );
        input[10].identity = "other_gpu".into();
        assert!(detect(&input).is_err());
        input[10].identity = input[0].identity.clone();
        input[10].window = input[0].window.clone();
        assert!(detect(&input).is_err());
    }
    #[test]
    fn materiality_spikes_and_short_suffixes_do_not_become_onsets() {
        let mut values = vec![10.0; 60];
        values[25] = 30.0;
        values[57..].fill(12.0);
        assert!(
            detect(&series(values))
                .unwrap()
                .evidence
                .unwrap()
                .candidates
                .is_empty()
        );
        let mut below = vec![10.0; 60];
        below[30..].fill(10.1);
        assert!(
            detect(&series(below))
                .unwrap()
                .evidence
                .unwrap()
                .candidates
                .is_empty()
        );
    }
}
