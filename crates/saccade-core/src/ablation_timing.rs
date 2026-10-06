//! Independent-repeat timing spread and baseline shift for ablation tables.
use crate::{
    Result,
    paired_stats::{Generator, quantile},
};
use serde::{Deserialize, Serialize};
/// Per-arm repeat evidence. Unpaired intervals never imply matched acquisition.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// All accepted repeat timings in milliseconds.
    pub samples_ms: Vec<f64>,
    /// Median timing.
    pub median_ms: f64,
    /// Interquartile range.
    pub iqr_ms: f64,
    /// Independent-sample Hodges–Lehmann shift against baseline.
    pub hl_delta_ms: f64,
    /// Seeded independent-arm bootstrap CI, unavailable without two repeats per arm.
    pub interval_ms: Option<[f64; 2]>,
    /// Explicit method and independence limitation.
    pub method: String,
}
fn shift(a: &[f64], b: &[f64]) -> Result<f64> {
    let mut diffs = a
        .iter()
        .flat_map(|x| b.iter().map(move |y| x - y))
        .collect::<Vec<_>>();
    diffs.sort_by(f64::total_cmp);
    quantile(&diffs, 0.5)
}
/// Summarize own repeats and bootstrap each arm separately against the baseline.
pub fn summarize(base: &[f64], arm: &[f64]) -> Result<Summary> {
    if base.is_empty()
        || arm.is_empty()
        || base.len() > 128
        || arm.len() > 128
        || base.iter().chain(arm).any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(crate::Error::Config(
            "ablation timings need 1..128 positive finite repeats per arm".into(),
        ));
    }
    let mut sorted = arm.to_vec();
    sorted.sort_by(f64::total_cmp);
    let interval_ms = if base.len() > 1 && arm.len() > 1 {
        let mut rng = Generator::new(11);
        let mut draws = Vec::new();
        for _ in 0..2048 {
            let a = (0..arm.len())
                .map(|_| rng.index(arm.len()).map(|i| arm[i]))
                .collect::<Result<Vec<_>>>()?;
            let b = (0..base.len())
                .map(|_| rng.index(base.len()).map(|i| base[i]))
                .collect::<Result<Vec<_>>>()?;
            draws.push(shift(&a, &b)?);
        }
        draws.sort_by(f64::total_cmp);
        Some([quantile(&draws, 0.025)?, quantile(&draws, 0.975)?])
    } else {
        None
    };
    Ok(Summary{samples_ms:arm.to_vec(),median_ms:quantile(&sorted,0.5)?,iqr_ms:quantile(&sorted,0.75)?-quantile(&sorted,0.25)?,hl_delta_ms:shift(arm,base)?,interval_ms,method:"unpaired_two_sample_HL_cross_differences; independent_repeat_percentile_bootstrap_95pct_seed11_2048; acquisition independence is external; one-repeat CI unavailable".into()})
}
/// Collect all complete repeats; missing timing means no summary, never a partial spread.
pub fn collect(paths: &[std::path::PathBuf], name: &str) -> Result<Option<Vec<f64>>> {
    let mut times = Vec::new();
    for p in paths {
        let Some(perf) = crate::perf::CapturePerf::read(p, name)? else {
            return Ok(None);
        };
        times.push(perf.frame.value);
    }
    Ok(Some(times))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn four_arm_lab_processing_effects_rank_and_have_own_spread() {
        let base = [10., 10.2, 9.8];
        let mut rows = [
            vec![9., 9.2, 8.8],
            vec![6., 6.1, 5.9],
            vec![11., 11.4, 10.6],
            vec![10., 10.2, 9.8],
        ]
        .iter()
        .map(|a| summarize(&base, a).unwrap())
        .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.hl_delta_ms.total_cmp(&b.hl_delta_ms));
        assert_eq!(
            rows.iter().map(|r| r.median_ms).collect::<Vec<_>>(),
            vec![6., 9., 10., 11.]
        );
        assert!(rows[0].interval_ms.unwrap()[1] < -3.);
        assert!(rows[3].interval_ms.unwrap()[0] > 0.);
        assert!(rows[3].iqr_ms > rows[0].iqr_ms);
    }
}
