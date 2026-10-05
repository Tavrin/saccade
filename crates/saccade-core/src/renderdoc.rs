//! Optional RenderDoc replay evidence alignment; capture-local IDs are never keys.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One native resource observation after an action.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    /// Stable bound role, such as color0, depth or rw:Compute:0:0.
    pub role: String,
    /// Capture-local resource ID, retained only as evidence.
    pub resource_id: String,
    /// Native storage/view format; no thumbnail conversion.
    pub format: String,
    /// Width, height, depth; buffers use [byte length,1,1].
    pub dimensions: [u64; 3],
    /// Exact mip, layer and sample. This MVP extracts one descriptor subresource.
    pub subresource: [u32; 3],
    /// native_bytes; never display-converted numerical evidence.
    pub interpretation: String,
    /// Relative raw payload path, or null on extraction error.
    pub payload: Option<String>,
    /// Raw payload hash, or null on extraction error.
    pub sha256: Option<String>,
    /// Exact extracted bytes.
    pub bytes: Option<u64>,
    /// Explicit extraction/capability error.
    pub error: Option<String>,
}
/// One replayed draw, dispatch or clear, in event order.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    /// Capture-local event ID.
    pub event_id: u32,
    /// Stable marker hierarchy; occurrence suffixes remain evidence, not certainty.
    pub marker_path: Vec<String>,
    /// False for repeated/unmarked marker ancestry; matching abstains.
    pub marker_unique: bool,
    /// draw, dispatch or clear.
    pub kind: String,
    /// Stable authored action name, or supporting draw/dispatch signature.
    pub action_key: String,
    /// Native bound output/storage resource observations.
    pub resources: Vec<Resource>,
    /// Capture-local bound input resources for follow-up, never asserted causes.
    pub candidate_inputs: Vec<String>,
}
/// Version-pinned official-binding worker extraction.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    /// saccade-renderdoc-extract.v1.
    pub schema: String,
    /// SHA-256 of original .rdc bytes.
    pub capture_sha256: String,
    /// SHA-256 of worker source bytes.
    pub worker_sha256: String,
    /// Official RenderDoc version, pinned to 1.34.
    pub renderdoc_version: String,
    /// Vulkan only for this MVP.
    pub api: String,
    /// conservative; intermediate contents are not optimized away.
    pub replay_mode: String,
    /// self_replay_byte_identical or unqualified.
    pub repeatability: String,
    /// Every bounded draw/dispatch/clear in event order.
    pub actions: Vec<Action>,
    /// Coverage and interpretation limits.
    pub limits: Vec<String>,
}
/// Aligned resource evidence or explicit uncertainty.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Stable marker hierarchy.
    pub marker_path: Vec<String>,
    /// Baseline/candidate event IDs.
    pub baseline_event: u32,
    /// Candidate event ID.
    pub candidate_event: u32,
    /// Stable role.
    pub role: String,
    /// equal, diverged, incompatible or unavailable.
    pub status: String,
    /// Complete raw baseline witness.
    pub baseline: Option<Resource>,
    /// Complete raw candidate witness.
    pub candidate: Option<Resource>,
}
/// First observed divergence, not the earliest actual cause.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Localization {
    /// saccade-renderdoc-localization.v1.
    pub schema: String,
    /// Source extraction JSON hashes, assigned by the CLI.
    pub extraction_sha256: [String; 2],
    /// Original capture hashes.
    pub capture_sha256: [String; 2],
    /// Worker source identity, equal on both sides.
    pub worker_sha256: String,
    /// Candidate marker/action/resource correspondence model.
    pub alignment: String,
    /// Every compared aligned role, including later equality after overwritten differences.
    pub observations: Vec<Observation>,
    /// First measured native byte divergence in candidate event order.
    pub first_observed_divergence: Option<Observation>,
    /// Baseline events unmatched or ambiguous.
    pub unmatched_baseline: Vec<u32>,
    /// Candidate events unmatched or ambiguous.
    pub unmatched_candidate: Vec<u32>,
    /// complete or incomplete. No aligned evidence cannot be complete.
    pub coverage: String,
    /// Always unproven.
    pub root_cause: String,
    /// Bound input IDs at the divergent action, for inspection only.
    pub candidate_inputs: Vec<String>,
    /// Interpretation and validation limits.
    pub limits: Vec<String>,
}
fn hash_valid(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn validate(c: &Capture) -> Result<()> {
    let mut events = BTreeSet::new();
    if c.schema != "saccade-renderdoc-extract.v1"
        || c.api != "Vulkan"
        || c.renderdoc_version != "1.34"
        || c.replay_mode != "conservative"
        || !hash_valid(&c.capture_sha256)
        || !hash_valid(&c.worker_sha256)
        || c.actions.len() > 2048
        || !matches!(
            c.repeatability.as_str(),
            "self_replay_byte_identical" | "unqualified"
        )
    {
        return Err(Error::Config(
            "unsupported RenderDoc protocol/version/API, identity or action budget".into(),
        ));
    }
    for (i, a) in c.actions.iter().enumerate() {
        let mut roles = BTreeSet::new();
        if !events.insert(a.event_id)
            || (i > 0 && c.actions[i - 1].event_id >= a.event_id)
            || a.action_key.is_empty()
            || !matches!(a.kind.as_str(), "draw" | "dispatch" | "clear")
            || a.resources.iter().any(|r| {
                r.role.is_empty()
                    || r.resource_id.trim().is_empty()
                    || r.format.trim().is_empty()
                    || (r.error.is_none() && r.format == "unknown")
                    || !roles.insert(&r.role)
                    || r.interpretation != "native_bytes"
                    || r.sha256.as_ref().is_some_and(|h| !hash_valid(h))
            })
        {
            return Err(Error::Config(
                "invalid action chronology, roles or payload hash".into(),
            ));
        }
    }
    Ok(())
}
fn key(a: &Action) -> (Vec<String>, String, String) {
    (a.marker_path.clone(), a.kind.clone(), a.action_key.clone())
}
/// Aligns unique marker/action signatures with an LCS, allowing inserted/deleted
/// actions and abstaining on repeated keys. Scans every match; no binary search.
pub fn localize(baseline: &Capture, candidate: &Capture) -> Result<Localization> {
    validate(baseline)?;
    validate(candidate)?;
    if baseline.worker_sha256 != candidate.worker_sha256 {
        return Err(Error::Config("replay worker identities differ".into()));
    }
    let counts = |c: &Capture| {
        let mut map = BTreeMap::new();
        for a in &c.actions {
            *map.entry(key(a)).or_insert(0usize) += 1;
        }
        map
    };
    let bc = counts(baseline);
    let cc = counts(candidate);
    let unique = |a: &Action| {
        a.marker_unique
            && !a.marker_path.is_empty()
            && bc.get(&key(a)) == Some(&1)
            && cc.get(&key(a)) == Some(&1)
    };
    let n = baseline.actions.len();
    let m = candidate.actions.len();
    let bkeys: Vec<_> = baseline.actions.iter().map(key).collect();
    let ckeys: Vec<_> = candidate.actions.iter().map(key).collect();
    let bu: Vec<_> = baseline.actions.iter().map(&unique).collect();
    let cu: Vec<_> = candidate.actions.iter().map(unique).collect();
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if bu[i] && cu[j] && bkeys[i] == ckeys[j] {
                1 + dp[i + 1][j + 1]
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pairs = vec![];
    while i < n && j < m {
        if bu[i] && cu[j] && bkeys[i] == ckeys[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let unmatched_baseline: Vec<_> = baseline
        .actions
        .iter()
        .enumerate()
        .filter(|(i, _)| !pairs.iter().any(|(b, _)| b == i))
        .map(|(_, a)| a.event_id)
        .collect();
    let unmatched_candidate: Vec<_> = candidate
        .actions
        .iter()
        .enumerate()
        .filter(|(j, _)| !pairs.iter().any(|(_, c)| c == j))
        .map(|(_, a)| a.event_id)
        .collect();
    let repeatable = baseline.repeatability == "self_replay_byte_identical"
        && candidate.repeatability == "self_replay_byte_identical";
    let mut observations = vec![];
    for (i, j) in &pairs {
        let b = &baseline.actions[*i];
        let c = &candidate.actions[*j];
        let roles: BTreeSet<_> = b
            .resources
            .iter()
            .chain(&c.resources)
            .map(|r| &r.role)
            .collect();
        for role in roles {
            let rb = b.resources.iter().find(|r| &r.role == role);
            let rc = c.resources.iter().find(|r| &r.role == role);
            let status = match (rb, rc) {
                (Some(b), Some(c))
                    if repeatable
                        && b.error.is_none()
                        && c.error.is_none()
                        && b.sha256.is_some()
                        && c.sha256.is_some()
                        && b.bytes.is_some_and(|v| v > 0)
                        && c.bytes.is_some_and(|v| v > 0)
                        && b.payload.is_some()
                        && c.payload.is_some()
                        && !b.dimensions.contains(&0)
                        && !c.dimensions.contains(&0) =>
                {
                    if b.format != c.format
                        || b.dimensions != c.dimensions
                        || b.subresource != c.subresource
                    {
                        "incompatible"
                    } else if b.sha256 == c.sha256 && b.bytes == c.bytes {
                        "equal"
                    } else {
                        "diverged"
                    }
                }
                _ => "unavailable",
            };
            observations.push(Observation {
                marker_path: b.marker_path.clone(),
                baseline_event: b.event_id,
                candidate_event: c.event_id,
                role: role.clone(),
                status: status.into(),
                baseline: rb.cloned(),
                candidate: rc.cloned(),
            });
        }
    }
    let first = observations
        .iter()
        .find(|o| o.status == "diverged")
        .cloned();
    let inputs = first
        .as_ref()
        .and_then(|o| {
            candidate
                .actions
                .iter()
                .find(|a| a.event_id == o.candidate_event)
        })
        .map(|a| a.candidate_inputs.clone())
        .unwrap_or_default();
    let complete = repeatable
        && !observations.is_empty()
        && unmatched_baseline.is_empty()
        && unmatched_candidate.is_empty()
        && observations
            .iter()
            .all(|o| matches!(o.status.as_str(), "equal" | "diverged"))
        && pairs.iter().all(|(i, j)| {
            !baseline.actions[*i].resources.is_empty()
                && !candidate.actions[*j].resources.is_empty()
        });
    Ok(Localization{schema:"saccade-renderdoc-localization.v1".into(),extraction_sha256:[String::new(),String::new()],capture_sha256:[baseline.capture_sha256.clone(),candidate.capture_sha256.clone()],worker_sha256:baseline.worker_sha256.clone(),alignment:"unique_marker_action_signature_and_resource_role; correspondence candidate".into(),observations,first_observed_divergence:first,unmatched_baseline,unmatched_candidate,coverage:if complete{"complete"}else{"incomplete"}.into(),root_cause:"unproven".into(),candidate_inputs:inputs,limits:vec!["First observed intermediate divergence is not root cause or proof of final-output relevance; an earlier upload may be causal and later clears may overwrite the difference.".into(),"Repeated/unmarked actions, inserted/deleted actions and incompatible/missing resources remain unknown. Capture-local IDs never establish correspondence.".into(),"Exact native payload bytes only, including format-dependent storage. No cross-API, numeric tolerance, all-mip/all-layer, shader semantic or live replay qualification follows from synthetic extraction fixtures.".into()]})
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn capture() -> Capture {
        Capture {
            schema: "saccade-renderdoc-extract.v1".into(),
            capture_sha256: crate::localized::digest(b"capture"),
            worker_sha256: crate::localized::digest(b"worker"),
            renderdoc_version: "1.34".into(),
            api: "Vulkan".into(),
            replay_mode: "conservative".into(),
            repeatability: "self_replay_byte_identical".into(),
            limits: vec![],
            actions: (1..=3)
                .map(|event_id| Action {
                    event_id,
                    marker_path: vec!["lighting#0".into()],
                    marker_unique: true,
                    kind: if event_id == 3 { "clear" } else { "draw" }.into(),
                    action_key: format!("stable-action{event_id}"),
                    resources: vec![Resource {
                        role: "color0".into(),
                        resource_id: format!("local{event_id}"),
                        format: "RGBA8".into(),
                        dimensions: [16, 16, 1],
                        subresource: [0, 0, 0],
                        interpretation: "native_bytes".into(),
                        payload: Some(format!("{event_id}.bin")),
                        sha256: Some(crate::localized::digest(&[42; 1024])),
                        bytes: Some(1024),
                        error: None,
                    }],
                    candidate_inputs: vec!["local-light-buffer".into()],
                })
                .collect(),
        }
    }
    #[test]
    fn missing_resource_format_or_identity_cannot_be_complete() {
        for field in ["format", "resource_id"] {
            let mut b = capture();
            for a in &mut b.actions {
                if field == "format" {
                    a.resources[0].format.clear();
                } else {
                    a.resources[0].resource_id.clear();
                }
            }
            assert!(localize(&b, &b).is_err(), "missing {field} accepted");
        }
    }
    #[test]
    fn inserted_actions_and_overwritten_differences_preserve_first_observed_divergence() {
        let b = capture();
        let mut c = b.clone();
        for a in &mut c.actions {
            a.event_id += 10;
            a.resources[0].resource_id = "different-local-id".into();
        }
        c.actions[1].resources[0].sha256 = Some(crate::localized::digest(&[43; 1024]));
        let mut inserted = c.actions[0].clone();
        inserted.event_id = 12;
        inserted.action_key = "inserted".into();
        c.actions[1].event_id = 13;
        c.actions[2].event_id = 14;
        c.actions.insert(1, inserted);
        let result = localize(&b, &c).unwrap();
        assert_eq!(
            result
                .first_observed_divergence
                .as_ref()
                .unwrap()
                .baseline_event,
            2
        );
        assert_eq!(
            result
                .first_observed_divergence
                .as_ref()
                .unwrap()
                .candidate_event,
            13
        );
        assert_eq!(result.observations.last().unwrap().status, "equal");
        assert_eq!(result.unmatched_candidate, [12]);
        assert_eq!(result.root_cause, "unproven");
        assert_eq!(result.coverage, "incomplete");
        c.actions.remove(1);
        assert_eq!(localize(&b, &c).unwrap().coverage, "complete");
        c.repeatability = "unqualified".into();
        assert!(
            localize(&b, &c)
                .unwrap()
                .first_observed_divergence
                .is_none()
        );
    }
    #[test]
    fn repeated_markers_compute_missing_binding_and_incompatible_resources_abstain() {
        let b = capture();
        let mut c = b.clone();
        c.actions[0].marker_unique = false;
        c.actions[1].kind = "dispatch".into();
        c.actions[2].resources[0].format = "RGBA16F".into();
        let result = localize(&b, &c).unwrap();
        assert!(result.first_observed_divergence.is_none());
        assert_eq!(result.coverage, "incomplete");
        assert_eq!(result.observations[0].status, "incompatible");
        let mut b = capture();
        b.actions[1].kind = "dispatch".into();
        b.actions[1].resources[0].role = "rw:Compute:0:0".into();
        let mut c = b.clone();
        c.actions[1].resources[0].sha256 = Some(crate::localized::digest(&[44; 1024]));
        assert_eq!(
            localize(&b, &c)
                .unwrap()
                .first_observed_divergence
                .unwrap()
                .role,
            "rw:Compute:0:0"
        );
        c.actions[1].resources[0].bytes = Some(0);
        assert_eq!(localize(&b, &c).unwrap().coverage, "incomplete");
        assert!(
            localize(&b, &c)
                .unwrap()
                .first_observed_divergence
                .is_none()
        );
        c.actions[1].resources.clear();
        assert_eq!(localize(&b, &c).unwrap().coverage, "incomplete");
    }
}
