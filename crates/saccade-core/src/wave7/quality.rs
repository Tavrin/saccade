//! Separately named learned quality measurements; no FLIP/SSIMULACRA2 fusion.
use super::{
    models::{Result, VisionError, valid_hash},
    vision::{Provenance, VisionImage},
};
use serde::{Deserialize, Serialize};
/// Named metric report contract.
pub const QUALITY_SCHEMA: &str = "saccade-learned-quality.v1";
/// Learned metric identity, never an implicit regression threshold.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LearnedMetric {
    /// LPIPS v0.1, AlexNet features/calibration.
    LpipsAlexV01,
    /// DISTS full-reference fallback.
    Dists,
    /// MUSIQ technical checkpoint, no-reference quality.
    MusiqTechnical,
}
impl LearnedMetric {
    /// Registry family id.
    pub fn model_id(self) -> &'static str {
        match self {
            Self::LpipsAlexV01 => "lpips-alex-v0.1",
            Self::Dists => "dists",
            Self::MusiqTechnical => "musiq-technical",
        }
    }
    /// Whether a full-resolution reference is required.
    pub fn needs_reference(self) -> bool {
        self != Self::MusiqTechnical
    }
}
/// A measurement kept independent of all existing metrics and verdicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityMeasurement {
    /// Name of metric/checkpoint family.
    pub metric: LearnedMetric,
    /// Finite scalar; scale is model/checkpoint specific.
    pub value: f32,
    /// lower_is_better for distances; higher_is_better for MUSIQ.
    pub direction: String,
    /// Exact model/version/resolution handling/export identity.
    pub provenance: Provenance,
}
/// Quality backend, with fake/tiny stand-ins admitted only via explicit adapters.
pub trait QualityBackend {
    /// Compute one declared measurement.
    fn score(
        &mut self,
        metric: LearnedMetric,
        image: &VisionImage,
        reference: Option<&VisionImage>,
    ) -> Result<QualityMeasurement>;
}
/// Standalone learned comparison report or single-image technical-quality report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityReport {
    /// QUALITY_SCHEMA.
    pub schema: String,
    /// Exact encoded image digest.
    pub image_sha256: String,
    /// Exact reference digest when full-reference.
    pub reference_sha256: Option<String>,
    /// Separately named metrics, never a fused score.
    pub named_metrics: Vec<QualityMeasurement>,
    /// Always false: measurements here never decide existing compare verdicts.
    pub affects_compare_verdict: bool,
}
impl QualityReport {
    /// Reject wrong reference binding/metric/identity or unsupported numeric values.
    pub fn validate(
        &self,
        metric: LearnedMetric,
        image: &VisionImage,
        reference: Option<&VisionImage>,
    ) -> Result<()> {
        if self.schema != QUALITY_SCHEMA
            || self.image_sha256 != image.sha256
            || self.reference_sha256 != reference.map(|i| i.sha256.clone())
            || self.affects_compare_verdict
            || self.named_metrics.len() != 1
            || metric.needs_reference() != reference.is_some()
            || !valid_hash(&self.image_sha256)
        {
            return Err(VisionError::Invalid("quality binding/metric count".into()));
        }
        let m = &self.named_metrics[0];
        m.provenance.validate()?;
        if m.metric != metric
            || m.provenance.model_id != metric.model_id()
            || !m.value.is_finite()
            || m.value < 0.
            || m.direction
                != if metric.needs_reference() {
                    "lower_is_better"
                } else {
                    "higher_is_better"
                }
        {
            return Err(VisionError::Invalid("learned metric identity/value".into()));
        }
        Ok(())
    }
}
/// Run one measurement; paired images must have matching original dimensions.
pub fn measure(
    metric: LearnedMetric,
    image: &VisionImage,
    reference: Option<&VisionImage>,
    backend: &mut dyn QualityBackend,
) -> Result<QualityReport> {
    if metric.needs_reference() != reference.is_some()
        || reference.is_some_and(|r| r.size() != image.size())
    {
        return Err(VisionError::Invalid(
            "metric reference/dimension mismatch".into(),
        ));
    }
    let m = backend.score(metric, image, reference)?;
    let r = QualityReport {
        schema: QUALITY_SCHEMA.into(),
        image_sha256: image.sha256.clone(),
        reference_sha256: reference.map(|i| i.sha256.clone()),
        named_metrics: vec![m],
        affects_compare_verdict: false,
    };
    r.validate(metric, image, reference)?;
    Ok(r)
}
#[cfg(feature = "local-models")]
impl QualityBackend for super::runtime::OnnxModel {
    fn score(
        &mut self,
        metric: LearnedMetric,
        image: &VisionImage,
        reference: Option<&VisionImage>,
    ) -> Result<QualityMeasurement> {
        let adapter = if metric.needs_reference() {
            "scalar-pair-v1"
        } else {
            "scalar-image-v1"
        };
        if self.model.id != metric.model_id()
            || self.model.input.adapter != adapter
            || self.model.task
                != if metric.needs_reference() {
                    "full_reference_quality"
                } else {
                    "no_reference_quality"
                }
        {
            return Err(VisionError::Invalid(
                "quality model/export contract mismatch".into(),
            ));
        }
        let output = self.run(image, reference)?;
        let data = output
            .get(&self.model.input.output)
            .ok_or_else(|| VisionError::Invalid("missing declared quality output".into()))?;
        if data.values.len() != 1 {
            return Err(VisionError::Invalid(
                "quality graph must return one scalar".into(),
            ));
        }
        Ok(QualityMeasurement {
            metric,
            value: data.values[0],
            direction: if metric.needs_reference() {
                "lower_is_better".into()
            } else {
                "higher_is_better".into()
            },
            provenance: self.provenance(image),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::super::models::digest;
    use super::*;
    struct Tiny;
    impl QualityBackend for Tiny {
        fn score(
            &mut self,
            m: LearnedMetric,
            _: &VisionImage,
            _: Option<&VisionImage>,
        ) -> Result<QualityMeasurement> {
            Ok(QualityMeasurement {
                metric: m,
                value: 0.25,
                direction: if m.needs_reference() {
                    "lower_is_better"
                } else {
                    "higher_is_better"
                }
                .into(),
                provenance: Provenance::fixture(m.model_id()),
            })
        }
    }
    #[test]
    fn named_pair_metrics_never_change_a_verdict() {
        let i = VisionImage {
            pixels: image::RgbImage::new(8, 8),
            sha256: digest(b"fixture"),
        };
        for m in [LearnedMetric::LpipsAlexV01, LearnedMetric::Dists] {
            let r = measure(m, &i, Some(&i), &mut Tiny).unwrap();
            assert!(!r.affects_compare_verdict);
            assert_eq!(r.named_metrics[0].metric, m);
        }
    }
    #[test]
    fn no_reference_and_dimension_contracts_are_enforced() {
        let i = VisionImage {
            pixels: image::RgbImage::new(8, 8),
            sha256: digest(b"fixture"),
        };
        assert!(measure(LearnedMetric::LpipsAlexV01, &i, None, &mut Tiny).is_err());
        assert!(measure(LearnedMetric::MusiqTechnical, &i, Some(&i), &mut Tiny).is_err());
        assert!(measure(LearnedMetric::MusiqTechnical, &i, None, &mut Tiny).is_ok());
    }
}
