//! Closed advisory envelope, including local verification and provider provenance.
use super::geometry::Geometry;
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};

/// Versioned assist envelope.
pub const SCHEMA: &str = "saccade-assist.v1";
/// Catalog version, hashed into every request.
pub const CATALOG_VERSION: &str = "assist-catalog/1";
/// Advisory-only policy, independently qualified from historical human truth.
pub const POLICY_VERSION: &str = "constructed-assist/2";
/// Pinned starting Gemini model.
pub const GEMINI: &str = "gemini-3.8-flash";
/// Pinned starting Jev model.
pub const JEV: &str = "jev-1.13.0";
/// Operations share evidence and verification, never decision authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Task {
    /// Describe visible changes.
    Explain,
    /// Describe potentially concealed content per exclusion.
    AuditMask,
    /// Check a bounded visible condition.
    CheckUi,
}
/// Stable image role assigned locally after blind remapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Reference screenshot.
    Before,
    /// Candidate screenshot.
    After,
    /// Single-image check.
    Single,
}
/// Typed visual statements; no behavior, causal or approval category exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Visible text.
    Text,
    /// Visible presence or absence.
    Presence,
    /// Clipping within the supplied region.
    Clipping,
    /// Two known regions overlap.
    Overlap,
    /// Visual appearance.
    Appearance,
}
/// Visibility does not imply safety or complete capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Wholly visible.
    Visible,
    /// Partly visible.
    Partial,
    /// Hidden behind other content.
    Occluded,
    /// Original pixels/evidence unavailable.
    Unavailable,
}
/// Semantic answer; never a comparison verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Evidence provisionally supports the condition/change.
    Observed,
    /// Evidence provisionally does not support the condition/change.
    NotObserved,
    /// Missing, contradictory, unsupported or incomplete evidence.
    Unverifiable,
}
/// Mechanical validity and textual support remain different assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Jev considers the statement supported by supplied evidence.
    Supported,
    /// Jev considers it unsupported.
    Unsupported,
    /// Support not established.
    Insufficient,
}
/// Locally bound task and input identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// Discriminated operation.
    pub task: Task,
    /// Complete immutable catalog, condition and policy hash.
    pub request_hash: Digest,
    /// Exact report bytes when present.
    pub report_hash: Option<Digest>,
    /// Ordered screenshot hashes.
    pub screenshot_hashes: Vec<Digest>,
    /// Exact visible condition hash.
    pub condition_hash: Option<Digest>,
    /// Catalog schema/version.
    pub catalog_version: String,
    /// Qualification policy/version.
    pub policy_version: String,
}
/// Attributed observation, remapped to original-pixel space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Observation {
    /// Unique within the request.
    pub observation_id: String,
    /// Original image identity role.
    pub image_role: Role,
    /// Closed visual category.
    pub kind: Kind,
    /// Untrusted descriptive data, never instructions.
    pub statement: String,
    /// Original pixels, not provider-normalized values.
    pub geometry: Geometry,
    /// Visible scope.
    pub visibility: Visibility,
    /// Catalog references existing in the same image.
    pub evidence_refs: Vec<String>,
    /// Reported uncertainty, not a calibrated probability.
    pub uncertainty: f64,
}
/// Locally calculated verification; providers cannot assign these fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Verification {
    /// Closed wire decoding completed.
    pub schema_valid: bool,
    /// Hashes match frozen task and images.
    pub identity_valid: bool,
    /// Coordinates map inside the original images.
    pub geometry_valid: bool,
    /// References exist and intersect the observation geometry.
    pub citations_valid: bool,
    /// consistent, unresolved, or single_image.
    pub order_consistency: String,
    /// Textual support, not pixel verification.
    pub support: Support,
    /// False only for exact source-derived facts with no model dependency.
    pub depends_on_model_observation: bool,
}
/// Complete normalized usage; overlapping provider totals are retained, not summed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// Includes cached input when provider defines it so.
    pub input_tokens: Option<u64>,
    /// Candidate output, excluding separately reported thinking.
    pub candidate_tokens: Option<u64>,
    /// Thinking count when exposed.
    pub thinking_tokens: Option<u64>,
    /// Cached subset of input.
    pub cached_input_tokens: Option<u64>,
    /// Provider total, retained for cross-checking.
    pub total_tokens: Option<u64>,
    /// Modality usage as reported.
    pub modality_details: serde_json::Value,
}
/// Every dispatch or replay retains exact provider identity and costs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// gemini or jev.
    pub provider: String,
    /// Fixed selected model.
    pub requested_model: String,
    /// Actual returned model.
    pub returned_model: String,
    /// Required provider revision; drift is refused.
    pub returned_revision: String,
    /// Exact system instruction hash.
    pub prompt_hash: Digest,
    /// Payload encoding version.
    pub encoder_version: String,
    /// Explicit generation settings.
    pub sampling_settings: serde_json::Value,
    /// Exact payload bytes.
    pub request_hash: Digest,
    /// Exact response bytes.
    pub response_hash: Digest,
    /// ab, ba, single or support.
    pub order: String,
    /// Separate token counts and provider totals.
    pub usage: Usage,
    /// Actual cost if all required usage is available; unknown remains null.
    pub cost_usd: Option<f64>,
    /// Expiring rate schedule ID.
    pub cost_basis: String,
    /// miss, replay or bypass; replay is never an independent sample.
    pub cache_status: String,
    /// Original request start time.
    pub started_ms: u64,
    /// Original request finish time.
    pub finished_ms: u64,
    /// Wall time, retained independently from queue delay.
    pub elapsed_ms: u64,
}
/// Immutable result sidecar with unchanged deterministic verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// saccade-assist.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-assist.v1")))]
    pub schema: String,
    /// Task/input binding.
    pub identity: Identity,
    /// Exact catalog scope, distinct from observations.
    pub scope: super::catalog::Catalog,
    /// Semantic answer, always advisory.
    pub outcome: Outcome,
    /// Candidate visible observations.
    pub observations: Vec<Observation>,
    /// Local checks and dependent support.
    pub verification: Verification,
    /// Per-call provenance, including both blind orders and support.
    pub provenance: Vec<Provenance>,
    /// Exact original report verdict or null for a single image.
    pub deterministic_verdict: Option<String>,
    /// Contradictions, missing evidence, incomplete scope and provider failures.
    pub limitations: Vec<String>,
    /// True means execution ended with missing stages, distinct from semantic abstention.
    pub incomplete: bool,
}

/// Exact prepared request artifact; not authorization to dispatch.
pub const REQUESTS_SCHEMA: &str = "saccade-assist-requests.v1";
/// Replay records, never independent qualification samples.
pub const OBSERVATIONS_SCHEMA: &str = "saccade-assist-observations.v1";
/// Individual and aggregate mask accounting, without verdict authority.
pub const MASK_AUDIT_SCHEMA: &str = "saccade-assist-mask-audit.v1";
/// Separate constructed-only evaluation schema.
pub const CONSTRUCTED_SCHEMA: &str = "saccade-constructed-truth.v1";
/// Private renderer-verified oracle artifact, never sent to vision.
pub const ORACLE_SCHEMA: &str = "saccade-constructed-oracle.v1";
/// Exact-source receipt emitted only after every heavy gate passes.
pub const GATES_SCHEMA: &str = "saccade-assist-gates.v1";

/// Report-bound original individual mask membership input.
pub const MASKS_SCHEMA: &str = "saccade-assist-masks.v1";
