//! Closed proposed answers. Adapter audit identity is separate from model text.
use super::canonical::{self, Digest};
use super::request::{DecisionRequest, ProviderIdentity};
use super::{Result, require};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// Untrusted provider body; no source labels, audit fields or executable actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProviderAnswer {
    /// Exact request identity echoed by the response.
    pub request_id: Digest,
    /// One of the request's closed choices.
    pub answer: String,
    /// Optional complete distribution; absent probabilities remain null.
    pub probabilities: Option<BTreeMap<String, f64>>,
    /// Bounded explanatory codes, not commands.
    pub reason_codes: Vec<String>,
    /// Cited facts or observations from this request.
    pub evidence_ids: Vec<String>,
    /// Declared evidence shortfalls.
    pub missing_evidence: Vec<String>,
    /// Verified against the cited evidence, never trusted blindly.
    pub depends_on_model_observation: bool,
    /// Short non-executable explanation.
    pub note: String,
}
/// Adapter-assigned local audit fields; model output cannot self-assign them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProviderAudit {
    /// Resolved answering provider/model.
    pub identity: ProviderIdentity,
    /// Exact dispatched payload bytes.
    pub payload_sha256: Digest,
    /// Exact returned response bytes.
    pub response_sha256: Digest,
    /// Reference to the local execution ledger, when available.
    pub execution_ref: Option<super::case::ArtifactRef>,
    /// Time is provenance, excluded from proposal semantic identity.
    pub timestamp_unix_ms: Option<u64>,
}
/// Advice with adapter audit identity. No promotion or approval fields exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionProposal {
    /// Content-derived answer/audit identity.
    pub proposal_id: Digest,
    /// Underlying case identity.
    pub case_id: Digest,
    /// Exact request answered.
    pub request_id: Digest,
    /// Validated untrusted answer.
    pub response: ProviderAnswer,
    /// Adapter-assigned identity and payload references.
    pub provider: ProviderAudit,
}
impl DecisionProposal {
    /// Combines a model answer and separately supplied adapter audit fields.
    pub fn from_response(
        request: &DecisionRequest,
        mut response: ProviderAnswer,
        provider: ProviderAudit,
    ) -> Result<Self> {
        response.depends_on_model_observation =
            observation_dependency(request, &response.evidence_ids)?;
        let mut proposal = Self {
            proposal_id: Digest::of_bytes(b""),
            case_id: request.case_id.clone(),
            request_id: request.request_id.clone(),
            response,
            provider,
        };
        proposal.proposal_id = proposal.identity()?;
        proposal.validate_for(request)?;
        Ok(proposal)
    }
    /// Semantic identity including actual answering model and exact payload/response hashes.
    pub fn identity(&self) -> Result<Digest> {
        canonical::digest(
            &json!({"case_id":self.case_id,"request_id":self.request_id,"response":self.response,
            "provider":{"identity":self.provider.identity,"payload_sha256":self.provider.payload_sha256,"response_sha256":self.provider.response_sha256,
                "execution_ref":self.provider.execution_ref.as_ref().map(|r| &r.sha256)}}),
        )
    }
    /// Checks intrinsic validity; use validate_for to validate the closed question.
    pub fn validate(&self) -> Result<()> {
        require(self.proposal_id == self.identity()?, "stale proposal_id")?;
        require(
            self.request_id == self.response.request_id && !self.response.answer.is_empty(),
            "proposal response has a different request",
        )?;
        self.provider.identity.validate()?;
        if let Some(reference) = &self.provider.execution_ref {
            reference.validate()?;
        }
        if let Some(probabilities) = &self.response.probabilities {
            require(
                !probabilities.is_empty()
                    && probabilities
                        .values()
                        .all(|p| p.is_finite() && (0.0..=1.0).contains(p)),
                "invalid probability value",
            )?;
            require(
                (probabilities.values().sum::<f64>() - 1.0).abs() <= 1e-6,
                "probabilities must sum to one",
            )?;
        }
        Ok(())
    }
    /// Validates answer membership, distributions, references and observation dependence.
    pub fn validate_for(&self, request: &DecisionRequest) -> Result<()> {
        self.validate()?;
        request.validate()?;
        require(
            self.case_id == request.case_id && self.request_id == request.request_id,
            "proposal refers to a different request or case",
        )?;
        require(
            request.question.answers.contains(&self.response.answer),
            "answer is outside the closed choice set",
        )?;
        if let Some(probabilities) = &self.response.probabilities {
            require(
                probabilities.keys().collect::<BTreeSet<_>>()
                    == request.question.answers.iter().collect(),
                "probability choices differ from request answers",
            )?;
        }
        require(
            self.response.note.chars().count() <= request.constraints.max_reason_chars,
            "answer explanation exceeds its bound",
        )?;
        require(
            !request.constraints.required_citations
                || self.response.answer == "abstain"
                || !self.response.evidence_ids.is_empty(),
            "answer needs evidence citations",
        )?;
        require(
            self.response.depends_on_model_observation
                == observation_dependency(request, &self.response.evidence_ids)?,
            "incorrect observation-dependency flag",
        )
    }
}
fn observation_dependency(request: &DecisionRequest, ids: &[String]) -> Result<bool> {
    let mut depends = false;
    for id in ids {
        if request.evidence.observations.iter().any(|f| &f.id == id) {
            depends = true;
        } else if let Some(fact) = request.evidence.facts.iter().find(|f| &f.id == id) {
            depends |= fact.depends_on_model_observation;
        } else {
            return Err(super::ContractError::Invalid(format!(
                "unknown evidence citation {id:?}"
            )));
        }
    }
    Ok(depends)
}
