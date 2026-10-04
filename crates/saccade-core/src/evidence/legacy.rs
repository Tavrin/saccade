//! Read-only historical artifacts. Raw records retain all attribution, promotion
//! flags, nulls and missing fields; no adapter can turn one into an attested receipt.
//! Existing schema validators remain shipped. Opaque archival records are not
//! reinterpreted as measured facts or canonical human decisions.
use super::canonical::{self, Digest};
use super::case::Availability;
use super::{Result, require};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::path::Path;

/// Historical schema IDs and original validators, including disabled producer features.
pub const HISTORICAL_SCHEMAS: &[(&str, &str)] = &[
    (
        "saccade-a11y.v1",
        include_str!("../../schemas/saccade-a11y.v1.schema.json"),
    ),
    (
        "saccade-ablate.v1",
        include_str!("../../schemas/saccade-ablate.v1.schema.json"),
    ),
    (
        "saccade-approve.v1",
        include_str!("../../schemas/saccade-approve.v1.schema.json"),
    ),
    (
        "saccade-ask-result.v1",
        include_str!("../../schemas/saccade-ask-result.v1.schema.json"),
    ),
    (
        "saccade-bisect.v1",
        include_str!("../../schemas/saccade-bisect.v1.schema.json"),
    ),
    (
        "saccade-blind-key.v1",
        include_str!("../../schemas/saccade-blind-key.v1.schema.json"),
    ),
    (
        "saccade-calibration.v1",
        include_str!("../../schemas/saccade-calibration.v1.schema.json"),
    ),
    (
        "saccade-decide-result.v1",
        include_str!("../../schemas/saccade-decide-result.v1.schema.json"),
    ),
    (
        "saccade-decision-request.v1",
        include_str!("../../schemas/saccade-decision-request.v1.schema.json"),
    ),
    (
        "saccade-decisions.v1",
        include_str!("../../schemas/saccade-decisions.v1.schema.json"),
    ),
    (
        "saccade-entries.v1",
        include_str!("../../schemas/saccade-entries.v1.schema.json"),
    ),
    (
        "saccade-error.v1",
        include_str!("../../schemas/saccade-error.v1.schema.json"),
    ),
    (
        "saccade-explain-blind-key.v1",
        include_str!("../../schemas/saccade-explain-blind-key.v1.schema.json"),
    ),
    (
        "saccade-explain-result.v1",
        include_str!("../../schemas/saccade-explain-result.v1.schema.json"),
    ),
    (
        "saccade-explain.v1",
        include_str!("../../schemas/saccade-explain.v1.schema.json"),
    ),
    (
        "saccade-inbox-item.v1",
        include_str!("../../schemas/saccade-inbox-item.v1.schema.json"),
    ),
    (
        "saccade-judge-bench.v1",
        include_str!("../../schemas/saccade-judge-bench.v1.schema.json"),
    ),
    (
        "saccade-judge-selftest.v1",
        include_str!("../../schemas/saccade-judge-selftest.v1.schema.json"),
    ),
    (
        "saccade-judge-vote-api.v1",
        include_str!("../../schemas/saccade-judge-vote-api.v1.schema.json"),
    ),
    (
        "saccade-judge-votes.v1",
        include_str!("../../schemas/saccade-judge-votes.v1.schema.json"),
    ),
    (
        "saccade-judge.v1",
        include_str!("../../schemas/saccade-judge.v1.schema.json"),
    ),
    (
        "saccade-labels.v1",
        include_str!("../../schemas/saccade-labels.v1.schema.json"),
    ),
    (
        "saccade-noise.v1",
        include_str!("../../schemas/saccade-noise.v1.schema.json"),
    ),
    (
        "saccade-perf-diff.v1",
        include_str!("../../schemas/saccade-perf-diff.v1.schema.json"),
    ),
    (
        "saccade-perf.v1",
        include_str!("../../schemas/saccade-perf.v1.schema.json"),
    ),
    (
        "saccade-rank.v1",
        include_str!("../../schemas/saccade-rank.v1.schema.json"),
    ),
    (
        "saccade-report.v1",
        include_str!("../../schemas/saccade-report.v1.schema.json"),
    ),
    (
        "saccade-result.v1",
        include_str!("../../schemas/saccade-result.v1.schema.json"),
    ),
    (
        "saccade-review.v1",
        include_str!("../../schemas/saccade-review.v1.schema.json"),
    ),
    (
        "saccade-runs.v1",
        include_str!("../../schemas/saccade-runs.v1.schema.json"),
    ),
    (
        "saccade-safety.v1",
        include_str!("../../schemas/saccade-safety.v1.schema.json"),
    ),
    (
        "saccade-sequence.v1",
        include_str!("../../schemas/saccade-sequence.v1.schema.json"),
    ),
    (
        "saccade-summary.v1",
        include_str!("../../schemas/saccade-summary.v1.schema.json"),
    ),
    (
        "saccade-view-summary.v1",
        include_str!("../../schemas/saccade-view-summary.v1.schema.json"),
    ),
];

/// Returns the preserved validator; pre-rename flipdiff IDs resolve to the same shape.
pub fn schema(identifier: &str) -> Option<Cow<'static, str>> {
    let name = identifier
        .strip_prefix("flipdiff-")
        .map(|s| format!("saccade-{s}"));
    let normalized = name.as_deref().unwrap_or(identifier);
    let validator = HISTORICAL_SCHEMAS
        .iter()
        .find(|(id, _)| *id == normalized)
        .map(|(_, schema)| *schema)?;
    if name.is_some() {
        // Adapt validator identity only; never rewrite historical evidence.
        Some(Cow::Owned(
            validator
                .replace(&format!("\"{normalized}\""), &format!("\"{identifier}\""))
                .replace(
                    &format!("/{normalized}.schema.json"),
                    &format!("/{identifier}.schema.json"),
                ),
        ))
    } else {
        Some(Cow::Borrowed(validator))
    }
}

/// Historical authority is audit evidence, not permission to apply a baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// A recorded model/source promotion; never a human approval.
    ModelPromoted,
    /// A human source label was recorded; its identity was not attested.
    HumanLabelUnattested,
    /// Advice only, without a final disposition.
    ProposalOnly,
    /// Final/source origin was not established by the historical record.
    Unknown,
}
/// One historical disposition, retained without invented input/review bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalDecision {
    /// Historical entry ID.
    pub entry_id: String,
    /// Original final disposition, including ambiguous ties or nulls.
    pub disposition: Option<String>,
    /// Conservative interpretation of recorded authority.
    pub authority: Authority,
    /// Exact proposal objects with their source/promoted/proposed flags.
    pub proposals: Vec<Value>,
    /// Input hashes, missing rather than guessed from current files.
    pub input_hashes: Availability<Vec<Option<String>>>,
    /// An old record has no canonical case binding.
    pub case_binding: Availability<Digest>,
}
/// A lossless historical read plus explicit limits on normalized interpretation.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalArtifact {
    /// Original schema, including a pre-rename identifier when supplied.
    pub schema: String,
    /// Exact file bytes' digest, not a semantic case ID.
    pub sha256: Digest,
    /// Original JSON, preserving nulls and absent fields.
    pub raw: Value,
    /// Historical dispositions; these are deliberately not HumanDecision records.
    pub decisions: Vec<HistoricalDecision>,
    /// Qualification absent from perf v1 remains missing.
    pub qualification: Availability<Value>,
    /// Unsupported inference/authority claims remain visible.
    pub limits: Vec<String>,
}
impl HistoricalArtifact {
    /// Reads a known historical artifact without rewriting it or reading current inputs.
    pub fn read(path: &Path) -> Result<Self> {
        Self::from_bytes(&std::fs::read(crate::paths::native(path))?)
    }
    /// Checks the schema discriminator, required top-level keys and typed shapes where
    /// existing always-available types exist. Full schema validators remain accessible
    /// through schema(); opaque records retain their original data without coercion.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let raw: Value = canonical::decode(bytes)?;
        let identifier = raw.get("schema").and_then(Value::as_str).ok_or_else(|| {
            super::ContractError::Invalid("historical artifact needs a schema ID".into())
        })?;
        let validator = schema(identifier).ok_or_else(|| {
            super::ContractError::Invalid(format!("unsupported historical schema {identifier:?}"))
        })?;
        let shape: Value = serde_json::from_str(&validator)?;
        if let Some(required) = shape.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                require(
                    raw.get(key).is_some(),
                    &format!("historical artifact is missing {key}"),
                )?;
            }
        }
        let normalized = identifier
            .strip_prefix("flipdiff-")
            .map_or_else(|| identifier.to_owned(), |s| format!("saccade-{s}"));
        match normalized.as_str() {
            "saccade-report.v1" => {
                serde_json::from_value::<crate::Report>(raw.clone())?;
            }
            "saccade-decisions.v1" => {
                serde_json::from_value::<crate::view::Decisions>(raw.clone())?;
            }
            "saccade-explain.v1" => {
                serde_json::from_value::<crate::explain::ExplainPack>(raw.clone())?;
            }
            "saccade-blind-key.v1" => {
                serde_json::from_value::<crate::view::BlindKey>(raw.clone())?;
            }
            "saccade-explain-blind-key.v1" => {
                serde_json::from_value::<crate::explain::ExplainBlindKey>(raw.clone())?;
            }
            "saccade-inbox-item.v1" => {
                serde_json::from_value::<crate::inbox::Item>(raw.clone())?;
            }
            "saccade-judge-votes.v1" => {
                serde_json::from_value::<crate::judge_vote::VoteRun>(raw.clone())?;
            }
            "saccade-labels.v1" => {
                serde_json::from_value::<crate::labels::Labels>(raw.clone())?;
            }
            "saccade-perf.v1" => {
                serde_json::from_value::<crate::perf::CapturePerf>(raw.clone())?;
            }
            "saccade-ablate.v1" => {
                serde_json::from_value::<crate::ablate::Ablation>(raw.clone())?;
            }
            "saccade-sequence.v1" => {
                serde_json::from_value::<crate::sequence::SequenceReport>(raw.clone())?;
            }
            "saccade-rank.v1" => {
                serde_json::from_value::<crate::rank::RankReport>(raw.clone())?;
            }
            "saccade-bisect.v1" => {
                serde_json::from_value::<crate::bisect::BisectResult>(raw.clone())?;
            }
            _ => {}
        }
        let decisions = if normalized == "saccade-decisions.v1" {
            read_decisions(&raw)?
        } else {
            Vec::new()
        };
        let qualification = present(
            raw.get("qualification"),
            "historical artifact lacks qualification",
        );
        Ok(Self { schema: identifier.into(), sha256: Digest::of_bytes(bytes), raw, decisions, qualification,
            limits: vec!["Historical source labels and promoted finals do not establish attested human approval.".into(),
                "Absent context, repeat noise, semantic labels and provider responses remain missing.".into()] })
    }
    /// Selects a historical field without converting missing/null values into zero,
    /// a background label, or an abstention. JSON pointer syntax is explicit.
    pub fn evidence(&self, pointer: &str) -> Availability<Value> {
        present(
            self.raw.pointer(pointer),
            &format!("historical artifact lacks {pointer}"),
        )
    }
}
fn present(value: Option<&Value>, reason: &str) -> Availability<Value> {
    match value {
        Some(value) if !value.is_null() => Availability::Available {
            value: value.clone(),
        },
        _ => Availability::missing(reason),
    }
}
fn read_decisions(raw: &Value) -> Result<Vec<HistoricalDecision>> {
    let mut decisions = Vec::new();
    if let Some(sets) = raw.get("sets").and_then(Value::as_array) {
        for set in sets {
            let entry_id = set
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    super::ContractError::Invalid("historical decision lacks entry name".into())
                })?
                .to_owned();
            let proposals = set
                .get("proposals")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let disposition = set
                .get("decision")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let authority = if proposals
                .iter()
                .any(|p| p.get("promoted").and_then(Value::as_bool) == Some(true))
            {
                Authority::ModelPromoted
            } else if disposition.is_none() {
                Authority::ProposalOnly
            } else if proposals.iter().any(|p| {
                p.get("source")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.eq_ignore_ascii_case("human"))
                    && p.get("proposed").and_then(Value::as_bool) == Some(false)
            }) {
                Authority::HumanLabelUnattested
            } else {
                Authority::Unknown
            };
            let input_hashes = match set.get("sha256") {
                Some(Value::Array(hashes)) if !hashes.is_empty() => Availability::Available {
                    value: serde_json::from_value(Value::Array(hashes.clone()))?,
                },
                _ => Availability::missing("historical decision lacks reviewed input hashes"),
            };
            decisions.push(HistoricalDecision {
                entry_id,
                disposition,
                authority,
                proposals,
                input_hashes,
                case_binding: Availability::missing(
                    "historical decision lacks a canonical case/review binding",
                ),
            });
        }
    }
    Ok(decisions)
}
