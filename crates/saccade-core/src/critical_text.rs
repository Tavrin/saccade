//! Frozen critical-region/string gates, independent of global image averages.
use crate::{Error, Result, general::text, text_quality as tq, ui_review::Source};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Critical text policy discriminator.
pub const POLICY_SCHEMA: &str = "saccade-critical-text-policy.v1";
/// Critical text report discriminator.
pub const SCHEMA: &str = "saccade-critical-text.v1";
/// Failures dominate insufficient evidence; only every passing region yields pass.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// All declared strings and pixel thresholds satisfied.
    Pass,
    /// At least one observed string or pixel threshold failed.
    Fail,
    /// Evidence cannot qualify the policy.
    InsufficientEvidence,
}
/// One critical region at a frozen capture scale.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Unique caller-owned identifier.
    pub id: String,
    /// x,y,width,height in physical capture pixels; captures must share dimensions.
    pub rect_px: [u32; 4],
    /// Exact accepted whole-region strings; explicit typography alternatives only.
    pub accepted_text: Vec<String>,
    /// Required pixel-legibility thresholds, applied to both captures.
    pub legibility: tq::Policy,
}
/// Reusable caller-owned policy; no domain-specific string normalization.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Versioned policy discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-critical-text-policy.v1")))]
    pub schema: String,
    /// Exact capture scale required for the declared rectangles.
    pub dimensions: [u32; 2],
    /// Minimum retained OCR confidence, uncalibrated; does not apply to source facts.
    pub minimum_ocr_confidence: f64,
    /// 1..64 critical regions, all required.
    pub regions: Vec<Region>,
}
/// Literal observation with absent/uncertain evidence retained.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Exact Unicode text from one wholly contained observation; never synthesized whitespace.
    pub text: Option<String>,
    /// True/false only when the evidence supports comparison.
    pub accepted: Option<bool>,
    /// source_fact, uncertain_ocr or unavailable.
    pub assurance: String,
    /// Missing/ambiguous geometry, coverage or confidence reasons.
    pub reasons: Vec<String>,
}
/// Independent string and pixel evidence for one required region.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionResult {
    /// Policy region ID.
    pub id: String,
    /// Region verdict; invalid baseline never qualifies a candidate.
    pub state: State,
    /// Baseline and candidate observations, in that order.
    pub observations: [Observation; 2],
    /// Baseline and candidate pixel measurements, in that order.
    pub pixels: [tq::RegionResult; 2],
    /// Exact Unicode rates when both texts were observed, even for accepted variants.
    pub rates: Option<text::Rates>,
    /// Reasons for failure or abstention.
    pub reasons: Vec<String>,
}
/// Full versioned evidence packet; source/OCR strings are always inert data.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Content-addressed transport identity, absent before emission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// Optional caller-supplied backlinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Versioned report discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-critical-text.v1")))]
    pub schema: String,
    /// Every region is required; failures dominate unknown evidence.
    pub state: State,
    /// Exact encoded baseline and candidate image SHA-256.
    pub image_sha256: [String; 2],
    /// Exact supplied policy bytes SHA-256.
    pub policy_sha256: String,
    /// Exact imported source bytes SHA-256, absent for executed OCR or missing evidence.
    pub source_sha256: [Option<String>; 2],
    /// Frozen effective policy.
    pub policy: Policy,
    /// Image-bound sources, including producer, kind, coverage and confidence.
    pub sources: [Option<Source>; 2],
    /// Every declared region, including unknown evidence.
    pub regions: Vec<RegionResult>,
    /// Explicit interpretation limits.
    pub limitations: Vec<String>,
}
fn observe(source: Option<&Source>, region: &Region, cutoff: f64) -> Observation {
    let mut out = Observation {
        text: None,
        accepted: None,
        assurance: "unavailable".into(),
        reasons: vec![],
    };
    let Some(source) = source else {
        out.reasons.push("text observations not supplied".into());
        return out;
    };
    let ocr = !matches!(source.kind.as_str(), "dom" | "accessibility_tree");
    out.assurance = if ocr { "uncertain_ocr" } else { "source_fact" }.into();
    let r = region.rect_px.map(f64::from);
    let mut nodes = Vec::new();
    for node in &source.nodes {
        let Some(b) = node.bounds else {
            out.reasons.push("unlocated text observation".into());
            return out;
        };
        if b[0] < 0.
            || b[1] < 0.
            || b[2] <= 0.
            || b[3] <= 0.
            || b[0] + b[2] > f64::from(source.dimensions[0])
            || b[1] + b[3] > f64::from(source.dimensions[1])
        {
            out.reasons.push("invalid text observation geometry".into());
            return out;
        }
        if b[0] < r[0] + r[2] && b[1] < r[1] + r[3] && b[0] + b[2] > r[0] && b[1] + b[3] > r[1] {
            if b[0] < r[0] || b[1] < r[1] || b[0] + b[2] > r[0] + r[2] || b[1] + b[3] > r[1] + r[3]
            {
                out.reasons
                    .push("text observation crosses region boundary".into());
                return out;
            }
            nodes.push(node);
        }
    }
    if nodes.is_empty() {
        out.reasons
            .push("no text observed in critical region".into());
        if !ocr && source.complete {
            out.text = Some(String::new());
            out.accepted = Some(false);
        }
        return out;
    }
    if nodes.len() != 1 {
        out.reasons.push(
            "multiple text units; declare one critical region per unit to preserve exact spacing"
                .into(),
        );
        return out;
    }
    out.text = Some(nodes[0].text.clone());
    if ocr
        && nodes
            .iter()
            .any(|n| n.ocr_confidence.is_none_or(|c| c < cutoff))
    {
        out.reasons
            .push("OCR confidence absent or below declared cutoff".into());
    } else if !ocr && !source.complete {
        out.reasons.push("source scope incomplete".into());
    } else {
        out.accepted = out.text.as_ref().map(|t| region.accepted_text.contains(t));
    }
    out
}
/// Compare all required regions using exact strings and the existing text-quality sampler.
/// The caller must bind each source to its encoded image before invoking this API.
pub fn evaluate(
    images: [&RgbaImage; 2],
    hashes: [String; 2],
    sources: [Option<Source>; 2],
    policy: Policy,
    policy_sha256: String,
    source_sha256: [Option<String>; 2],
) -> Result<Report> {
    let size = [images[0].width(), images[0].height()];
    if policy.schema != POLICY_SCHEMA
        || policy.dimensions != size
        || [images[1].width(), images[1].height()] != size
        || policy.regions.is_empty()
        || policy.regions.len() > 64
        || !policy.minimum_ocr_confidence.is_finite()
        || !(0.0..=100.).contains(&policy.minimum_ocr_confidence)
    {
        return Err(Error::Config(
            "critical text policy schema, dimensions, regions or confidence invalid".into(),
        ));
    }
    for (i, source) in sources.iter().enumerate() {
        if let Some(source) = source {
            source.validate(&hashes[i], size)?;
            if source.nodes.len() > 2048
                || source.nodes.iter().map(|n| n.text.len()).sum::<usize>() > 65536
            {
                return Err(Error::Config(
                    "critical text source observation bound exceeded".into(),
                ));
            }
        }
    }
    let mut ids = BTreeSet::new();
    let mut regions = Vec::new();
    let mut total_pixels = 0u64;
    for (i, region) in policy.regions.iter().enumerate() {
        tq::validate_rect(size, region.rect_px)?;
        total_pixels += u64::from(region.rect_px[2]) * u64::from(region.rect_px[3]);
        if total_pixels > crate::general::input::MAX_PIXELS
            || region.id.is_empty()
            || region.id.len() > 256
            || !ids.insert(&region.id)
            || region.accepted_text.is_empty()
            || region.accepted_text.len() > 16
            || region
                .accepted_text
                .iter()
                .any(|t| t.is_empty() || t.len() > 4096)
            || region.accepted_text.iter().collect::<BTreeSet<_>>().len()
                != region.accepted_text.len()
        {
            return Err(Error::Config(
                "critical text region IDs, accepted strings or pixel work bound invalid".into(),
            ));
        }
        let observations = [
            observe(sources[0].as_ref(), region, policy.minimum_ocr_confidence),
            observe(sources[1].as_ref(), region, policy.minimum_ocr_confidence),
        ];
        let pixels = [
            tq::legibility(images[0], region.rect_px, i, region.legibility)?,
            tq::legibility(images[1], region.rect_px, i, region.legibility)?,
        ];
        let rates = match (&observations[0].text, &observations[1].text) {
            (Some(a), Some(b)) => Some(text::rates(a, b)?),
            _ => None,
        };
        let mut reasons = Vec::new();
        let baseline_valid =
            observations[0].accepted == Some(true) && pixels[0].state == tq::State::Legible;
        if !baseline_valid {
            reasons.push("baseline did not establish accepted, legible text".into());
        }
        let candidate_failed =
            observations[1].accepted == Some(false) || pixels[1].state == tq::State::Illegible;
        if observations[1].accepted == Some(false) {
            reasons.push("critical_string_mismatch".into());
        }
        if pixels[1].state == tq::State::Illegible {
            reasons.push("critical_pixel_threshold_failed".into());
        }
        let state = if candidate_failed {
            State::Fail
        } else if baseline_valid
            && observations[1].accepted == Some(true)
            && pixels[1].state == tq::State::Legible
        {
            State::Pass
        } else {
            reasons.push("critical evidence insufficient".into());
            State::InsufficientEvidence
        };
        regions.push(RegionResult {
            id: region.id.clone(),
            state,
            observations,
            pixels,
            rates,
            reasons,
        });
    }
    let state = if regions.iter().any(|r| r.state == State::Fail) {
        State::Fail
    } else if regions.iter().all(|r| r.state == State::Pass) {
        State::Pass
    } else {
        State::InsufficientEvidence
    };
    Ok(Report { report_id:None, source_refs:vec![], schema:SCHEMA.into(), state,
        image_sha256:hashes, policy_sha256, source_sha256, policy, sources, regions,
        limitations:vec!["Pass applies only to declared regions, exact accepted strings and pixel thresholds at the frozen scale.".into(),
            "Source facts describe producer text; hashes bind the source to bytes but do not prove accurate source export or visible glyph identity.".into(),
            "OCR correspondence and confidence are heuristic; OCR agreement is not human readability or glyph completeness.".into(),
            "Pixel measures use opaque SDR dominant backgrounds and component body-height proxies, not font metrics or legal/scientific correctness.".into(),
            "No implicit typography folding, layout mapping, provider calls or model downloads.".into()] })
}
