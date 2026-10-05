//! Additive evidence contracts for deterministic analysis extensions.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Availability is separate from a measurement or verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Capability {
    /// The requested computation was performed within its declared scope.
    Available,
    /// Evidence was not supplied or cannot establish applicability.
    Unknown {
        /// Machine-readable explanation.
        reason: String,
    },
    /// This implementation cannot perform the requested computation.
    Unsupported {
        /// Machine-readable explanation.
        reason: String,
    },
    /// The caller explicitly excluded the computation.
    Excluded {
        /// Machine-readable explanation.
        reason: String,
    },
    /// Inputs or execution failed validation.
    Rejected {
        /// Machine-readable explanation.
        reason: String,
    },
}

impl Default for Capability {
    fn default() -> Self {
        Self::Unknown {
            reason: "not_recorded".into(),
        }
    }
}

/// Content-bound resource; absent hashes or licences remain explicit unknowns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Resource {
    /// Logical resource name, not a machine-specific path.
    pub name: String,
    /// Role such as input, implementation, weights or configuration.
    pub role: String,
    /// Lowercase SHA-256 of exact resource bytes, when available.
    pub sha256: Option<String>,
    /// SPDX expression or a referenced custom licence; never inferred from code.
    pub license: Option<String>,
}

/// Reproduction details shared by optional analysis features.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// Algorithm name and revision, independently of the tool version.
    pub implementation: String,
    /// Producing package version.
    pub tool_version: String,
    /// SPDX expression for this implementation.
    pub code_license: String,
    /// Exact resources, in producer-declared stable order.
    pub resources: Vec<Resource>,
    /// Explicit preprocessing, execution and sampling settings.
    pub settings: BTreeMap<String, String>,
}

impl Provenance {
    /// Starts a native implementation manifest without inventing resource hashes.
    pub fn native(implementation: &str) -> Self {
        Self {
            implementation: implementation.into(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            code_license: "MIT OR Apache-2.0".into(),
            resources: Vec::new(),
            settings: BTreeMap::new(),
        }
    }
}

/// One optional result. Historical absence cannot imply successful execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Analysis<T> {
    /// Whether this producer established the requested evidence.
    pub capability: Capability,
    /// Reproduction information; absent for historical, unrecorded evidence.
    pub provenance: Option<Provenance>,
    /// Measurements, never an independent approval authority.
    pub evidence: Option<T>,
    /// Scope restrictions in stable order.
    pub limitations: Vec<String>,
}
impl<T> Default for Analysis<T> {
    fn default() -> Self {
        Self {
            capability: Capability::default(),
            provenance: None,
            evidence: None,
            limitations: vec!["Historical absence does not establish that a check ran.".into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_absence_and_unknown_fields_are_not_success() {
        let unknown = Analysis::<u32>::default();
        assert!(matches!(unknown.capability, Capability::Unknown { .. }));
        let value = serde_json::to_value(&unknown).unwrap_or_default();
        assert_eq!(
            serde_json::from_value::<Analysis<u32>>(value.clone()).ok(),
            Some(unknown)
        );
        let mut newer = value;
        newer["future_authority"] = true.into();
        assert!(serde_json::from_value::<Analysis<u32>>(newer).is_err());
    }
}
