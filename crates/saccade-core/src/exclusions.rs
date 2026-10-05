//! Deterministic accounting of what a configured comparison did not check.
use crate::evidence::analysis::{Analysis, Capability, Provenance};
use crate::report::{Entry, Metric, Report, Status};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Full-map evidence retained before masks are applied to the verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PixelExclusions {
    /// Image dimensions.
    pub dimensions: [u32; 2],
    /// Excluded pixels as row-major [start, length] runs.
    pub runs: Vec<[u64; 2]>,
    /// SHA-256 of the resolved row-major mask (one 0/1 byte per pixel).
    pub mask_sha256: String,
    /// Number of excluded pixels.
    pub pixels: u64,
    /// Summed FLIP error on excluded pixels.
    pub error_sum: f64,
    /// Mean FLIP on excluded pixels; absent for an empty mask.
    pub error_mean: Option<f64>,
    /// Maximum FLIP on excluded pixels; absent for an empty mask.
    pub error_max: Option<f64>,
    /// Entry metric on the complete unmasked error map.
    pub full_frame_value: f64,
    /// Diagnostic verdict with all masks removed, same regions and thresholds.
    pub without_masks: Status,
}

/// A configured bound and its measured remaining margin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Threshold {
    /// Entry, region or hotspot scope.
    pub scope: String,
    /// Metric name.
    pub metric: String,
    /// Configured upper limit.
    pub limit: f64,
    /// Measured value; absent when not measured.
    pub value: Option<f64>,
    /// Limit minus measured value; a negative value exceeds the bound.
    pub headroom: Option<f64>,
}

/// Exclusions and unknowns for one entry, including passing entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EntryAudit {
    /// Image name.
    pub name: String,
    /// Original configured verdict; the audit cannot change it.
    pub configured_status: Status,
    /// All deciding thresholds and their remaining margin.
    pub thresholds: Vec<Threshold>,
    /// Region names without a deciding threshold.
    pub informational_regions: Vec<String>,
    /// Metadata changes explicitly ignored by policy.
    pub ignored_metadata: Vec<crate::report::MetaDiff>,
    /// Capture validity, including missing keys and source provenance.
    pub validity: crate::meta::CaptureValidity,
    /// Channel, HDR and other unmeasured scope statements.
    pub limitations: Vec<String>,
}

/// Run scope beyond the per-pixel masks stored on each entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ExclusionAudit {
    /// Observed names removed by ignore globs or entry selection.
    pub excluded_captures: Vec<String>,
    /// Selected names without a complete measurable pair.
    pub incomplete_captures: Vec<String>,
    /// Performance qualification; absent performance stays unknown.
    pub performance: Capability,
    /// Declared entry selection globs.
    pub selection: Vec<String>,
    /// Declared ignore globs.
    pub ignore: Vec<String>,
    /// Metadata ignore globs, including defaults.
    pub metadata_ignore: Vec<String>,
    /// Per-entry bounds and gaps in report order.
    pub entries: Vec<EntryAudit>,
}

/// Measures masked error without modifying the error map or configured verdict.
pub(crate) fn pixels(
    cmp: &crate::compare::Comparison,
    mask: Option<&[bool]>,
    entry: &Entry,
    config: &crate::config::RunConfig,
) -> crate::Result<PixelExclusions> {
    let bits: Vec<u8> = (0..cmp.error_map.len())
        .map(|i| u8::from(mask.is_some_and(|m| m[i])))
        .collect();
    let mut runs: Vec<[u64; 2]> = Vec::new();
    let mut count = 0u64;
    let mut sum = 0.0f64;
    let mut max = 0.0f64;
    for (i, (&bit, &error)) in bits.iter().zip(&cmp.error_map).enumerate() {
        if bit == 0 {
            continue;
        }
        count += 1;
        sum += f64::from(error);
        max = max.max(f64::from(error));
        match runs.last_mut() {
            Some([start, len]) if *start + *len == i as u64 => *len += 1,
            _ => runs.push([i as u64, 1]),
        }
    }
    let mut unmasked_config = config.clone();
    unmasked_config.masks.clear();
    let mut unmasked_entry = entry.clone();
    crate::regions::evaluate(&mut unmasked_entry, cmp, &unmasked_config)?;
    let value = crate::run::metric_value(&cmp.metrics, entry.metric_used);
    let without_masks = if config.mode == crate::report::Mode::Identity {
        if entry.bit_identical == Some(true) {
            Status::Pass
        } else {
            Status::Fail
        }
    } else {
        let status = crate::regions::combine(
            crate::run::status_of(value, entry.threshold),
            &unmasked_entry.regions,
        );
        if status == Status::Pass && config.hotspot_fail.is_some_and(|t| cmp.metrics.max >= t) {
            Status::Fail
        } else {
            status
        }
    };
    Ok(PixelExclusions {
        dimensions: [cmp.metrics.width, cmp.metrics.height],
        runs,
        mask_sha256: format!("{:x}", Sha256::digest(&bits)),
        pixels: count,
        error_sum: sum,
        error_mean: (count > 0).then_some(sum / count.max(1) as f64),
        error_max: (count > 0).then_some(max),
        full_frame_value: value,
        without_masks,
    })
}
fn threshold(scope: String, metric: Metric, limit: f64, value: Option<f64>) -> Threshold {
    let value = value.filter(|v| v.is_finite());
    Threshold {
        scope,
        metric: format!("{metric:?}").to_lowercase(),
        limit,
        value,
        headroom: value.map(|v| limit - v),
    }
}

/// Summarizes an existing report without reading or inventing missing captures.
pub fn audit(report: &Report, excluded_captures: Vec<String>) -> Analysis<ExclusionAudit> {
    let entries = report.entries.iter().map(|e| {
        let mut thresholds = vec![threshold("entry".into(), e.metric_used, e.threshold, e.value)];
        for r in &e.regions {
            if let Some(limit) = r.threshold { thresholds.push(threshold(format!("region:{}", r.name), r.metric_used, limit, Some(r.value))); }
        }
        if let Some(limit) = report.config.hotspot_fail { thresholds.push(threshold("hotspot (fails at equality)".into(), Metric::Max, limit, e.metrics.map(|m| m.max))); }
        let mut limitations = Vec::new();
        if e.buffer.is_some() { limitations.push("Numeric buffer channels only; colour FLIP and rendered appearance are not checked.".into()); }
        else if e.hdr.is_some() { limitations.push("HDR-FLIP covers the recorded exposure range; spectral channels, display output and physical luminance calibration are not checked.".into()); }
        else { limitations.push("SDR colour/alpha appearance only; HDR radiance, spectral channels and physical display output are not checked.".into()); }
        if e.pixel_exclusions.is_none() { limitations.push("Resolved mask and excluded error were not recorded; exclusion coverage is unknown.".into()); }
        if report.config.mode == crate::report::Mode::Identity { limitations.push("Thresholds are diagnostic; exact native sample identity determines this verdict.".into()); }
        EntryAudit { name: e.name.clone(), configured_status: e.status, thresholds, informational_regions: e.regions.iter().filter(|r| r.threshold.is_none()).map(|r| r.name.clone()).collect(), ignored_metadata: e.meta_ignored_diff.clone(), validity: e.capture_validity.clone(), limitations }
    }).collect();
    let performance = match report.perf_diff.as_ref().map(|p| p.comparability) {
        Some(crate::perf::Comparability::Qualified) => {
            if report
                .perf_diff
                .as_ref()
                .is_some_and(|p| p.noise_comparability == crate::perf::Comparability::Qualified)
            {
                Capability::Available
            } else {
                Capability::Unknown {
                    reason: "performance_repeat_noise_unqualified".into(),
                }
            }
        }
        Some(crate::perf::Comparability::Rejected) => Capability::Rejected {
            reason: "performance_comparability_rejected".into(),
        },
        _ => Capability::Unknown {
            reason: "performance_missing_or_unqualified".into(),
        },
    };
    let mut provenance = Provenance::native("exclusion-audit/1");
    for entry in &report.entries {
        for (role, hash) in [
            ("baseline", entry.baseline_sha256.clone()),
            ("capture", entry.capture_sha256.clone()),
            (
                "resolved_mask",
                entry
                    .pixel_exclusions
                    .as_ref()
                    .map(|p| p.mask_sha256.clone()),
            ),
        ] {
            provenance
                .resources
                .push(crate::evidence::analysis::Resource {
                    name: format!("{}:{role}", entry.name),
                    role: role.into(),
                    sha256: hash,
                    license: None,
                });
        }
    }
    provenance
        .settings
        .insert("mask_encoding".into(), "row-major 0/1 bytes".into());
    Analysis { capability: Capability::Available, provenance: Some(provenance), evidence: Some(ExclusionAudit { excluded_captures, incomplete_captures: report.entries.iter().filter(|e| !matches!(e.status, Status::Pass | Status::Fail)).map(|e| e.name.clone()).collect(), performance, selection: report.config.entries.clone(), ignore: report.config.ignore.clone(), metadata_ignore: report.config.meta.ignored.clone(), entries }), limitations: vec!["Only supplied capture names are known; no expected-capture manifest was provided.".into(), "Excluded FLIP error is measured on the original full-map neighbourhoods; removing masks is a diagnostic counterfactual, not approval.".into()] }
}

/// Concise stable text for portable report and terminal views.
pub fn text(report: &Report) -> String {
    let Some(a) = &report.exclusion_audit else {
        return "Exclusion audit unavailable (historical or non-comparison producer).".into();
    };
    let Some(e) = &a.evidence else {
        return "Exclusion audit unavailable.".into();
    };
    let mut lines = vec![
        format!(
            "Selection: {:?}; ignore: {:?}; metadata ignore: {:?}.",
            e.selection, e.ignore, e.metadata_ignore
        ),
        format!(
            "Excluded captures: {:?}. Incomplete captures: {:?}. Performance: {:?}.",
            e.excluded_captures, e.incomplete_captures, e.performance
        ),
    ];
    for item in &e.entries {
        lines.push(format!(
            "{}: {:?}; thresholds {:?}; ignored metadata {:?}; validity {:?}; {}",
            item.name,
            item.configured_status,
            item.thresholds,
            item.ignored_metadata,
            item.validity,
            item.limitations.join(" ")
        ));
        if let Some(p) = report
            .entries
            .iter()
            .find(|entry| entry.name == item.name)
            .and_then(|entry| entry.pixel_exclusions.as_ref())
        {
            lines.push(format!(
                "Masked pixels: {}; excluded error mean {:?}, max {:?}; without masks: {:?}.",
                p.pixels, p.error_mean, p.error_max, p.without_masks
            ));
        }
    }
    lines.extend(a.limitations.clone());
    lines.join("\n")
}
