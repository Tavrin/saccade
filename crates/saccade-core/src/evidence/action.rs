//! Bounded actions, execution audits and the common future transport envelope.
use super::canonical::Digest;
use super::case::{ArtifactRef, ValidityStatus};
use super::{Result, require};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Closed deterministic next-action catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Inspect existing evidence.
    InspectEvidence,
    /// Collect missing measurements.
    CollectMissingEvidence,
    /// Fix a capture's validity.
    RepairCapture,
    /// Obtain attributed visual observations.
    RequestVision,
    /// Ask a person to resolve the case.
    RequestHuman,
    /// Retry at a specified time.
    RetryAt,
    /// Prepare an immutable reviewed update manifest.
    PrepareBaselineChange,
    /// No executable recommendation.
    None,
}
/// Authority which the caller must obtain before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// Explicit human disposition.
    HumanDecision,
    /// Startup/invocation network authorization.
    NetworkAuthorization,
    /// External capture producer execution.
    CaptureExecution,
}
/// Named MCP tools only; no arbitrary shell or baseline-write operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Tool {
    /// Measurement transport.
    #[serde(rename = "saccade_measure")]
    Measure,
    /// Local bounded inspection.
    #[serde(rename = "saccade_inspect")]
    Inspect,
    /// Prepare evidence.
    #[serde(rename = "saccade_evidence")]
    Evidence,
    /// Authorized review.
    #[serde(rename = "saccade_review")]
    Review,
    /// Proposed answers only.
    #[serde(rename = "saccade_propose")]
    Propose,
    /// Human review item.
    #[serde(rename = "saccade_ask_human")]
    AskHuman,
}
/// Typed local action arguments; no provider-generated arbitrary payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ActionArguments {
    /// Existing hashed artifact.
    pub artifact: Option<ArtifactRef>,
    /// Selected entry.
    pub entry: Option<String>,
    /// Explicit image request; false by default in summaries.
    pub include_images: bool,
    /// Exact question instance when relevant.
    pub request_id: Option<Digest>,
    /// Retry time for retry_at.
    pub retry_unix_ms: Option<u64>,
}
/// Deterministically generated bounded recommendation, never model executable text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NextAction {
    /// Stable recommendation ID.
    pub id: String,
    /// One permitted action kind.
    pub kind: ActionKind,
    /// Typed policy explanation.
    pub reason_code: String,
    /// Deterministic priority, lower first.
    pub priority: u32,
    /// Required external authority.
    pub requires: Vec<Requirement>,
    /// Known local tool or no executable tool.
    pub tool: Option<Tool>,
    /// Structured arguments.
    pub arguments: ActionArguments,
    /// Argument array; consumers never pass it to a shell.
    pub cli_argv: Vec<String>,
    /// Case precondition, checked at execution.
    pub expected_case_id: Digest,
}
impl NextAction {
    /// Checks closed action semantics; authorization/execution is a transport responsibility.
    pub fn validate(&self) -> Result<()> {
        require(
            !self.id.is_empty() && !self.reason_code.is_empty(),
            "actions need identity and reason",
        )?;
        if let Some(reference) = &self.arguments.artifact {
            reference.validate()?;
        }
        require(
            self.cli_argv.is_empty()
                || (self.cli_argv.first().is_some_and(|s| s == "saccade")
                    && self.cli_argv.len() > 1),
            "CLI action must be an argument array starting with saccade",
        )?;
        let required = match self.kind {
            ActionKind::RequestVision => Some(Requirement::NetworkAuthorization),
            ActionKind::PrepareBaselineChange => Some(Requirement::HumanDecision),
            ActionKind::RepairCapture => Some(Requirement::CaptureExecution),
            _ => None,
        };
        require(
            required.is_none_or(|r| self.requires.contains(&r)),
            "action omits required authority",
        )?;
        require(
            self.kind != ActionKind::RetryAt || self.arguments.retry_unix_ms.is_some(),
            "retry_at needs a retry time",
        )?;
        require(
            self.kind != ActionKind::None || (self.tool.is_none() && self.cli_argv.is_empty()),
            "none cannot be executable",
        )
    }
    /// Rejects recommendations whose case evidence or intent has changed.
    pub fn validate_for(&self, current_case: &Digest) -> Result<()> {
        self.validate()?;
        require(
            &self.expected_case_id == current_case,
            "stale expected_case_id",
        )
    }
}
/// Recorded result of payload policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PolicyResult {
    /// Allowed by the effective root/startup policy.
    Allowed,
    /// Explicitly denied.
    Denied,
    /// Incomplete provenance/authorization.
    Unknown,
}
/// Local execution audit reference; contains neither credentials nor session tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExecutionAudit {
    /// Exact case identity.
    pub case_id: Digest,
    /// Optional exact request.
    pub request_id: Option<Digest>,
    /// Actual or intended provider/model.
    pub provider: super::request::ProviderIdentity,
    /// Exact outbound payload hash.
    pub payload_sha256: Digest,
    /// All transitive root IDs checked for this payload.
    pub source_roots: Vec<String>,
    /// Denial/unknown provenance must block dispatch.
    pub policy_result: PolicyResult,
    /// Ledger containing attempts, retries and outcomes.
    pub ledger: ArtifactRef,
}
impl ExecutionAudit {
    /// Checks record shape, without authorizing dispatch.
    pub fn validate(&self) -> Result<()> {
        self.provider.validate()?;
        self.ledger.validate()
    }
}
/// Execution state separate from measurement, capture validity and review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Execution {
    /// Completed execution, possibly with a regression.
    Complete,
    /// An unresolved job/request.
    Pending,
    /// Tool/computation failed.
    Error,
}
/// Measurement finding, not approval authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MeasurementStatus {
    /// Selected native decoded samples matched.
    Identical,
    /// Selected samples differed.
    Different,
    /// Measurement acceptance criteria passed.
    Pass,
    /// Measurement criteria failed.
    Regression,
    /// Evidence unavailable.
    Unknown,
}
/// Human review status, independent of measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Awaiting review.
    Pending,
    /// Explicitly accepted disposition.
    Accepted,
    /// Explicitly rejected disposition.
    Rejected,
    /// Review is unresolved.
    Unresolved,
}
/// Pagination retains omitted counts instead of truncating JSON bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Number of omitted records.
    pub omitted: u64,
    /// Opaque continuation or null.
    pub next_cursor: Option<String>,
}
/// Bounded entry summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EntrySummary {
    /// Stable report entry ID.
    pub entry_id: String,
    /// Measurement status.
    pub measurement: MeasurementStatus,
    /// Error when a computation failed.
    pub error: Option<String>,
}
/// Structured tool error shared by CLI and MCP; regressions are measurement status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Stable targeted error code.
    pub code: String,
    /// Short explanation.
    pub message: String,
    /// Compiled feature needed for feature_unavailable.
    pub required_feature: Option<String>,
}
/// Future bounded CLI/MCP envelope. Existing v1 writers are migrated by R5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResultEnvelope {
    /// Always saccade-result.v2.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-result.v2")))]
    pub schema: String,
    /// Named operation.
    pub operation: String,
    /// Execution state.
    pub execution: Execution,
    /// Measurement finding.
    pub measurement: MeasurementStatus,
    /// Independent capture validity.
    pub validity: ValidityStatus,
    /// Validity reasons must survive bounded summaries.
    pub validity_reasons: Vec<String>,
    /// Human review state.
    pub review: ReviewStatus,
    /// Immutable full artifact, when produced.
    pub artifact: Option<ArtifactRef>,
    /// Retained total counts.
    pub counts: BTreeMap<String, u64>,
    /// Up to five default entry summaries.
    #[cfg_attr(feature = "schema", schemars(length(max = 5)))]
    pub entries: Vec<EntrySummary>,
    /// Up to three deterministic recommendations.
    #[cfg_attr(feature = "schema", schemars(length(max = 3)))]
    pub next_actions: Vec<NextAction>,
    /// Missing evidence and unsupported claims.
    pub limits: Vec<String>,
    /// Execution errors, distinct from measurement regressions.
    pub errors: Vec<Failure>,
    /// Omitted counts and continuation.
    pub page: Page,
}
impl ResultEnvelope {
    /// Validates default bounds, preserving complete JSON and unresolved evidence.
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == "saccade-result.v2" && !self.operation.is_empty(),
            "unsupported result envelope",
        )?;
        require(
            self.entries.len() <= 5 && self.next_actions.len() <= 3,
            "default result bounds exceeded",
        )?;
        if let Some(reference) = &self.artifact {
            reference.validate()?;
        }
        for action in &self.next_actions {
            action.validate()?;
        }
        require(
            serde_json::to_vec(self)?.len() <= 4096,
            "summary exceeds the 4 KiB serialized text budget",
        )
    }
}
