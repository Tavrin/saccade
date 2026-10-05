//! Fixed-plan run-paired Hodges–Lehmann effects and seeded percentile bootstrap.
use crate::{
    Error, Result,
    evidence::canonical::{self, Digest},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// Frozen assignment for one independently acquired matched run pair.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    /// Stable run pair ID, declared before acquisition.
    pub id: String,
    /// ab or ba, the actual acquisition order to control/report order bias.
    pub order: String,
}
/// A fixed acquisition and analysis plan, not a sequential stopping procedure.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// saccade-perf-plan.v1.
    pub schema: String,
    /// run; correlated frames are not independent resampling units.
    pub independent_unit: String,
    /// fixed_count; opportunistic stopping is not supported.
    pub stopping_rule: String,
    /// All expected run pairs, in acquisition order (6..128).
    pub pairs: Vec<Assignment>,
    /// Confidence level chosen in advance (0.8..0.99).
    pub confidence: f64,
    /// Fixed number of bootstrap draws (1024..8192).
    pub resamples: u32,
    /// SplitMix64 seed chosen in advance.
    pub seed: u64,
}
/// One matched run observation, not an arbitrary pair of correlated frames.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Exactly the planned pair ID.
    pub pair_id: String,
    /// Actual acquisition order, matching its predeclared assignment.
    pub order: String,
    /// Reference run timing in milliseconds, strictly positive and finite.
    pub before_ms: f64,
    /// Candidate run timing in milliseconds, strictly positive and finite.
    pub after_ms: f64,
}
/// Sidecar alongside candidate saccade-perf.json, bound to both aggregate inputs.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Samples {
    /// saccade-perf-pairs.v1.
    pub schema: String,
    /// Exact reference aggregate performance file bytes.
    pub before_perf_sha256: Digest,
    /// Exact candidate aggregate performance file bytes.
    pub after_perf_sha256: Digest,
    /// Frozen plan body, retained for review.
    pub plan: Plan,
    /// Canonical plan hash, pinned before acquisition by the producer.
    pub plan_sha256: Digest,
    /// Complete pair observations, in the declared plan order.
    pub observations: Vec<Observation>,
}
/// Reproducible robust frame effect, separate from aggregate and noise evidence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// paired_hodges_lehmann_walsh; pseudomedian shift, not population median.
    pub estimator: String,
    /// percentile bootstrap with whole run-pair resampling and linear quantiles.
    pub interval_method: String,
    /// Candidate minus reference paired timing effect, ms.
    pub delta_ms: f64,
    /// Paired percentile interval, ms.
    pub interval_ms: [f64; 2],
    /// 100×(exp(HL(log(after)-log(before)))-1).
    pub delta_pct: f64,
    /// Transformed interval on the same paired log-ratios.
    pub interval_pct: [f64; 2],
    /// Prespecified confidence level.
    pub confidence: f64,
    /// Number of complete independently acquired run pairs.
    pub pairs: usize,
    /// Fixed resample count.
    pub resamples: u32,
    /// Reproducible generator seed.
    pub seed: u64,
    /// Canonical plan identity.
    pub plan_sha256: Digest,
    /// Complete series identity, including raw observations and aggregate pins.
    pub samples_sha256: Digest,
    /// Complete fixed plan and raw observations, retained for independent replay.
    pub samples: Samples,
    /// Equal-valued bootstrap effect distribution; cannot support a new verdict.
    pub degenerate: bool,
    /// Numerical and sampling limits.
    pub limits: Vec<String>,
}
fn invalid(s: &str) -> Error {
    Error::Config(s.into())
}
/// Median of all Walsh averages (i ≤ j), including diagonal pairs.
/// For a paired analysis, pass differences or log-ratios, not two unpaired arrays.
pub fn hodges_lehmann(values: &[f64]) -> Result<f64> {
    if values.is_empty() || values.len() > 128 || values.iter().any(|v| !v.is_finite()) {
        return Err(invalid("HL needs 1..128 finite paired values"));
    }
    let mut walsh = Vec::with_capacity(values.len() * (values.len() + 1) / 2);
    for (i, &a) in values.iter().enumerate() {
        for &b in &values[i..] {
            walsh.push(a * 0.5 + b * 0.5);
        }
    }
    let mid = walsh.len() / 2;
    let even = walsh.len() % 2 == 0;
    let (lower, median, _) = walsh.select_nth_unstable_by(mid, f64::total_cmp);
    Ok(if even {
        lower.iter().copied().fold(f64::NEG_INFINITY, f64::max) * 0.5 + *median * 0.5
    } else {
        *median
    })
}
/// Linear interpolated empirical quantile, matching NumPy/SciPy's default convention.
pub fn quantile(sorted: &[f64], q: f64) -> Result<f64> {
    if sorted.is_empty()
        || !q.is_finite()
        || !(0.0..=1.0).contains(&q)
        || sorted.iter().any(|v| !v.is_finite())
        || sorted.windows(2).any(|v| v[0] > v[1])
    {
        return Err(invalid(
            "quantile requires sorted finite values and q in [0,1]",
        ));
    }
    let p = q * (sorted.len() - 1) as f64;
    let low = p.floor() as usize;
    let high = p.ceil() as usize;
    let weight = p - low as f64;
    let a = sorted[low];
    let b = sorted[high];
    // Opposite signs can overflow b-a; same-sign interpolation retains precision.
    Ok(if a.signum() != b.signum() {
        a * (1.0 - weight) + b * weight
    } else {
        a + weight * (b - a)
    })
}
/// Deterministic SplitMix64 stream; no global random state.
#[derive(Debug, Clone)]
pub struct Generator(u64);
impl Generator {
    /// Starts the exact prespecified stream.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    /// Uniform bounded index using rejection sampling; returns an error for zero length.
    pub fn index(&mut self, length: usize) -> Result<usize> {
        if length == 0 {
            return Err(invalid("nonempty sample required"));
        }
        let n = length as u64;
        let threshold = n.wrapping_neg() % n;
        loop {
            self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^= z >> 31;
            if z >= threshold {
                return Ok((z % n) as usize);
            }
        }
    }
}
impl Samples {
    /// Enforce full acquisition count, exact plan/IDs/order and valid independent units.
    pub fn validate(&self) -> Result<()> {
        let p = &self.plan;
        if self.schema != "saccade-perf-pairs.v1"
            || p.schema != "saccade-perf-plan.v1"
            || p.independent_unit != "run"
            || p.stopping_rule != "fixed_count"
            || !(6..=128).contains(&p.pairs.len())
            || self.observations.len() != p.pairs.len()
            || !p.confidence.is_finite()
            || !(0.8..=0.99).contains(&p.confidence)
            || !(1024..=8192).contains(&p.resamples)
            || canonical::digest(p).map_err(|e| invalid(&e.to_string()))? != self.plan_sha256
        {
            return Err(invalid(
                "paired performance requires a hash-pinned fixed plan, 6..128 complete independent run pairs and prespecified confidence/resamples",
            ));
        }
        let mut seen = BTreeSet::new();
        for (expected, observed) in p.pairs.iter().zip(&self.observations) {
            if expected.id.is_empty()
                || !seen.insert(&expected.id)
                || !["ab", "ba"].contains(&expected.order.as_str())
                || observed.pair_id != expected.id
                || observed.order != expected.order
                || [observed.before_ms, observed.after_ms]
                    .iter()
                    .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1e12)
            {
                return Err(invalid(
                    "run pair IDs/order/timings do not match the frozen acquisition plan",
                ));
            }
        }
        if !p.pairs.iter().any(|v| v.order == "ab") || !p.pairs.iter().any(|v| v.order == "ba") {
            return Err(invalid(
                "fixed paired plan must include both ab and ba acquisition orders",
            ));
        }
        Ok(())
    }
}
/// Estimate paired effects; no repeated peeking, auto-acquisition or early stopping.
pub fn estimate(samples: &Samples) -> Result<Effect> {
    samples.validate()?;
    let d: Vec<_> = samples
        .observations
        .iter()
        .map(|v| v.after_ms - v.before_ms)
        .collect();
    let ratios: Vec<_> = samples
        .observations
        .iter()
        .map(|v| v.after_ms.ln() - v.before_ms.ln())
        .collect();
    let mut generator = Generator::new(samples.plan.seed);
    let mut deltas = Vec::with_capacity(samples.plan.resamples as usize);
    let mut logs = Vec::with_capacity(samples.plan.resamples as usize);
    let mut resampled = vec![0.; d.len()];
    let mut resampled_logs = vec![0.; d.len()];
    for _ in 0..samples.plan.resamples {
        for i in 0..d.len() {
            let j = generator.index(d.len())?;
            resampled[i] = d[j];
            resampled_logs[i] = ratios[j];
        }
        deltas.push(hodges_lehmann(&resampled)?);
        logs.push(hodges_lehmann(&resampled_logs)?);
    }
    deltas.sort_unstable_by(f64::total_cmp);
    logs.sort_unstable_by(f64::total_cmp);
    let tail = (1.0 - samples.plan.confidence) * 0.5;
    let interval_ms = [quantile(&deltas, tail)?, quantile(&deltas, 1.0 - tail)?];
    let percent = |v: f64| v.exp_m1() * 100.;
    let interval_pct = [
        percent(quantile(&logs, tail)?),
        percent(quantile(&logs, 1.0 - tail)?),
    ];
    let delta_pct = percent(hodges_lehmann(&ratios)?);
    if !delta_pct.is_finite() || interval_pct.iter().any(|v| !v.is_finite()) {
        return Err(invalid("paired log-ratio effect overflows"));
    }
    Ok(Effect {
        estimator: "paired_hodges_lehmann_walsh".into(),
        interval_method: "paired_run_percentile_linear_quantile".into(),
        delta_ms: hodges_lehmann(&d)?,
        interval_ms,
        delta_pct,
        interval_pct,
        confidence: samples.plan.confidence,
        pairs: d.len(),
        resamples: samples.plan.resamples,
        seed: samples.plan.seed,
        plan_sha256: samples.plan_sha256.clone(),
        samples_sha256: canonical::digest(samples).map_err(|e| invalid(&e.to_string()))?,
        samples: samples.clone(),
        degenerate: deltas.first() == deltas.last(),
        limits: vec![
            "HL is a pseudomedian shift; skew, run-order/thermal drift and workload mismatch are not removed.".into(),
            "Percentile intervals resample whole declared independent run pairs. Dependence/block resampling and sequential confidence are unqualified.".into(),
            "Plan freezing, independence and timing acquisition are producer assertions; interval coverage is not universally guaranteed.".into(),
            "Term/counter deltas remain aggregate diagnostics; robust inference here is for frame timing only.".into(),
        ],
    })
}
/// Read an optional bounded candidate-side series, verifying both aggregate byte identities.
#[cfg(feature = "graphics")]
pub(crate) fn read(
    before: &std::path::Path,
    after: &std::path::Path,
    name: &str,
) -> Result<Option<Effect>> {
    let path = after.join("saccade-perf-pairs.json");
    // Entry absence alone enables historical fallback; a dangling link is supplied evidence.
    let entry = match std::fs::symlink_metadata(&path) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                context: "reading paired performance sidecar".into(),
                source,
            });
        }
    };
    let metadata = if entry.file_type().is_symlink() {
        std::fs::metadata(&path).map_err(|source| Error::Io {
            context: "resolving supplied paired performance sidecar".into(),
            source,
        })?
    } else {
        entry
    };
    if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err(invalid("paired sidecar must be a regular file <=4 MiB"));
    }
    let root = std::fs::canonicalize(after).map_err(|source| Error::Io {
        context: "resolving paired performance capture root".into(),
        source,
    })?;
    let resolved = std::fs::canonicalize(&path).map_err(|source| Error::Io {
        context: "resolving paired performance sidecar".into(),
        source,
    })?;
    if !resolved.starts_with(root) {
        return Err(invalid(
            "paired performance sidecar escapes the capture root",
        ));
    }
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        context: "reading paired performance sidecar".into(),
        source,
    })?;
    let samples: Samples = canonical::decode(&bytes).map_err(|e| invalid(&e.to_string()))?;
    for (dir, hash) in [
        (before, &samples.before_perf_sha256),
        (after, &samples.after_perf_sha256),
    ] {
        let bytes = std::fs::read(dir.join(name)).map_err(|source| Error::Io {
            context: "hashing paired aggregate performance input".into(),
            source,
        })?;
        if Digest::of_bytes(&bytes) != *hash {
            return Err(invalid(
                "paired samples refer to stale aggregate performance inputs",
            ));
        }
    }
    Ok(Some(estimate(&samples)?))
}
