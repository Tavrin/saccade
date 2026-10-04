//! Evidence identity, explicit availability, and references to measurements.

use super::canonical::{self, Digest};
use super::{Result, require};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Missing and failed observations are distinct from an available zero or abstention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum Availability<T> {
    /// Evidence was supplied.
    Available {
        /// Supplied value.
        value: T,
    },
    /// No measurement or response was supplied.
    Missing {
        /// What is needed.
        reason: String,
    },
    /// An attempted computation or response failed.
    Unavailable {
        /// Failure without invented evidence.
        reason: String,
    },
}

impl<T> Availability<T> {
    /// Builds explicit missing evidence.
    pub fn missing(reason: &str) -> Self {
        Self::Missing {
            reason: reason.into(),
        }
    }
    /// Accesses only evidence that exists.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value } => Some(value),
            _ => None,
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Available { .. } => Ok(()),
            Self::Missing { reason } | Self::Unavailable { reason } => {
                require(!reason.trim().is_empty(), "missing evidence needs a reason")
            }
        }
    }
}

/// Exact local content reference. Paths are provenance, not semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    /// Relative to its containing document by default, using `/` separators.
    pub path: String,
    /// Hash of exact file bytes.
    pub sha256: Digest,
}
impl ArtifactRef {
    /// Captures a local file hash and its portable provenance path.
    pub fn from_file(path: &Path, document: &Path, absolute: bool) -> Result<Self> {
        let resolved = crate::paths::canonicalize(path)?;
        let base = document.parent().map_or(Path::new("."), |p| p);
        Ok(Self {
            path: crate::paths::record(&resolved, base, absolute),
            sha256: Digest::of_bytes(&std::fs::read(crate::paths::native(&resolved))?),
        })
    }
    /// Resolves provenance and verifies bytes before consumption.
    pub fn verify(&self, document: &Path) -> Result<()> {
        self.validate()?;
        let resolved = crate::paths::canonicalize(crate::paths::resolve(&self.path, document))?;
        require(
            Digest::of_bytes(&std::fs::read(crate::paths::native(&resolved))?) == self.sha256,
            "stale artifact content",
        )
    }
    pub(crate) fn validate(&self) -> Result<()> {
        require(
            !self.path.is_empty() && !self.path.contains('\\'),
            "artifact paths must be nonempty and use / separators",
        )
    }
}

/// Provenance excluded from semantic digests; source policy follows relocation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Creation or observation time, when known.
    pub timestamp_unix_ms: Option<u64>,
    /// Original portable path spellings.
    pub paths: Vec<String>,
    /// Transitive root identifiers, retained for later transport authorization.
    pub source_roots: Vec<String>,
    /// Human or producer attribution; never an authority grant.
    pub source: Option<String>,
}

/// Native decoded sample interpretation, separate from file-byte equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NativeSamples {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Ordered channel interpretation, including alpha when present.
    pub channels: Vec<String>,
    /// Native representation, for example u8, u16 or f32.
    pub sample_type: String,
    /// Color interpretation used by the comparison.
    pub color: String,
}

/// A selected input and relevant sidecar content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Stable input role/name in the named scope.
    pub id: String,
    /// Exact image or buffer content.
    pub content: ArtifactRef,
    /// Relevant sidecars, in declared order.
    pub sidecars: Vec<ArtifactRef>,
    /// Explicit native interpretation or missing decode evidence.
    pub native_samples: Availability<NativeSamples>,
    /// Capture identity and context, absent rather than guessed.
    pub capture: Availability<BTreeMap<String, Value>>,
    /// Binary/source/build identity.
    pub build: Availability<BTreeMap<String, Value>>,
    /// Time, locations and policy lineage.
    pub provenance: Provenance,
}

/// Named scope and ordered exclusions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Selected report entry IDs.
    pub entries: Vec<String>,
    /// Excluded entries, regions or masks; ordering is preserved.
    pub exclusions: Vec<String>,
}

/// Reference to the authoritative measurement, without copying its metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    /// Exact report bytes for verification and reviewer binding.
    pub report: ArtifactRef,
    /// Report semantics excluding the report's explicit time/location fields.
    pub semantic_sha256: Digest,
    /// Entry IDs selected from that report.
    pub entry_ids: Vec<String>,
    /// Measurement algorithm/version.
    pub metric_version: String,
    /// Effective-configuration format/version.
    pub config_version: String,
}
impl Measurement {
    /// Reads and hashes an authoritative pair report, retaining exact and semantic hashes.
    pub fn from_report(
        path: &Path,
        document: &Path,
        entry_ids: Vec<String>,
        metric_version: String,
        config_version: String,
    ) -> Result<Self> {
        let resolved = crate::paths::canonicalize(path)?;
        let raw = std::fs::read(crate::paths::native(&resolved))?;
        let report: crate::Report = canonical::decode(&raw)?;
        require(
            report.schema == crate::report::REPORT_SCHEMA,
            "expected saccade-report.v1",
        )?;
        require(
            !entry_ids.is_empty()
                && entry_ids
                    .iter()
                    .all(|id| report.entries.iter().any(|e| &e.name == id)),
            "measurement scope must name existing report entries",
        )?;
        Ok(Self {
            report: ArtifactRef {
                path: crate::paths::record(
                    &resolved,
                    document.parent().unwrap_or(Path::new(".")),
                    false,
                ),
                sha256: Digest::of_bytes(&raw),
            },
            semantic_sha256: Self::report_identity(&report)?,
            entry_ids,
            metric_version,
            config_version,
        })
    }
    /// Semantic projection of the measured report. Only explicit provenance is omitted.
    pub fn report_identity(report: &crate::Report) -> Result<Digest> {
        let mut value = serde_json::to_value(report)?;
        if let Some(object) = value.as_object_mut() {
            for key in ["generated_at_unix", "baseline_dir", "capture_dir"] {
                object.remove(key);
            }
            if let Some(entries) = object.get_mut("entries").and_then(Value::as_array_mut) {
                for entry in entries {
                    if let Some(object) = entry.as_object_mut() {
                        object.remove("paths");
                    }
                }
            }
        }
        canonical::digest(&value)
    }
}

/// Separate capture-comparability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ValidityStatus {
    /// Required capture checks passed.
    Valid,
    /// At least one capture check failed.
    Invalid,
    /// Required context is missing.
    Unknown,
}
/// Capture validity with retained reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Validity {
    /// Independent of native sample equality.
    pub status: ValidityStatus,
    /// Failed or missing requirements.
    pub reasons: Vec<String>,
}

/// Attribution category; declarations and models are never measured facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FactSource {
    /// Computed from files or validated producer data.
    Measured,
    /// Supplied by a project or reviewer.
    Declared,
    /// Extracted by an attributed vision model.
    ModelObservation,
    /// A decision model's interpretation.
    Inference,
}
/// Typed scalar observations or aggregate references, with no editable metric report copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FactValue {
    /// Numeric result; must be finite.
    Number(f64),
    /// Boolean observation.
    Boolean(bool),
    /// Closed category or descriptive observation.
    Text(String),
    /// Constituent facts with explicit additivity semantics.
    Aggregate(Aggregate),
}
/// Fact references retain producer aggregation semantics rather than summing scopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Aggregate {
    /// Ordered constituent fact IDs.
    pub members: Vec<String>,
    /// Producer-declared relationship; validation does not manufacture a frame total.
    pub semantics: Aggregation,
}
/// Closed aggregation relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    /// Ordered paired findings, without addition.
    Ordered,
    /// Producer validated additive terms.
    Additive,
    /// Values cannot be added, for example unrelated distribution medians.
    NonAdditive,
    /// Nested scopes overlap and cannot establish additive frame time.
    Nested,
}
/// Observation with explicit availability, scope and source content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Fact {
    /// Unique reference within the evidence selection.
    pub id: String,
    /// Named observation or measurement.
    pub name: String,
    /// Units, including explicit dimensionless/category units.
    pub units: String,
    /// Named entry, region, pass or scope.
    pub scope: Scope,
    /// Measured, declared, observed or inferred.
    pub source: FactSource,
    /// Underlying immutable artifact.
    pub artifact: ArtifactRef,
    /// Source semantics; measured report facts use its semantic digest. Model
    /// observations use the actual response content hash, not a model name alone.
    pub source_identity: Digest,
    /// Available zero is different from missing evidence.
    pub value: Availability<FactValue>,
    /// Model-observation dependency, verified against references.
    pub depends_on_model_observation: bool,
    /// IDs of observations used for this conclusion.
    pub observation_refs: Vec<String>,
}
impl Fact {
    pub(crate) fn semantic(&self) -> Value {
        json!({"id":self.id,"name":self.name,"units":self.units,"scope":self.scope,"source":self.source,
            "source_identity":self.source_identity,"value":self.value,"depends_on_model_observation":self.depends_on_model_observation,"observation_refs":self.observation_refs})
    }
    /// Rejects non-finite values and unmarked observation-dependent conclusions.
    pub fn validate(&self) -> Result<()> {
        require(
            !self.id.is_empty() && !self.name.is_empty() && !self.units.is_empty(),
            "facts need identity, name and units",
        )?;
        self.artifact.validate()?;
        self.value.validate()?;
        require(
            self.source != FactSource::ModelObservation
                || self.source_identity == self.artifact.sha256,
            "model observations must identify actual response content",
        )?;
        if let Some(FactValue::Number(value)) = self.value.value() {
            require(value.is_finite(), "fact numbers must be finite")?;
        }
        require(
            self.depends_on_model_observation == !self.observation_refs.is_empty(),
            "fact observation dependence must match its references",
        )
    }
}

/// A predeclared permitted intervention. Unchanged keys remain visible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DeclaredChange {
    /// Capture/configuration key.
    pub key: String,
    /// Why this difference is intended.
    pub reason: String,
    /// Optional expected values; absent values do not prove an intervention.
    pub expected_before: Option<Value>,
    /// Optional expected result.
    pub expected_after: Option<Value>,
}
/// A computable acceptance criterion rather than an aesthetic score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    /// Stable criterion ID.
    pub id: String,
    /// Referenced measurement/fact.
    pub fact_id: String,
    /// Closed numerical or equality comparator.
    pub comparator: Comparator,
    /// Expected value in named units.
    pub expected: FactValue,
    /// Units of the acceptance bound.
    pub units: String,
}
/// Supported computable comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    /// Exact equality.
    Equal,
    /// At most.
    LessOrEqual,
    /// At least.
    GreaterOrEqual,
}
/// Intent bound to the case, with explicit provenance assurance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Intent {
    /// Local reference used by requests.
    pub id: String,
    /// Claim or objective supplied before reviewing results.
    pub objective: String,
    /// Structured file or lower-assurance text provenance.
    pub assurance: IntentAssurance,
    /// Predeclared differences.
    pub expected_changes: Vec<DeclaredChange>,
    /// Properties required to remain unchanged.
    pub invariants: Vec<String>,
    /// Computable acceptance criteria.
    pub criteria: Vec<Criterion>,
    /// Optional immutable source file.
    pub source: Option<ArtifactRef>,
    /// Immutable mask bytes referenced by a structured visual intent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask_sources: Vec<ArtifactRef>,
    /// Attribution and creation time.
    pub provenance: Provenance,
}
/// Structured declarations and free text carry different assurance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IntentAssurance {
    /// Human/project supplied structured file.
    Structured,
    /// Lower-assurance command-line text.
    Text,
}

/// Evidence and intent; workflow state never changes the underlying case identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EvidenceCase {
    /// Content-derived semantic identity.
    pub case_id: Digest,
    /// Selected inputs and sidecars.
    pub inputs: Vec<Input>,
    /// Authoritative report reference.
    pub measurement: Measurement,
    /// Ordered scope and exclusions.
    pub scope: Scope,
    /// Effective settings; no path-based configuration identity.
    pub effective_config: BTreeMap<String, Value>,
    /// Calibration content identity or explicit missingness.
    pub calibration: Availability<Digest>,
    /// Independent validity with reasons.
    pub validity: Validity,
    /// Attributed observations.
    pub facts: Vec<Fact>,
    /// Declared objective and criteria, or explicit absent intent.
    pub intent: Availability<Intent>,
    /// Exact closed questions about this evidence.
    pub requests: Vec<super::request::DecisionRequest>,
    /// Proposed answers, separate from decisions.
    pub proposals: Vec<super::proposal::DecisionProposal>,
    /// Reviewed human dispositions, never inferred from proposals.
    pub human_decisions: Vec<super::human::HumanDecision>,
    /// Bounded deterministic recommendations.
    pub next_actions: Vec<super::action::NextAction>,
    /// Missing evidence and unsupported claims.
    pub limits: Vec<String>,
    /// Relocatable provenance retained for auditing and policy.
    pub provenance: Provenance,
}
impl EvidenceCase {
    /// Hashes evidence and intent, excluding workflow, time and path provenance.
    pub fn identity(&self) -> Result<Digest> {
        let inputs: Vec<_> = self.inputs.iter().map(|i| json!({"id":i.id,"content":i.content.sha256,
            "sidecars":i.sidecars.iter().map(|s| &s.sha256).collect::<Vec<_>>(),"native_samples":i.native_samples,"capture":i.capture,"build":i.build})).collect();
        let mut intent = serde_json::to_value(&self.intent)?;
        if let Some(value) = intent.get_mut("value").and_then(Value::as_object_mut) {
            value.remove("provenance");
            if let Some(source) = value.get_mut("source")
                && !source.is_null()
            {
                *source = source["sha256"].clone();
            }
            if let Some(masks) = value.get_mut("mask_sources").and_then(Value::as_array_mut) {
                for mask in masks {
                    *mask = mask["sha256"].clone();
                }
            }
        }
        let facts: Vec<_> = self
            .facts
            .iter()
            .filter(|f| matches!(f.source, FactSource::Measured | FactSource::Declared))
            .map(Fact::semantic)
            .collect();
        canonical::digest(
            &json!({"inputs":inputs,"measurement":{"semantic_sha256":self.measurement.semantic_sha256,
            "entry_ids":self.measurement.entry_ids,"metric_version":self.measurement.metric_version,"config_version":self.measurement.config_version},
            "scope":self.scope,"effective_config":self.effective_config,"calibration":self.calibration,"intent":intent,"validity":self.validity,"facts":facts,"limits":self.limits}),
        )
    }
    /// Recomputes the identity while constructing a case; dependent records must then be rebuilt.
    pub fn refresh_id(&mut self) -> Result<()> {
        self.case_id = self.identity()?;
        Ok(())
    }
    /// Validates the complete case and all attached records against exact identities.
    pub fn validate(&self) -> Result<()> {
        require(self.case_id == self.identity()?, "stale case_id")?;
        require(
            !self.inputs.is_empty() && !self.scope.entries.is_empty(),
            "case needs inputs and a nonempty scope",
        )?;
        unique(self.inputs.iter().map(|i| i.id.as_str()))?;
        unique(self.scope.entries.iter().map(String::as_str))?;
        require(
            self.scope.entries == self.measurement.entry_ids,
            "measurement scope differs from case scope",
        )?;
        self.measurement.report.validate()?;
        require(
            !self.measurement.metric_version.is_empty()
                && !self.measurement.config_version.is_empty(),
            "measurement versions are required",
        )?;
        self.calibration.validate()?;
        self.intent.validate()?;
        for input in &self.inputs {
            input.content.validate()?;
            for sidecar in &input.sidecars {
                sidecar.validate()?;
            }
            input.native_samples.validate()?;
            input.capture.validate()?;
            input.build.validate()?;
            if let Some(samples) = input.native_samples.value() {
                require(
                    samples.width > 0
                        && samples.height > 0
                        && !samples.channels.is_empty()
                        && !samples.sample_type.is_empty()
                        && !samples.color.is_empty(),
                    "invalid native sample description",
                )?;
            }
        }
        if let Some(intent) = self.intent.value() {
            require(
                !intent.id.is_empty() && !intent.objective.trim().is_empty(),
                "intent requires identity and objective",
            )?;
            if let Some(source) = &intent.source {
                source.validate()?;
            }
            for mask in &intent.mask_sources {
                mask.validate()?;
            }
            for change in &intent.expected_changes {
                require(
                    !change.key.is_empty() && !change.reason.trim().is_empty(),
                    "declarations need a key and reason",
                )?;
            }
            unique(intent.criteria.iter().map(|c| c.id.as_str()))?;
            for criterion in &intent.criteria {
                require(
                    !criterion.units.is_empty() && !criterion.fact_id.is_empty(),
                    "criteria need fact references and units",
                )?;
                if let FactValue::Number(n) = criterion.expected {
                    require(n.is_finite(), "criterion numbers must be finite")?;
                }
            }
        }
        unique(self.facts.iter().map(|f| f.id.as_str()))?;
        for fact in &self.facts {
            fact.validate()?;
            if fact.source == FactSource::Measured
                && fact.artifact.sha256 == self.measurement.report.sha256
            {
                require(
                    fact.source_identity == self.measurement.semantic_sha256,
                    "report fact has a different semantic source",
                )?;
            }
            for id in &fact.observation_refs {
                require(
                    self.facts
                        .iter()
                        .any(|o| o.id == *id && o.source == FactSource::ModelObservation),
                    "unknown fact observation reference",
                )?;
            }
        }
        unique(self.requests.iter().map(|r| r.request_id.as_str()))?;
        for request in &self.requests {
            request.validate_for(self)?;
        }
        for proposal in &self.proposals {
            let request = self
                .requests
                .iter()
                .find(|r| r.request_id == proposal.request_id)
                .ok_or_else(|| {
                    super::ContractError::Invalid("proposal has no matching request".into())
                })?;
            proposal.validate_for(request)?;
        }
        for decision in &self.human_decisions {
            decision.validate_for(self)?;
        }
        for action in &self.next_actions {
            action.validate_for(&self.case_id)?;
        }
        Ok(())
    }
}

pub(crate) fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        require(
            !id.is_empty() && seen.insert(id),
            "empty or duplicate evidence ID",
        )?;
    }
    Ok(())
}
