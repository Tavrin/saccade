//! Closed provider-neutral questions and observation-dependent cache identity.
use super::canonical::{self, Digest};
use super::case::{EvidenceCase, Fact, FactSource, unique};
use super::{Result, require};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Actual resolved provider/model identity, supplied by the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentity {
    /// User-approved provider identifier, not an endpoint.
    pub provider: String,
    /// Actual model used.
    pub model: String,
    /// Resolved revision/version when the provider exposes one.
    pub revision: Option<String>,
}
impl ProviderIdentity {
    pub(crate) fn validate(&self) -> Result<()> {
        require(
            !self.provider.is_empty() && !self.model.is_empty(),
            "actual provider and model identities are required",
        )
    }
}
/// Completed extractor identity; enriched requests cannot use unresolved placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ObservationContext {
    /// Actual feature extractor, including a fallback model when used.
    pub extractor: ProviderIdentity,
    /// Observation rubric/version.
    pub rubric_version: String,
    /// Hash of presentation transforms and crop selection.
    pub transform_identity: Digest,
    /// Hash of configured fallback chain and policy, not only the successful model.
    pub fallback_chain_identity: Digest,
    /// Hash of actual fallback attempts/outcomes.
    pub fallback_outcome_identity: Digest,
}
/// Question instance; the catalog and scoring rules belong to R9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Versioned question ID, for example intent.match.v1.
    pub id: String,
    /// Fixed rubric text used for this instance.
    pub text: String,
    /// Closed, ordered answer set, including abstain.
    pub answers: Vec<String>,
}
/// Selected evidence, without a second editable pair report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RequestEvidence {
    /// Encoder format/version.
    pub encoder_version: String,
    /// Selected measured or declared facts and attributed inferences.
    pub facts: Vec<Fact>,
    /// Actual model observations, distinct from deterministic facts.
    pub observations: Vec<Fact>,
    /// Intent ID when intent exists; absent intent is recorded in missing.
    pub intent_ref: Option<String>,
    /// Explicit evidence shortfalls; no response is never synthesized as abstention.
    pub missing: Vec<String>,
    /// Required whenever model observations are present.
    pub observation_context: Option<ObservationContext>,
    /// Transform identity even for requests without model observations.
    pub presentation_identity: Digest,
}
/// Providers supply advice only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// A model answer is a proposal.
    Proposal,
}
/// Boundaries validated before accepting an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    /// Always proposal.
    pub role: Role,
    /// Require fact citations for non-abstaining answers.
    pub required_citations: bool,
    /// Maximum explanatory Unicode characters.
    pub max_reason_chars: usize,
}
/// Shared CLI/MCP/provider request body; HTTP wrappers remain outside this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    /// Question/evidence/encoding/policy identity.
    pub request_id: Digest,
    /// Underlying evidence and intent identity.
    pub case_id: Digest,
    /// Closed versioned question.
    pub question: Question,
    /// Precisely selected evidence.
    pub evidence: RequestEvidence,
    /// Answer validation constraints.
    pub constraints: Constraints,
    /// Effective routing/egress/decision policy identity inputs, not credentials.
    pub policy: BTreeMap<String, Value>,
}
impl DecisionRequest {
    /// Request identity excludes artifact path spellings while binding all selected facts.
    pub fn identity(&self) -> Result<Digest> {
        canonical::digest(&json!({"case_id":self.case_id,"question":self.question,
            "evidence":{"encoder_version":self.evidence.encoder_version,
                "facts":self.evidence.facts.iter().map(Fact::semantic).collect::<Vec<_>>(),
                "observations":self.evidence.observations.iter().map(Fact::semantic).collect::<Vec<_>>(),
                "intent_ref":self.evidence.intent_ref,"missing":self.evidence.missing,
                "observation_context":self.evidence.observation_context,"presentation_identity":self.evidence.presentation_identity},
            "constraints":self.constraints,"policy":self.policy}))
    }
    /// Identity of the compatibility domain for calibrators, including actual observations.
    /// Training splits and fitted parameters are separately bound by evaluation producers.
    pub fn calibration_identity(&self) -> Result<Digest> {
        canonical::digest(
            &json!({"question":self.question,"encoder_version":self.evidence.encoder_version,
            "observation_context":self.evidence.observation_context,"presentation_identity":self.evidence.presentation_identity,"policy":self.policy}),
        )
    }
    /// Rebuilds a request identity after the actual observation outcome is known.
    pub fn refresh_id(&mut self) -> Result<()> {
        self.request_id = self.identity()?;
        Ok(())
    }
    /// Validates a standalone request without requiring a computational producer.
    pub fn validate(&self) -> Result<()> {
        require(self.request_id == self.identity()?, "stale request_id")?;
        require(
            !self.question.id.is_empty() && !self.question.text.trim().is_empty(),
            "question identity and text are required",
        )?;
        unique(self.question.answers.iter().map(String::as_str))?;
        require(
            self.question.answers.len() >= 2
                && self.question.answers.iter().any(|a| a == "abstain"),
            "closed questions must include abstain and another answer",
        )?;
        require(
            !self.evidence.encoder_version.is_empty() && self.constraints.max_reason_chars > 0,
            "encoder version and answer bound are required",
        )?;
        unique(
            self.evidence
                .facts
                .iter()
                .chain(&self.evidence.observations)
                .map(|f| f.id.as_str()),
        )?;
        for fact in self
            .evidence
            .facts
            .iter()
            .chain(&self.evidence.observations)
        {
            fact.validate()?;
        }
        require(
            self.evidence
                .facts
                .iter()
                .all(|f| f.source != FactSource::ModelObservation),
            "model observations belong in observations",
        )?;
        require(
            self.evidence
                .observations
                .iter()
                .all(|f| f.source == FactSource::ModelObservation),
            "observations need model attribution",
        )?;
        for fact in &self.evidence.facts {
            for id in &fact.observation_refs {
                require(
                    self.evidence.observations.iter().any(|o| &o.id == id),
                    "unknown request observation reference",
                )?;
            }
        }
        if let Some(context) = &self.evidence.observation_context {
            context.extractor.validate()?;
            require(
                !context.rubric_version.is_empty(),
                "observation rubric version is required",
            )?;
        }
        require(
            self.evidence.observations.is_empty() || self.evidence.observation_context.is_some(),
            "enriched requests require actual observation identity",
        )
    }
    /// Verifies selected facts, scope and intent against the underlying case.
    pub fn validate_for(&self, case: &EvidenceCase) -> Result<()> {
        self.validate()?;
        require(
            self.case_id == case.case_id && case.case_id == case.identity()?,
            "request refers to a stale or different case",
        )?;
        require(
            self.evidence.intent_ref.as_deref() == case.intent.value().map(|i| i.id.as_str()),
            "request intent reference differs from the case",
        )?;
        for fact in &self.evidence.facts {
            require(
                case.facts.iter().any(|f| f.semantic() == fact.semantic()),
                "request fact differs from the case",
            )?;
        }
        Ok(())
    }
}
