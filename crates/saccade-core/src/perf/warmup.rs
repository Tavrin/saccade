//! Local warmup convergence, separate from clocks and repeat noise.
use serde::{Deserialize, Serialize};
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub version: String,
    pub window: usize,
    pub mean_relative_tolerance: f64,
    pub coefficient_of_variation_max: f64,
    pub slope_relative_max: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            version: "warmup-1".into(),
            window: 16,
            mean_relative_tolerance: 0.02,
            coefficient_of_variation_max: 0.03,
            slope_relative_max: 0.001,
        }
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub policy: Policy,
    pub iterations_ms: Vec<f64>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Convergence {
    pub policy: Policy,
    pub samples: usize,
    pub evaluated_range: [usize; 2],
    pub window_means_ms: [f64; 2],
    pub relative_mean_change: f64,
    pub coefficient_of_variation: f64,
    pub relative_slope_per_iteration: f64,
    pub converged: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    WarmupNotConverged,
    WarmupEvidenceUnavailable,
    WarmupDataInvalid,
    ClockUnqualified,
    NoiseUnqualified,
    QualificationUnavailable,
    ProducerRejected,
    ConditionMismatch,
}
pub fn evaluate(e: &Evidence) -> Result<Convergence, String> {
    let p = &e.policy;
    if p.version != "warmup-1"
        || !(4..=256).contains(&p.window)
        || e.iterations_ms.len() > 4096
        || e.iterations_ms.len() < 2 * p.window
        || e.iterations_ms.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || [
            p.mean_relative_tolerance,
            p.coefficient_of_variation_max,
            p.slope_relative_max,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1.0)
    {
        return Err("invalid warmup policy or iteration data (need two windows, <=4096 finite positive samples)".into());
    }
    let n = e.iterations_ms.len();
    let start = n - 2 * p.window;
    let tail = &e.iterations_ms[start..];
    let w = p.window;
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let means = [mean(&tail[..w]), mean(&tail[w..])];
    let relative = (means[1] - means[0]).abs() / means[0].max(means[1]);
    let m = mean(tail);
    let cv = (tail.iter().map(|v| (v - m).powi(2)).sum::<f64>() / tail.len() as f64).sqrt() / m;
    let center = (tail.len() - 1) as f64 / 2.0;
    let slope = tail
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64 - center) * (v - m))
        .sum::<f64>()
        / tail
            .iter()
            .enumerate()
            .map(|(i, _)| (i as f64 - center).powi(2))
            .sum::<f64>()
        / m;
    if [relative, cv, slope].iter().any(|v| !v.is_finite()) {
        return Err("warmup arithmetic overflow".into());
    }
    Ok(Convergence {
        policy: p.clone(),
        samples: n,
        evaluated_range: [start, n],
        window_means_ms: means,
        relative_mean_change: relative,
        coefficient_of_variation: cv,
        relative_slope_per_iteration: slope,
        converged: relative <= p.mean_relative_tolerance
            && cv <= p.coefficient_of_variation_max
            && slope.abs() <= p.slope_relative_max,
    })
}
#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    #[test]
    fn steady_tail_passes_drift_and_oscillation_fail() {
        let evidence = |v| Evidence {
            policy: Policy::default(),
            iterations_ms: v,
        };
        assert!(evaluate(&evidence(vec![10.0; 64])).is_ok_and(|v| v.converged));
        assert!(
            evaluate(&evidence((0..64).map(|i| 10.0 + i as f64 * 0.1).collect()))
                .is_ok_and(|v| !v.converged)
        );
        assert!(
            evaluate(&evidence(
                (0..64)
                    .map(|i| if i % 2 == 0 { 8.0 } else { 12.0 })
                    .collect()
            ))
            .is_ok_and(|v| !v.converged)
        );
        assert!(evaluate(&evidence(vec![10.0; 8])).is_err());
    }
    #[test]
    fn local_drift_rejects_producer_warmup_and_has_distinct_reason() {
        let mut capture:super::super::CapturePerf=serde_json::from_value(serde_json::json!({"schema":"saccade-perf.v2","kind":"measurement","unit":"ms","frame":{"value":10.0,"stat":"mean","samples":64},"terms":[],"counters":{},"context":{"measurement":null,"timer":null,"quantum_ms":null,"sample_window":null,"raw_samples":[],"hardware":{},"aggregation":"independent_statistics","attribution_coverage":null,"unresolved_remainder_ms":null,"capture_hash":null,"configuration_hash":null,"producer":"fixture","producer_version":"1","qualification":{"status":"qualified","rule":"fixture","version":"1","checks":{"warmup_complete":true,"clock_qualified":false},"reasons":[],"warmup":{"policy":Policy::default(),"iterations_ms":(0..64).map(|i|10.0+i as f64*0.1).collect::<Vec<_>>()}}}})).unwrap_or_else(|e| panic!("fixture: {e}"));
        assert_eq!(
            capture.qualification().0,
            super::super::Comparability::Rejected
        );
        let codes = capture.qualification_codes();
        assert!(codes.contains(&Reason::WarmupNotConverged));
        assert!(codes.contains(&Reason::ClockUnqualified));
        if let Some(c) = &mut capture.context {
            c.qualification.warmup = None;
        }
        assert!(
            !capture
                .qualification_codes()
                .contains(&Reason::WarmupNotConverged)
        );
    }
}
