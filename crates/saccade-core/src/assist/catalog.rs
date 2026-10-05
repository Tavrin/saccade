//! Immutable typed evidence catalog, kept separate from model observations.
use super::{
    Result, digest,
    geometry::{Geometry, Transform},
    require,
    schema::{CATALOG_VERSION, Identity, POLICY_VERSION, Role, Task},
};
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Original screenshot scope; pre-masked captures cannot reveal original content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// Stable local image role, never included as a provider mutation label.
    pub role: Role,
    /// Exact screenshot file bytes.
    pub sha256: Digest,
    /// Exact encoded PNG view bytes, separately bound from the original file.
    pub encoded_sha256: Digest,
    /// Original-pixel dimensions.
    pub dimensions: [u32; 2],
    /// Captured geometry within the original image.
    pub capture_scope: [u32; 4],
    /// True only when the producer declares the entire requested scope captured.
    pub complete: bool,
    /// False when blacking out happened before capture.
    pub original_pixels: bool,
    /// Identity or explicit crop/resize mapping.
    pub transform: Transform,
}
/// Catalog citation target; existing region geometry does not prove a statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// Stable request-local ID.
    pub id: String,
    /// Owning image role.
    pub image_role: Role,
    /// Exact original-pixel rectangle.
    pub rect: [u32; 4],
}
/// Exclusion identity before union; overlap membership is retained in runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Mask {
    /// Stable content/config identity.
    pub id: String,
    /// Producer/config origin.
    pub origin: String,
    /// Stated reason; missing rationale stays explicit.
    pub rationale: Option<String>,
    /// Dimensions of membership map.
    pub dimensions: [u32; 2],
    /// Sorted nonoverlapping row-major [start,length] runs, before union.
    pub runs: Vec<[u64; 2]>,
    /// Exact row-major 0/1 membership hash.
    pub membership_hash: Digest,
    /// True only for masks that retain the original pixels.
    pub original_pixels: bool,
}
/// Only screenshot-answerable conditions are accepted; behavioral success has no variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// Exact label visibly present in a bounded region.
    LabelVisible {
        /// Requested label; untrusted literal data.
        label: String,
    },
    /// Known target must be wholly within known panel.
    NotClipped {
        /// Catalog target ID.
        target: String,
        /// Catalog containing panel ID.
        panel: String,
    },
    /// Known catalog regions must not overlap.
    NonOverlap {
        /// First region ID.
        first: String,
        /// Second region ID.
        second: String,
    },
    /// Named error/banner absent within complete bounded capture scope.
    BannerAbsent {
        /// Requested banner label; untrusted literal data.
        label: String,
    },
}
/// Existing measurements/source facts are never serialized as model observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    /// assist-catalog/1.
    pub version: String,
    /// Immutable screenshot descriptors.
    pub images: Vec<Image>,
    /// Citation targets.
    pub regions: Vec<Region>,
    /// Individual exclusions, possibly overlapping.
    pub exclusions: Vec<Mask>,
    /// Numerical report measurements, attributed as deterministic.
    pub measurements: serde_json::Value,
    /// Wave 3 hash/dimension-bound source contracts.
    pub source_evidence: Vec<crate::ui_review::Source>,
    /// Exact serialized source packet identities.
    pub source_evidence_hashes: Vec<Digest>,
}
impl Catalog {
    /// Bounded, unique image and reference identities, source packet binding and masks.
    pub fn validate(&self) -> Result<()> {
        require(
            self.version == CATALOG_VERSION
                && !self.images.is_empty()
                && self.images.len() <= 2
                && self.regions.len() <= 256
                && self.exclusions.len() <= 128,
            "catalog version or size",
        )?;
        let mut roles = BTreeSet::new();
        for image in &self.images {
            require(roles.insert(image.role), "duplicate image role")?;
            let pixels = u64::from(image.dimensions[0]) * u64::from(image.dimensions[1]);
            require(pixels > 0 && pixels <= 4_194_304, "image pixel limit")?;
            image.transform.validate(image.dimensions)?;
            Geometry::Box(image.capture_scope.map(f64::from)).validate(image.dimensions)?;
        }
        let mut ids = BTreeSet::new();
        for region in &self.regions {
            require(
                !region.id.is_empty() && region.id.len() <= 160 && ids.insert(&region.id),
                "invalid region ID",
            )?;
            let image = self.image(region.image_role)?;
            Geometry::Box(region.rect.map(f64::from)).validate(image.dimensions)?;
        }
        for mask in &self.exclusions {
            require(
                !mask.id.is_empty()
                    && mask.id.len() <= 160
                    && ids.insert(&mask.id)
                    && !mask.origin.is_empty()
                    && mask.origin.len() <= 512
                    && mask.rationale.as_ref().is_none_or(|r| r.len() <= 1024),
                "invalid mask metadata",
            )?;
            require(
                self.images.iter().all(|i| i.dimensions == mask.dimensions),
                "mask/image dimensions differ",
            )?;
            let bits = mask.bits()?;
            require(
                Digest::of_bytes(&bits) == mask.membership_hash,
                "mask membership identity",
            )?;
        }
        require(
            self.source_evidence.len() <= self.images.len()
                && self.source_evidence.len() == self.source_evidence_hashes.len(),
            "source count mismatch",
        )?;
        let mut source_images = BTreeSet::new();
        for (source, hash) in self
            .source_evidence
            .iter()
            .zip(&self.source_evidence_hashes)
        {
            require(
                digest(source)? == *hash && source_images.insert(&source.capture_sha256),
                "source hash/duplicate",
            )?;
            let image = self
                .images
                .iter()
                .find(|i| i.sha256.as_str()[7..] == source.capture_sha256)
                .ok_or(super::Error::Invalid("source screenshot mismatch"))?;
            source
                .validate(&source.capture_sha256, image.dimensions)
                .map_err(|_| super::Error::Invalid("source contract"))?;
            for node in &source.nodes {
                if let Some(b) = node.bounds {
                    Geometry::Box(b).validate(image.dimensions)?;
                }
            }
        }
        require(
            serde_json::to_vec(&self.measurements)
                .map_err(|_| super::Error::Invalid("measurements"))?
                .len()
                <= 64 * 1024,
            "measurements size",
        )
    }
    /// Resolve original image geometry by its locally assigned role.
    pub fn image(&self, role: Role) -> Result<&Image> {
        self.images
            .iter()
            .find(|i| i.role == role)
            .ok_or(super::Error::Invalid("absent image role"))
    }
    /// Bind the entire catalog and bounded condition, including all masks/settings.
    pub fn identity(
        &self,
        task: Task,
        report_hash: Option<Digest>,
        condition: Option<&Condition>,
    ) -> Result<Identity> {
        self.validate()?;
        require(
            (task == Task::CheckUi) == condition.is_some(),
            "task/condition mismatch",
        )?;
        if let Some(condition) = condition {
            self.validate_condition(condition)?;
        }
        Ok(Identity {
            task,
            request_hash: digest(&(task, &report_hash, self, condition, POLICY_VERSION))?,
            report_hash,
            screenshot_hashes: self.images.iter().map(|i| i.sha256.clone()).collect(),
            condition_hash: condition.map(digest).transpose()?,
            catalog_version: CATALOG_VERSION.into(),
            policy_version: POLICY_VERSION.into(),
        })
    }
    /// A condition cannot refer to absent/cross-image targets or unbounded labels.
    pub fn validate_condition(&self, condition: &Condition) -> Result<()> {
        match condition {
            Condition::LabelVisible { label } | Condition::BannerAbsent { label } => require(
                !label.trim().is_empty() && label.len() <= 512,
                "empty or oversized label",
            ),
            Condition::NotClipped {
                target: first,
                panel: second,
            }
            | Condition::NonOverlap { first, second } => {
                let a = self
                    .regions
                    .iter()
                    .find(|r| &r.id == first)
                    .ok_or(super::Error::Invalid("condition first target missing"))?;
                let b = self
                    .regions
                    .iter()
                    .find(|r| &r.id == second)
                    .ok_or(super::Error::Invalid("condition second target missing"))?;
                require(
                    first != second && a.image_role == b.image_role,
                    "condition target identity",
                )
            }
        }
    }
}
impl Mask {
    /// Decode sorted runs with checked arithmetic and a fixed memory bound.
    pub fn bits(&self) -> Result<Vec<u8>> {
        let pixels = u64::from(self.dimensions[0]) * u64::from(self.dimensions[1]);
        require(
            pixels > 0 && pixels <= 4_194_304 && self.runs.len() <= pixels as usize,
            "mask pixel limit",
        )?;
        let mut bits = vec![0; pixels as usize];
        let mut previous = 0;
        for &[start, length] in &self.runs {
            let end = start
                .checked_add(length)
                .ok_or(super::Error::Invalid("mask overflow"))?;
            require(
                length > 0 && start >= previous && end <= pixels,
                "invalid mask runs",
            )?;
            bits[start as usize..end as usize].fill(1);
            previous = end;
        }
        Ok(bits)
    }
}
/// Closed question definition with task-specific abstention and prohibited conclusions.
#[derive(Debug, Clone, Serialize)]
pub struct Question {
    /// Versioned catalog identity.
    pub id: &'static str,
    /// Fixed answer set.
    pub answers: &'static [&'static str],
    /// Required evidence types.
    pub required: &'static [&'static str],
    /// Mandatory abstention rule.
    pub abstention: &'static str,
    /// Prohibited authority/causal/behavioral conclusions.
    pub prohibited: &'static str,
    /// Task-specific scoring definition.
    pub scoring: &'static str,
}
/// Assist questions extend rather than redefine the historical question catalog.
pub fn questions() -> [Question; 4] {
    let prohibited = "No approval, exclusions, verdict override, root cause, behavioral success or independent truth from model agreement.";
    [
        Question {
            id: "assist.evidence_need.v1",
            answers: &["structured", "vision", "unavailable"],
            required: &["catalog", "condition"],
            abstention: "Missing required scope returns unavailable; required pixels cannot be skipped.",
            prohibited,
            scoring: "Necessary-evidence skip rate and paired cost/recall at matched coverage.",
        },
        Question {
            id: "assist.claim_support.v1",
            answers: &["supported", "unsupported", "insufficient"],
            required: &["validated_observations", "catalog"],
            abstention: "Missing or contradictory evidence returns insufficient.",
            prohibited,
            scoring: "Unsupported committed assertion reduction and coverage loss.",
        },
        Question {
            id: "assist.mask_relevance.v1",
            answers: &["potentially_relevant", "not_observed", "unavailable"],
            required: &["original_pixels", "individual_masks"],
            abstention: "Pre-masked pixels or missing expected content cannot prove safe masks.",
            prohibited,
            scoring: "Important masked change recall, false reassurance, assertion precision.",
        },
        Question {
            id: "assist.ui_visible.v1",
            answers: &["observed", "not_observed", "unverifiable"],
            required: &["bounded_condition", "capture_scope"],
            abstention: "Incomplete capture, ambiguity or behavioral request is unverifiable.",
            prohibited,
            scoring: "Bounded visible condition accuracy, availability and false reassurance.",
        },
    ]
}
