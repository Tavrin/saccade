//! Shared, offline evidence contracts for transports, providers and review.
//!
//! Producers own measurements in `saccade-report.v1`; these records reference
//! them. Deserialization checks shape; [`Document::validate`] checks semantic
//! identities and bindings before a caller uses an artifact.

pub mod action;
pub mod analysis;
pub mod canonical;
pub mod case;
pub mod human;
pub mod legacy;
pub mod proposal;
pub mod request;

use serde::{Deserialize, Serialize};

/// Version of the canonical evidence family.
pub const SCHEMA: &str = "saccade-evidence.v1";

/// Contract failures, independent of optional computing and network features.
#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    /// Malformed JSON or a field of the wrong type.
    #[error("evidence JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A broken identity, reference or semantic constraint.
    #[error("invalid evidence: {0}")]
    Invalid(String),
    /// Local evidence could not be read or written.
    #[error("evidence IO: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of validating or reading an evidence contract.
pub type Result<T> = std::result::Result<T, ContractError>;

pub(crate) fn require(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(ContractError::Invalid(reason.into()))
    }
}

/// Closed evidence document discriminator, common to every transport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Artifact {
    /// Underlying evidence and intent.
    Case(Box<case::EvidenceCase>),
    /// Exact question instance.
    DecisionRequest(Box<request::DecisionRequest>),
    /// Attributed advice; never an approval.
    DecisionProposal(Box<proposal::DecisionProposal>),
    /// Explicit human disposition with reviewed content bindings.
    HumanDecision(Box<human::HumanDecision>),
    /// Reserved read-only policy record; never human authority.
    AutomatedDecision(Box<human::AutomatedDecision>),
    /// Applied baseline-change audit.
    ApprovalReceipt(Box<human::ApprovalReceipt>),
    /// Private mapping, excluded from anonymous exports.
    PresentationMap(Box<human::PresentationMap>),
    /// Bounded action with a case precondition.
    NextAction {
        /// Bounded recommendation with its own action-kind discriminator.
        action: Box<action::NextAction>,
    },
    /// Local external-attempt audit reference.
    ExecutionAudit(Box<action::ExecutionAudit>),
}

/// Versioned on-disk document. Provider HTTP bodies are adapter details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Document {
    /// Always `saccade-evidence.v1`.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-evidence.v1")))]
    pub schema: String,
    /// One discriminated canonical record.
    #[serde(flatten)]
    pub artifact: Artifact,
}

impl Document {
    /// Wraps a record for persistence or transport.
    pub fn new(artifact: Artifact) -> Self {
        Self {
            schema: SCHEMA.into(),
            artifact,
        }
    }

    /// Validates identities and internal semantics. Cross-document bindings
    /// additionally require the corresponding `validate_for` operation.
    pub fn validate(&self) -> Result<()> {
        require(self.schema == SCHEMA, "unsupported evidence schema")?;
        match &self.artifact {
            Artifact::Case(c) => c.validate(),
            Artifact::DecisionRequest(r) => r.validate(),
            Artifact::DecisionProposal(p) => p.validate(),
            Artifact::HumanDecision(h) => h.validate(),
            Artifact::AutomatedDecision(a) => a.validate(),
            Artifact::ApprovalReceipt(r) => r.validate(),
            Artifact::PresentationMap(m) => m.validate(),
            Artifact::NextAction { action } => action.validate(),
            Artifact::ExecutionAudit(a) => a.validate(),
        }
    }

    /// Reports attested human, unattested CLI or reserved automated authority.
    /// A workbench decision without its receipt remains unattested.
    pub fn authority(&self) -> Option<human::Authority> {
        use human::{Authority, Channel};
        match &self.artifact {
            Artifact::AutomatedDecision(a) => Some(a.authority.clone()),
            Artifact::HumanDecision(d) if d.channel == Channel::Cli => Some(Authority::Cli),
            Artifact::ApprovalReceipt(r) if r.channel == Channel::Cli => Some(Authority::Cli),
            Artifact::ApprovalReceipt(r) if r.human_attestation.is_some() => Some(Authority::Human),
            _ => None,
        }
    }

    /// Checks the structural human-required boundary. A trusted workbench must
    /// additionally verify the receipt's live session attestation (R8).
    pub fn require_human_authority(&self) -> Result<()> {
        self.validate()?;
        require(
            self.authority()
                .is_some_and(|a| a.satisfies_human_required()),
            "human-required operation needs a workbench-attested receipt; cli and automated authority are insufficient",
        )
    }

    /// Reads a complete document and rejects malformed semantic identities.
    pub fn read(path: &std::path::Path) -> Result<Self> {
        let document: Self = canonical::decode(&std::fs::read(crate::paths::native(path))?)?;
        document.validate()?;
        Ok(document)
    }

    /// Writes a new portable bundle marker and its validated evidence document.
    /// Refuses replacement; output-root authorization belongs to the caller.
    pub fn write_bundle(&self, directory: &std::path::Path) -> Result<()> {
        use std::io::Write;
        self.validate()?;
        require(
            !matches!(self.artifact, Artifact::AutomatedDecision(_)),
            "automated authority is reserved; no writer is enabled",
        )?;
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::create_dir_all(crate::paths::native(directory))?;
        let marker = directory.join(".saccade-run");
        let mut marker_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(crate::paths::native(&marker))?;
        marker_file.write_all(b"saccade-evidence.v1\n")?;
        let path = directory.join("evidence.json");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(crate::paths::native(&path))?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        Ok(())
    }
}
