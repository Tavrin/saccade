//! Human review bindings and receipts. Structural validation does not attest a
//! human session; the workbench must verify its scoped token before issuance.
use super::canonical::{self, Digest};
use super::case::{ArtifactRef, Availability, EvidenceCase, Scope};
use super::{Result, require};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

/// Application approval channel; providers and MCP cannot issue receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Explicit CLI operation; no human attestation.
    Cli,
    /// Token-gated local workbench session.
    Workbench,
}
/// Explicit scoped human disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// Accept reviewed change.
    Accept,
    /// Reject reviewed change.
    Reject,
    /// Unresolved; requires further work.
    NeedsWork,
    /// No preference; never acceptance.
    Tie,
}
/// What the reviewer could know; independent blinding cannot be inferred from a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReviewerExposure {
    /// Whether the reviewer had access to the mapping.
    pub mapping_access: Availability<bool>,
    /// Whether the reviewer knew implementation/input context.
    pub implementation_context: Availability<bool>,
    /// Declared reviewer/session identity; not authentication.
    pub reviewer: Option<String>,
}
/// Exact inputs, scope and displayed content checked during human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReviewBinding {
    /// Underlying case identity.
    pub case_id: Digest,
    /// Content hashes by stable input ID.
    pub input_hashes: BTreeMap<String, Digest>,
    /// Exactly selected entries/exclusions.
    pub scope: Scope,
    /// Exact questions included in the reviewed presentation, when applicable.
    pub request_ids: Vec<Digest>,
    /// Exact artifact shown to the reviewer, including transforms/observations.
    pub reviewed_content: ArtifactRef,
}
impl ReviewBinding {
    /// Builds a binding from an already validated case and exact displayed artifact.
    pub fn for_case(case: &EvidenceCase, reviewed_content: ArtifactRef) -> Result<Self> {
        require(case.case_id == case.identity()?, "cannot bind a stale case")?;
        Ok(Self {
            case_id: case.case_id.clone(),
            input_hashes: case
                .inputs
                .iter()
                .map(|i| (i.id.clone(), i.content.sha256.clone()))
                .collect(),
            scope: case.scope.clone(),
            request_ids: case.requests.iter().map(|r| r.request_id.clone()).collect(),
            reviewed_content,
        })
    }
    /// Checks case/input/scope identity; displayed bytes must also be verified by the consumer.
    pub fn validate_for(&self, case: &EvidenceCase) -> Result<()> {
        self.reviewed_content.validate()?;
        require(
            self.case_id == case.case_id && case.case_id == case.identity()?,
            "stale review case",
        )?;
        let inputs = case
            .inputs
            .iter()
            .map(|i| (i.id.clone(), i.content.sha256.clone()))
            .collect();
        require(
            self.input_hashes == inputs && self.scope == case.scope,
            "review input hashes or scope differ",
        )?;
        require(
            self.request_ids
                .iter()
                .all(|id| case.requests.iter().any(|r| &r.request_id == id)),
            "review refers to an unknown question instance",
        )
    }
}
/// Human disposition. Source text alone never establishes attestation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HumanDecision {
    /// Identity of reviewed content, disposition and exposure.
    pub decision_id: Digest,
    /// Exact content reviewed.
    pub binding: ReviewBinding,
    /// Accept/reject/unresolved/tie.
    pub disposition: Disposition,
    /// Explicit deletion approval; separate from accepting additions/changes.
    pub approve_deletions: bool,
    /// Application channel of the recorded decision.
    pub channel: Channel,
    /// Reviewer exposure, retained even when unknown.
    pub exposure: ReviewerExposure,
    /// Human supplied explanation.
    pub note: String,
    /// Audit time, excluded from semantic decision identity.
    pub timestamp_unix_ms: Option<u64>,
}
impl HumanDecision {
    /// Hashes exactly reviewed semantic content and disposition.
    pub fn identity(&self) -> Result<Digest> {
        canonical::digest(
            &json!({"binding":{"case_id":self.binding.case_id,"input_hashes":self.binding.input_hashes,"scope":self.binding.scope,"request_ids":self.binding.request_ids,"reviewed_content":self.binding.reviewed_content.sha256},
            "disposition":self.disposition,"approve_deletions":self.approve_deletions,"channel":self.channel,"exposure":self.exposure,"note":self.note}),
        )
    }
    /// Recomputes identity when constructing a human record.
    pub fn refresh_id(&mut self) -> Result<()> {
        self.decision_id = self.identity()?;
        Ok(())
    }
    /// Checks intrinsic bindings without claiming a token was verified.
    pub fn validate(&self) -> Result<()> {
        require(self.decision_id == self.identity()?, "stale human decision")?;
        self.binding.reviewed_content.validate()?;
        require(
            !self.binding.input_hashes.is_empty() && !self.binding.scope.entries.is_empty(),
            "human decision needs input hashes and scope",
        )?;
        self.exposure.mapping_access.validate()?;
        self.exposure.implementation_context.validate()
    }
    /// Validates the current case/input scope as well as intrinsic identity.
    pub fn validate_for(&self, case: &EvidenceCase) -> Result<()> {
        self.validate()?;
        self.binding.validate_for(case)
    }
}
/// Audit evidence from a verified scoped workbench session; never the session token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HumanAttestation {
    /// Decision verified by the workbench.
    pub decision_id: Digest,
    /// Exact case/hashes/scope/displayed content verified.
    pub binding: ReviewBinding,
    /// Local attestation audit reference; secrets/tokens are excluded.
    pub audit_ref: ArtifactRef,
    /// Issuance time checked by the issuer.
    pub timestamp_unix_ms: u64,
}
/// One applied update, preserving before/after missingness for additions/deletions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppliedEntry {
    /// Selected report entry.
    pub entry_id: String,
    /// Baseline before applying, null for an addition.
    pub before: Option<Digest>,
    /// Baseline after applying, null for an explicitly approved deletion.
    pub after: Option<Digest>,
}
/// Receipt of an application operation, never a model proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceipt {
    /// Exact decision consumed.
    pub decision_id: Digest,
    /// Exact reviewed case and content.
    pub binding: ReviewBinding,
    /// CLI or workbench, with different attestation semantics.
    pub channel: Channel,
    /// Always null for CLI; workbench issuers verify the session before recording it.
    pub human_attestation: Option<HumanAttestation>,
    /// Only selected entries actually applied.
    pub applied: Vec<AppliedEntry>,
    /// Application time.
    pub timestamp_unix_ms: u64,
}
impl ApprovalReceipt {
    /// Validates structure; callers must verify attestation against trusted local state.
    pub fn validate(&self) -> Result<()> {
        self.binding.reviewed_content.validate()?;
        require(!self.applied.is_empty(), "receipt needs applied entries")?;
        super::case::unique(self.applied.iter().map(|a| a.entry_id.as_str()))?;
        for entry in &self.applied {
            require(
                self.binding.scope.entries.contains(&entry.entry_id)
                    && (entry.before.is_some() || entry.after.is_some()),
                "applied entry is outside scope or has no content",
            )?;
        }
        match (&self.channel, &self.human_attestation) {
            (Channel::Cli, None) => Ok(()),
            (Channel::Workbench, Some(attestation)) => {
                attestation.audit_ref.validate()?;
                require(
                    attestation.decision_id == self.decision_id
                        && attestation.binding == self.binding,
                    "attestation differs from receipt binding",
                )
            }
            _ => Err(super::ContractError::Invalid(
                "CLI receipts are unattested; workbench receipts require attestation".into(),
            )),
        }
    }
    /// Checks the selected updates against an explicit accepting decision and current case.
    pub fn validate_for(&self, decision: &HumanDecision, case: &EvidenceCase) -> Result<()> {
        self.validate()?;
        decision.validate_for(case)?;
        require(
            self.decision_id == decision.decision_id
                && self.binding == decision.binding
                && self.channel == decision.channel
                && decision.disposition == Disposition::Accept,
            "receipt requires the exact accepting human decision",
        )?;
        require(
            decision.approve_deletions || self.applied.iter().all(|e| e.after.is_some()),
            "deletions were not explicitly approved",
        )
    }
}
/// Private presentation mapping, never embedded in anonymous evidence exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PresentationMap {
    /// Case shown anonymously.
    pub case_id: Digest,
    /// Digest of exact anonymous presentation/transforms.
    pub presentation_identity: Digest,
    /// Anonymous slot to stable input ID, per entry.
    pub entries: BTreeMap<String, BTreeMap<String, String>>,
    /// Exact private unblinding audit record, when unblinded.
    pub unblinding_ref: Option<ArtifactRef>,
}
impl PresentationMap {
    /// Checks slot maps without exposing them through an anonymous export.
    pub fn validate(&self) -> Result<()> {
        require(!self.entries.is_empty(), "presentation map needs entries")?;
        for (entry, slots) in &self.entries {
            require(
                !entry.is_empty() && slots.len() >= 2,
                "presentation mapping needs an entry and at least two slots",
            )?;
            super::case::unique(slots.keys().map(String::as_str))?;
            super::case::unique(slots.values().map(String::as_str))?;
        }
        if let Some(reference) = &self.unblinding_ref {
            reference.validate()?;
        }
        Ok(())
    }
}
/// Evidence-bound human label for a particular question instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Label {
    /// Underlying case identity.
    pub case_id: Digest,
    /// Exact question/encoder/observation identity.
    pub request_id: Digest,
    /// Versioned question ID.
    pub question_id: String,
    /// Closed human answer.
    pub answer: String,
    /// Human disposition/label audit reference.
    pub human_decision_id: Digest,
    /// Reviewer exposure for interpretation of blind labels.
    pub exposure: ReviewerExposure,
}
impl Label {
    /// Validates a closed answer against its exact question and reviewed case.
    pub fn validate_for(
        &self,
        request: &super::request::DecisionRequest,
        decision: &HumanDecision,
    ) -> Result<()> {
        request.validate()?;
        decision.validate()?;
        require(
            self.case_id == request.case_id
                && self.case_id == decision.binding.case_id
                && self.request_id == request.request_id
                && decision.binding.request_ids.contains(&self.request_id)
                && self.question_id == request.question.id
                && self.human_decision_id == decision.decision_id
                && self.exposure == decision.exposure
                && request.question.answers.contains(&self.answer),
            "label differs from its reviewed question or decision",
        )
    }
}
/// Versioned human label collection, readable without evaluation/AI features.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Labels {
    /// Always saccade-labels.v2.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-labels.v2")))]
    pub schema: String,
    /// Human answers referencing exact cases and questions.
    pub items: Vec<Label>,
}

/// Authority reported by canonical readers. Automated authority is reserved for
/// P5; neither a policy identifier nor evidence equality is human attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "level", rename_all = "snake_case", deny_unknown_fields)]
pub enum Authority {
    /// Workbench-attested human authority, subject to trusted session verification.
    Human,
    /// Unattested explicit CLI operation.
    Cli,
    /// Reserved policy authority, distinct from a human disposition.
    Automated {
        /// Policy which authorized the record in a future producer.
        policy_id: String,
        /// Exact evidence case evaluated by the policy.
        evidence_digest: Digest,
    },
}
impl Authority {
    /// Display label; reserved automation never displays as human.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Cli => "cli",
            Self::Automated { .. } => "automated",
        }
    }
    /// Checks the authority level only. Human session attestation must also be
    /// verified by its trusted issuer; source labels cannot construct authority.
    pub fn satisfies_human_required(&self) -> bool {
        matches!(self, Self::Human)
    }
}
impl std::fmt::Display for Authority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Read-only reserved P5 record. It is never converted to HumanDecision or an
/// approval receipt and no supported writer may emit it in R1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AutomatedDecision {
    /// Identity of policy, exact binding and disposition.
    pub decision_id: Digest,
    /// Exactly reviewed evidence and scope.
    pub binding: ReviewBinding,
    /// Must be automated, with a policy ID and matching case evidence digest.
    pub authority: Authority,
    /// Reserved policy disposition, never human acceptance.
    pub disposition: Disposition,
}
impl AutomatedDecision {
    /// Computes the immutable reserved record identity; not an issuance API.
    pub fn identity(&self) -> Result<Digest> {
        canonical::digest(&json!({"binding":{"case_id":self.binding.case_id,
            "input_hashes":self.binding.input_hashes,"scope":self.binding.scope,
            "request_ids":self.binding.request_ids,"reviewed_content":self.binding.reviewed_content.sha256},
            "authority":self.authority,"disposition":self.disposition}))
    }
    /// Validates a read record without giving it human or CLI authority.
    pub fn validate(&self) -> Result<()> {
        let Authority::Automated {
            policy_id,
            evidence_digest,
        } = &self.authority
        else {
            return Err(super::ContractError::Invalid(
                "automated record needs automated authority".into(),
            ));
        };
        require(
            !policy_id.trim().is_empty() && evidence_digest == &self.binding.case_id,
            "automated authority needs a policy ID and the exact evidence digest",
        )?;
        self.binding.reviewed_content.validate()?;
        require(
            !self.binding.input_hashes.is_empty() && !self.binding.scope.entries.is_empty(),
            "automated record needs bound inputs and scope",
        )?;
        require(
            self.decision_id == self.identity()?,
            "stale automated decision",
        )
    }
}
