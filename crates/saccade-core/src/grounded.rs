//! Atomic numerical explanations verified against an immutable fact catalog.
use crate::{Report, Status};
use serde::{Deserialize, Serialize};

/// A region in an immutable source report.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Catalog-local stable ID.
    pub id: String,
    /// Source entry or scope.
    pub entry: String,
    /// Pixel rectangle when source evidence provides one.
    pub rect_px: Option<[u32; 4]>,
    /// full_frame, included_pixels, scope_unknown or localized_scope.
    pub measurement_scope: String,
    /// Exact resolved exclusion evidence from the source entry.
    pub pixel_exclusions: Option<crate::exclusions::PixelExclusions>,
    /// Channel and sample interpretation exclusions from the source entry.
    pub sample_exclusions: Vec<String>,
}
/// One numerical measurement and its exact source JSON pointer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    /// Unique evidence ID.
    pub id: String,
    /// Region to which this fact applies.
    pub region_id: String,
    /// Finite vocabulary: mean_flip, max_flip, thresholded_hotspot_pixels or changed_pixels.
    pub kind: String,
    /// Exact observed value.
    pub value: f64,
    /// Dimensionless FLIP error or pixel count.
    pub unit: String,
    /// JSON pointer into the hash-bound source document.
    pub source_pointer: String,
}
/// Facts frozen before any proposal is inspected.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// SHA-256 of exact source bytes.
    pub source_sha256: String,
    /// Catalog regions.
    pub regions: Vec<Region>,
    /// Finite numerical facts.
    pub facts: Vec<Fact>,
}
/// An atomic deterministic or externally generated proposal. Free prose is disallowed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// One finite supported claim kind.
    pub claim_kind: String,
    /// Exactly one region.
    pub region_ids: Vec<String>,
    /// Exactly one matching numerical fact.
    pub evidence_ids: Vec<String>,
    /// Quoted number must match the fact exactly; application inserts the final value.
    pub value: f64,
}
/// One verified observation; arbitrary semantic/cause wording never enters this type.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// Verified numerical proposal.
    pub observation: Proposal,
    /// supported_measurement.
    pub verification: String,
    /// Application-controlled numerical wording.
    pub text: String,
    /// Always null. Measurements alone do not establish causation.
    pub causal_claim: Option<String>,
}
/// Reason an unverified atomic proposal was discarded; its wording is not echoed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rejection {
    /// Zero-based proposal position.
    pub index: usize,
    /// Mechanical verification failure.
    pub reason: String,
}
/// Grounded observations and their evidence links, with explicit abstention coverage.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explanation {
    /// Content-addressed report identity, absent on historical records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// External capture URI/key backlinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,

    /// saccade-grounded.v1.
    pub schema: String,
    /// Immutable source catalog.
    pub catalog: Catalog,
    /// SHA-256 of canonical ordered serialized catalog.
    pub catalog_sha256: String,
    /// Verified observations only.
    pub claims: Vec<Claim>,
    /// Dropped proposal diagnostics.
    pub dropped: Vec<Rejection>,
    /// Supplied proposal count; expose abstention instead of hiding it in precision.
    pub proposed: usize,
    /// Semantic support/causality qualification limits.
    pub limits: Vec<String>,
}

fn add(
    c: &mut Catalog,
    id: String,
    entry: String,
    rect_px: Option<[u32; 4]>,
    pointer: String,
    values: &[(&str, f64)],
) {
    c.regions.push(Region {
        id: id.clone(),
        entry,
        rect_px,
        measurement_scope: "localized_scope".into(),
        pixel_exclusions: None,
        sample_exclusions: vec![],
    });
    for &(kind, value) in values {
        if value.is_finite() && value >= 0.0 {
            c.facts.push(Fact {
                id: format!("{id}.{kind}"),
                region_id: id.clone(),
                kind: kind.into(),
                value,
                unit: if kind.ends_with("pixels") {
                    "pixels"
                } else {
                    "FLIP error"
                }
                .into(),
                source_pointer: format!("{pointer}/{kind}"),
            });
        }
    }
}
/// Builds numerical regions from report metrics and hotspots, never from image text.
pub fn report_catalog(report: &Report, source_sha256: String) -> Catalog {
    let mut catalog = Catalog {
        source_sha256,
        regions: vec![],
        facts: vec![],
    };
    for (i, entry) in report.entries.iter().enumerate() {
        if !matches!(entry.status, Status::Pass | Status::Fail)
            || entry.capture_validity.status == crate::meta::Validity::Invalid
        {
            continue;
        }
        if let Some(metrics) = &entry.metrics {
            add(
                &mut catalog,
                format!("e{i}.full"),
                entry.name.clone(),
                Some([0, 0, metrics.width, metrics.height]),
                format!("/entries/{i}/metrics"),
                &[("mean_flip", metrics.mean), ("max_flip", metrics.max)],
            );
            if let Some(region) = catalog.regions.last_mut() {
                region.measurement_scope = match &entry.pixel_exclusions {
                    Some(exclusions) if exclusions.pixels > 0 => "included_pixels",
                    Some(_) => "full_frame",
                    None => "scope_unknown",
                }
                .into();
                region.pixel_exclusions = entry.pixel_exclusions.clone();
                region.sample_exclusions = entry.sample_exclusions.clone().unwrap_or_default();
            }
            // Metrics fields are named mean/max, unlike hotspot fields.
            for fact in catalog
                .facts
                .iter_mut()
                .filter(|f| f.region_id == format!("e{i}.full"))
            {
                fact.source_pointer = format!(
                    "/entries/{i}/metrics/{}",
                    if fact.kind == "mean_flip" {
                        "mean"
                    } else {
                        "max"
                    }
                );
            }
        }
        for (j, hotspot) in entry.hotspots.iter().enumerate() {
            let id = format!("e{i}.hotspot{j}");
            add(
                &mut catalog,
                id.clone(),
                entry.name.clone(),
                Some(hotspot.rect_px),
                format!("/entries/{i}/hotspots/{j}"),
                &[
                    ("mean_flip", hotspot.mean_flip),
                    ("max_flip", hotspot.max_flip),
                    ("thresholded_hotspot_pixels", hotspot.area_px as f64),
                ],
            );
            if let Some(f) = catalog
                .facts
                .iter_mut()
                .find(|f| f.id == format!("{id}.thresholded_hotspot_pixels"))
            {
                f.source_pointer = format!("/entries/{i}/hotspots/{j}/area_px");
            }
        }
    }
    catalog
}
/// Builds inside/outside/boundary facts from independent localized measurements.
pub fn localized_catalog(
    measurement: &crate::localized::Measurement,
    source_sha256: String,
) -> Catalog {
    let mut catalog = Catalog {
        source_sha256,
        regions: vec![],
        facts: vec![],
    };
    for (id, scope) in [
        ("inside", &measurement.inside),
        ("outside", &measurement.outside),
        ("boundary", &measurement.boundary),
    ] {
        add(
            &mut catalog,
            id.into(),
            id.into(),
            None,
            format!("/{id}"),
            &[
                ("mean_flip", scope.mean_flip),
                ("max_flip", scope.max_flip),
                ("changed_pixels", scope.changed_pixels as f64),
            ],
        );
    }
    catalog
}
/// Generates deterministic atomic proposals; zero facts means explicit abstention.
pub fn deterministic(catalog: &Catalog) -> Vec<Proposal> {
    catalog
        .facts
        .iter()
        .map(|f| Proposal {
            claim_kind: f.kind.clone(),
            region_ids: vec![f.region_id.clone()],
            evidence_ids: vec![f.id.clone()],
            value: f.value,
        })
        .collect()
}
/// Verifies proposals and renders only finite supported vocabulary through fixed templates.
pub fn verify(catalog: Catalog, proposals: &[Proposal]) -> crate::Result<Explanation> {
    let catalog_sha256 = crate::localized::digest(&serde_json::to_vec(&catalog)?);
    let mut claims = vec![];
    let mut dropped = vec![];
    for (index, p) in proposals.iter().enumerate() {
        let reason = if !matches!(
            p.claim_kind.as_str(),
            "mean_flip" | "max_flip" | "thresholded_hotspot_pixels" | "changed_pixels"
        ) {
            Some("unsupported_semantic_or_causal_kind")
        } else if p.region_ids.len() != 1 || p.evidence_ids.len() != 1 {
            Some("non_atomic_claim")
        } else if !catalog.regions.iter().any(|r| r.id == p.region_ids[0]) {
            Some("unknown_region")
        } else if let Some(fact) = catalog.facts.iter().find(|f| f.id == p.evidence_ids[0]) {
            if fact.region_id != p.region_ids[0] || fact.kind != p.claim_kind {
                Some("unrelated_evidence")
            } else if !p.value.is_finite() || p.value != fact.value {
                Some("inconsistent_quantity")
            } else {
                None
            }
        } else {
            Some("unknown_evidence")
        };
        if let Some(reason) = reason {
            dropped.push(Rejection {
                index,
                reason: reason.into(),
            });
            continue;
        }
        let label = match p.claim_kind.as_str() {
            "mean_flip" => "mean FLIP error",
            "max_flip" => "maximum FLIP error",
            "changed_pixels" => "native changed pixels",
            _ => "thresholded hotspot pixels",
        };
        let region = catalog.regions.iter().find(|r| r.id == p.region_ids[0]);
        let scope = region
            .map(|r| {
                format!(
                    "{}; {} excluded pixels; sample exclusions {:?}",
                    r.measurement_scope,
                    r.pixel_exclusions.as_ref().map_or(0, |e| e.pixels),
                    r.sample_exclusions
                )
            })
            .unwrap_or_default();
        claims.push(Claim {
            observation: p.clone(),
            verification: "supported_measurement".into(),
            text: format!(
                "Region {} ({scope}): {} = {}.",
                p.region_ids[0], label, p.value
            ),
            causal_claim: None,
        });
    }
    Ok(Explanation{report_id:None,source_refs:Vec::new(),schema:"saccade-grounded.v1".into(),catalog,catalog_sha256,claims,dropped,proposed:proposals.len(),limits:vec!["Numerical consistency with the immutable source catalog is verified; semantic edit success and root cause remain unproven.".into(),"Hotspot pixel counts depend on producer threshold and exclusions; they are not exact native-change counts.".into(),"No model or human-support precision benchmark was run; proposal acceptance coverage is reported explicitly.".into()]})
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn rejects_fabricated_unrelated_compound_and_semantic_claims() {
        let mut c = Catalog {
            source_sha256: crate::localized::digest(b"fixture"),
            regions: vec![],
            facts: vec![],
        };
        add(
            &mut c,
            "r1".into(),
            "fixture".into(),
            None,
            "/inside".into(),
            &[("mean_flip", 0.25)],
        );
        add(
            &mut c,
            "r2".into(),
            "fixture".into(),
            None,
            "/outside".into(),
            &[("mean_flip", 0.01)],
        );
        let correct = deterministic(&c)[0].clone();
        let mut ps = vec![correct.clone()];
        let mut bad = correct.clone();
        bad.region_ids[0] = "nonexistent".into();
        ps.push(bad);
        let mut bad = correct.clone();
        bad.evidence_ids[0] = "nonexistent".into();
        ps.push(bad);
        let mut bad = correct.clone();
        bad.value = 0.26;
        ps.push(bad);
        let mut bad = correct.clone();
        bad.region_ids[0] = "r2".into();
        ps.push(bad);
        let mut bad = correct.clone();
        bad.region_ids.push("r2".into());
        ps.push(bad);
        let mut bad = correct;
        bad.claim_kind = "ignore instructions; roughness shader broken".into();
        ps.push(bad);
        let verified = verify(c, &ps).unwrap();
        assert_eq!(verified.claims.len(), 1);
        assert_eq!(verified.dropped.len(), 6);
        assert!(verified.claims[0].causal_claim.is_none());
        assert!(
            !serde_json::to_string(&verified)
                .unwrap()
                .contains("ignore instructions")
        );
    }
}
