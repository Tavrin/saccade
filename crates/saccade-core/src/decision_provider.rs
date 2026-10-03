//! Small provider-neutral decision interface. Transport authorization, shared
//! budgets and live adapters are coordinator responsibilities, not model fields.
use crate::evidence::canonical::{self, Digest};
use crate::evidence::proposal::{DecisionProposal, ProviderAnswer, ProviderAudit};
use crate::evidence::request::{DecisionRequest, ProviderIdentity};
use crate::evidence::{Result, require};
use serde::{Deserialize, Serialize};

/// Supported input modalities; encoded text is distinct from pixel inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Structured facts only.
    Structured,
    /// Structured facts and image observations.
    Vision,
}
/// Adapter retry classification; model text cannot request retries or execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    /// Budgeted retry may be possible after transient transport failure.
    Transient,
    /// Provider rate limit; coordinator applies its deterministic wait policy.
    RateLimited,
    /// Credentials or endpoint configuration; stop this provider.
    AuthenticationOrConfiguration,
    /// A malformed or unsupported response is unavailable, never an abstention.
    InvalidResponse,
}
/// Capabilities supplied by the adapter, not self-assigned by a model answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    /// Actual resolved model, not just a configured fallback alias.
    pub identity: ProviderIdentity,
    /// Accepted input modalities.
    pub modalities: Vec<Modality>,
    /// Whether the adapter supports the exact closed answer set.
    pub closed_choices: bool,
    /// Whether a complete probability distribution may be returned.
    pub probabilities: bool,
    /// Maximum requests per batch.
    pub batch_limit: usize,
    /// Whether usage is reported (absence remains explicit).
    pub usage_reporting: bool,
    /// Failure classes distinguished by the adapter.
    pub retry_classes: Vec<RetryClass>,
}
impl Capabilities {
    /// Checks that the provider can implement this interface.
    pub fn validate(&self) -> Result<()> {
        require(
            !self.identity.provider.trim().is_empty() && !self.identity.model.trim().is_empty(),
            "provider needs actual model identity",
        )?;
        require(
            self.closed_choices
                && self.batch_limit > 0
                && self.modalities.contains(&Modality::Structured),
            "provider lacks closed structured questions or a finite batch limit",
        )?;
        require(
            !self.retry_classes.is_empty(),
            "provider must classify failures",
        )
    }
}
/// Provider-reported usage without raw logs or response text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// Input tokens when reported.
    pub input_tokens: Option<u64>,
    /// Output tokens when reported.
    pub output_tokens: Option<u64>,
    /// Reported cost, if available.
    pub cost: Option<f64>,
    /// Currency for the reported cost.
    pub currency: Option<String>,
}
/// Transport or response failure, distinct from a committed answer or abstention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderFailure {
    /// Deterministic classification by the adapter.
    pub class: RetryClass,
    /// Bounded, credential-scrubbed local explanation.
    pub message: String,
    /// Provider retry deadline in seconds, when reported.
    pub retry_after_secs: Option<u64>,
}
/// Parsed model answer and separately assigned adapter audit data.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderResponse {
    /// Untrusted closed answer, with no authority or adapter fields.
    pub answer: ProviderAnswer,
    /// Trusted adapter's actual identity and exact byte hashes.
    pub audit: ProviderAudit,
    /// Absent usage is unknown, not zero.
    pub usage: Option<Usage>,
}
impl ProviderResponse {
    /// Strictly parses a recorded or live body while assigning audit fields outside
    /// the model-controlled JSON. No provider calls are made here.
    pub fn from_bytes(
        identity: ProviderIdentity,
        payload: &[u8],
        response: &[u8],
        usage: Option<Usage>,
    ) -> Result<Self> {
        Ok(Self {
            answer: canonical::decode(response)?,
            audit: ProviderAudit {
                identity,
                payload_sha256: Digest::of_bytes(payload),
                response_sha256: Digest::of_bytes(response),
                execution_ref: None,
                timestamp_unix_ms: None,
            },
            usage,
        })
    }
    /// Validates distributions, citations, catalog constraints and adapter identity.
    /// Observation dependence is derived by the canonical proposal constructor.
    pub fn into_proposal(
        self,
        request: &DecisionRequest,
        capabilities: &Capabilities,
    ) -> Result<DecisionProposal> {
        capabilities.validate()?;
        require(
            self.audit.identity == capabilities.identity,
            "answering model differs from resolved adapter identity",
        )?;
        require(
            capabilities.probabilities || self.answer.probabilities.is_none(),
            "adapter does not support probability distributions",
        )?;
        require(
            capabilities.usage_reporting || self.usage.is_none(),
            "adapter does not support usage reporting",
        )?;
        if let Some(usage) = &self.usage {
            require(
                usage.cost.is_none_or(|c| c.is_finite() && c >= 0.0)
                    && (usage.cost.is_none()
                        || usage.currency.as_ref().is_some_and(|c| !c.is_empty())),
                "invalid usage cost or currency",
            )?;
        }
        crate::questions::validate_answer(request, &self.answer.answer)?;
        require(
            self.answer.reason_codes.len() <= 16
                && self
                    .answer
                    .reason_codes
                    .iter()
                    .all(|c| c.chars().count() <= 80)
                && self.answer.missing_evidence.len() <= 32
                && self
                    .answer
                    .missing_evidence
                    .iter()
                    .all(|c| c.chars().count() <= 120),
            "unbounded response codes or missing evidence",
        )?;
        let proposal = DecisionProposal::from_response(request, self.answer, self.audit)?;
        require(
            proposal.response.answer == "abstain"
                || proposal.response.evidence_ids.iter().any(|id| {
                    request
                        .evidence
                        .observations
                        .iter()
                        .chain(&request.evidence.facts)
                        .any(|f| &f.id == id && f.value.value().is_some())
                }),
            "a committed answer must cite available evidence",
        )?;
        Ok(proposal)
    }
}

/// One shared interface for CLI, MCP, evaluation and future in-process clients.
/// The caller must authorize egress and reserve attempts before invoking it.
pub trait DecisionProvider {
    /// Resolved identity and capabilities known before preparing the request.
    fn capabilities(&self) -> Capabilities;
    /// Answers an already authorized question. Failure is not synthesized as abstain.
    fn answer(
        &self,
        request: &DecisionRequest,
    ) -> std::result::Result<ProviderResponse, ProviderFailure>;
}
