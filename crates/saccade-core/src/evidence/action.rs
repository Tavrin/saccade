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
    /// Discriminator required by the local tool's input contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Existing hashed artifact.
    pub artifact: Option<ArtifactRef>,
    /// Selected entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Explicit image request; false by default in summaries.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_images: bool,
    /// Exact question instance when relevant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Digest>,
    /// Retry time for retry_at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Working directory in which relative CLI arguments resolve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
    /// Entry status in the measured report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Metric that decides the status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    /// Deciding value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Deciding threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<serde_json::Value>,
    /// Bounded hotspots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotspots: Option<serde_json::Value>,
    /// One-line measured explanation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// Index of a validity reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
    /// Validity reason when listing them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
/// Preserved measurement integration fields, part of the v2 contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MeasurementIntegration {
    /// Measurement operation, such as compare, identity, or noise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Gate verdict, independent of review authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// Exact pass/fail/error/missing/new/total counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totals: Option<BTreeMap<String, u64>>,
    /// Bounded failing entries; full errors remain in the report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "schema", schemars(length(max = 5)))]
    pub failing: Vec<serde_json::Value>,
    /// Portable report and rendered artifact paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<BTreeMap<String, Option<String>>>,
    /// Separate native decoded-sample equality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_equality: Option<serde_json::Value>,
    /// Preserved independent validity record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_validity: Option<serde_json::Value>,
    /// Missing provenance fields and the sidecar/flag that supplies them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity_missing: Option<serde_json::Value>,
    /// Undeclared metadata differences and the flags that resolve them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity_guidance: Option<serde_json::Value>,
    /// Named selected scope and exclusions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<serde_json::Value>,
    /// Worst measured failure relative to its threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst: Option<serde_json::Value>,
    /// Run-wide performance result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<serde_json::Value>,
    /// Combined image and performance gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall: Option<String>,
    /// Command for fuller performance analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_action: Option<String>,
}
/// Future bounded CLI/MCP envelope. Existing v1 writers are migrated by R5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResultEnvelope {
    /// Content address shared with the persisted report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
    /// External capture-index references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    /// Historical saccade-result.v2 or the linked saccade-result.v4 successor.
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
    #[cfg_attr(feature = "schema", schemars(length(max = 10)))]
    pub entries: Vec<EntrySummary>,
    /// Up to three deterministic recommendations.
    #[cfg_attr(feature = "schema", schemars(length(max = 3)))]
    pub next_actions: Vec<NextAction>,
    /// Missing evidence and unsupported claims.
    pub limits: Vec<String>,
    /// Execution errors, distinct from measurement regressions.
    pub errors: Vec<Failure>,
    /// Preserved named measurement integration contracts.
    #[serde(default, flatten)]
    pub integration: MeasurementIntegration,
    /// Operation-specific local capability/config data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Omitted counts and continuation.
    pub page: Page,
}
impl ResultEnvelope {
    /// Validates default bounds, preserving complete JSON and unresolved evidence.
    pub fn validate(&self) -> Result<()> {
        require(
            crate::report_links::original_schema(&self.schema) == "saccade-result.v2"
                && !self.operation.is_empty(),
            "unsupported result envelope",
        )?;
        require(
            self.entries.len() <= if self.operation == "inspect" { 10 } else { 5 }
                && self.next_actions.len() <= 3
                && self.integration.failing.len() <= 5,
            "default result bounds exceeded",
        )?;
        if let Some(reference) = &self.artifact {
            reference.validate()?;
        }
        for action in &self.next_actions {
            action.validate()?;
        }
        require(
            serde_json::to_vec(self)?.len()
                <= if self.operation == "inspect" {
                    8192
                } else {
                    4096
                },
            "summary exceeds the 4 KiB serialized text budget",
        )
    }
}
