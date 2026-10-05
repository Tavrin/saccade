//! Source-first UI text/layout review combined with frozen localized measurements.
use crate::{
    Error, Result,
    evidence::canonical::{self, Digest},
    localized,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Capture-bound producer text and layout facts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// saccade-ui-source.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-ui-source.v1")))]
    pub schema: String,
    /// Exact screenshot file SHA-256 (64 lowercase hex, like DOM region metadata).
    pub capture_sha256: String,
    /// Capture-pixel dimensions.
    pub dimensions: [u32; 2],
    /// dom, accessibility_tree, or tesseract_tsv. Never inferred from image geometry.
    pub kind: String,
    /// Producer/version, or pinned OCR executable/model hashes and settings.
    pub producer: serde_json::Value,
    /// Stable producer IDs for DOM/AX; OCR IDs name geometric word slots only.
    pub nodes: Vec<Node>,
    /// True only if the producer declares complete captured source scope.
    pub complete: bool,
}
/// Optional external Tesseract contract; readable without the CLI OCR feature.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OcrContract {
    /// saccade-tesseract.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-tesseract.v1")))]
    pub schema: String,
    /// Pinned local executable, resolved relative to the contract.
    pub executable: std::path::PathBuf,
    /// Exact executable bytes.
    pub executable_sha256: Digest,
    /// Expected version on the first line of --version output.
    pub version: String,
    /// Exact version output, retaining linked-library versions.
    pub version_output_sha256: Digest,
    /// Directory of pinned traineddata, relative to the contract.
    pub tessdata: std::path::PathBuf,
    /// Language names and exact traineddata hashes.
    pub models: BTreeMap<String, Digest>,
    /// Fixed segmentation mode, 6 or 11.
    pub psm: u8,
    /// Per-process timeout, 1..60000 milliseconds.
    pub timeout_ms: u64,
}
/// One visible/semantic node exported by the capture producer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Stable source ID, never matched by approximate text.
    pub id: String,
    /// Exact Unicode content; prices and identifiers are not normalized.
    pub text: String,
    /// Producer-declared role, not an OCR inference.
    #[serde(default)]
    pub role: String,
    /// x,y,width,height in capture pixels; omitted when unavailable.
    pub bounds: Option<[f64; 4]>,
    /// Authoritative producer reading ordinal; never geometric top-to-bottom order.
    pub reading_order: Option<u32>,
    /// Producer keyboard traversal ordinal; omitted for nonfocusable nodes.
    pub keyboard_order: Option<u32>,
    /// Explicit disclosure/footnote declaration from the source producer.
    #[serde(default)]
    pub disclosure: bool,
    /// OCR confidence, retained as observation rather than probability/proof.
    pub ocr_confidence: Option<f64>,
}
/// Independent source or uncertain OCR change.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// text_changed, missing_disclosure, reading_order_changed, etc.
    pub rule: String,
    /// Stable node ID or order scope.
    pub subject: String,
    /// source_fact or uncertain_ocr; neither confers semantic success.
    pub assurance: String,
    /// Before/after values retained exactly.
    pub evidence: serde_json::Value,
}
/// One review packet, retaining independent raw/localized measurements.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// saccade-ui-review.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-ui-review.v1")))]
    pub schema: String,
    /// Exact source evidence hashes and producer facts.
    pub sources: [Source; 2],
    /// Semantic hashes of supplied source documents.
    pub source_identities: [Digest; 2],
    /// Text/order/layout findings.
    pub findings: Vec<Finding>,
    /// Existing region, boundary and complement checks without exclusions.
    pub localized: localized::Measurement,
    /// Source coverage and interpretation limits.
    pub limits: Vec<String>,
}
fn invalid(message: &str) -> Error {
    Error::Config(message.into())
}
impl Source {
    /// Reject stale captures, duplicate IDs/orders, and OCR-promoted semantics.
    pub fn validate(&self, hash: &str, dimensions: [u32; 2]) -> Result<()> {
        if self.schema != "saccade-ui-source.v1"
            || self.capture_sha256 != hash
            || self.dimensions != dimensions
            || dimensions.contains(&0)
            || self.nodes.len() > 100_000
            || !["dom", "accessibility_tree", "tesseract_tsv", "ocrs"].contains(&self.kind.as_str())
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
        {
            return Err(invalid(
                "UI source schema, kind, dimensions or screenshot identity mismatch",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut reading = BTreeSet::new();
        let mut keyboard = BTreeSet::new();
        for n in &self.nodes {
            if n.id.is_empty()
                || !ids.insert(&n.id)
                || n.reading_order.is_some_and(|v| !reading.insert(v))
                || n.keyboard_order.is_some_and(|v| !keyboard.insert(v))
                || n.ocr_confidence
                    .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
                || n.bounds
                    .is_some_and(|b| b.iter().any(|v| !v.is_finite()) || b[2] < 0.0 || b[3] < 0.0)
            {
                return Err(invalid(
                    "invalid UI node IDs, ordinals, bounds or confidence",
                ));
            }
            if matches!(self.kind.as_str(), "tesseract_tsv" | "ocrs")
                && (n.reading_order.is_some()
                    || n.keyboard_order.is_some()
                    || n.disclosure
                    || !n.role.is_empty())
            {
                return Err(invalid(
                    "OCR cannot establish semantic role, disclosure or source order",
                ));
            }
        }
        if matches!(self.kind.as_str(), "tesseract_tsv" | "ocrs") && self.complete {
            return Err(invalid("OCR cannot establish complete source coverage"));
        }
        Ok(())
    }
}
fn finding(rule: &str, subject: &str, assurance: &str, evidence: serde_json::Value) -> Finding {
    Finding {
        rule: rule.into(),
        subject: subject.into(),
        assurance: assurance.into(),
        evidence,
    }
}
/// Compare stable producer IDs, retaining OCR uncertainty even at confidence 100.
pub fn compare(
    before: &Source,
    after: &Source,
    measurement: localized::Measurement,
) -> Result<Report> {
    use serde_json::json;
    before.validate(
        &measurement.region.reference_sha256,
        measurement.region.dimensions,
    )?;
    after.validate(&measurement.candidate_sha256, measurement.region.dimensions)?;
    let source = !matches!(before.kind.as_str(), "tesseract_tsv" | "ocrs")
        && !matches!(after.kind.as_str(), "tesseract_tsv" | "ocrs")
        && before.kind == after.kind;
    let assurance = if source {
        "source_fact"
    } else if matches!(before.kind.as_str(), "tesseract_tsv" | "ocrs")
        || matches!(after.kind.as_str(), "tesseract_tsv" | "ocrs")
    {
        "uncertain_ocr"
    } else {
        "uncertain_correspondence"
    };
    let b: BTreeMap<_, _> = before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let a: BTreeMap<_, _> = after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut findings = Vec::new();
    if before.kind != after.kind {
        findings.push(finding("source_kind_changed","capture","uncertain_ocr",json!({"before":before.kind,"after":after.kind,"matching":"IDs may not share semantics"})));
    }
    for id in b.keys().chain(a.keys()).copied().collect::<BTreeSet<_>>() {
        match (b.get(id),a.get(id)) {
            (Some(b),Some(a))=> {
                if b.text!=a.text {findings.push(finding("text_changed",id,assurance,json!({"before":b.text,"after":a.text,"confidence_before":b.ocr_confidence,"confidence_after":a.ocr_confidence})));}
                if b.bounds!=a.bounds {findings.push(finding("layout_changed",id,assurance,json!({"before":b.bounds,"after":a.bounds})));}
                if source && (b.role!=a.role||b.disclosure!=a.disclosure) {findings.push(finding("semantic_role_changed",id,assurance,json!({"before_role":b.role,"after_role":a.role,"before_disclosure":b.disclosure,"after_disclosure":a.disclosure})));}
            }
            (Some(b),None)=>findings.push(finding(if source&&b.disclosure&&after.complete {"missing_disclosure"}else if source&&after.complete {"node_removed"}else{"node_not_observed"},id,assurance,json!({"before":b.text,"candidate_source_complete":after.complete,"confidence":b.ocr_confidence}))),
            (None,Some(a))=>findings.push(finding("node_added",id,assurance,json!({"after":a.text,"confidence":a.ocr_confidence}))),
            _=>{}
        }
    }
    if source {
        for keyboard in [false, true] {
            let ordinal = |n: &Node| {
                if keyboard {
                    n.keyboard_order
                } else {
                    n.reading_order
                }
            };
            // Compare only ordinals observed on both sides of the same common node.
            let common: Vec<_> = b
                .keys()
                .filter(|id| a.contains_key(**id))
                .copied()
                .collect();
            let missing_before: Vec<_> = common
                .iter()
                .filter(|id| ordinal(b[**id]).is_none())
                .copied()
                .collect();
            let missing_after: Vec<_> = common
                .iter()
                .filter(|id| ordinal(a[**id]).is_none())
                .copied()
                .collect();
            if !missing_before.is_empty() || !missing_after.is_empty() {
                findings.push(finding(
                    if keyboard { "keyboard_order_coverage" } else { "reading_order_coverage" },
                    "common_nodes", "unavailable",
                    json!({"missing_before":missing_before,"missing_after":missing_after,"reason":"relative order is compared only for jointly observed producer ordinals"}),
                ));
            }
            let supported: BTreeSet<_> = common
                .into_iter()
                .filter(|id| ordinal(b[*id]).is_some() && ordinal(a[*id]).is_some())
                .collect();
            // Relative sequence avoids labelling reindexing after deletion as a reorder.
            let order = |s: &Source| {
                let mut nodes: Vec<_> = s
                    .nodes
                    .iter()
                    .filter(|n| supported.contains(n.id.as_str()))
                    .filter_map(|n| ordinal(n).map(|i| (i, n.id.as_str())))
                    .collect();
                nodes.sort_unstable();
                nodes
                    .into_iter()
                    .map(|(_, id)| id.to_owned())
                    .collect::<Vec<_>>()
            };
            let old = order(before);
            let new = order(after);
            if old != new {
                findings.push(finding(if keyboard {"keyboard_order_changed"}else{"reading_order_changed"},"common_nodes",assurance,json!({"before":old,"after":new,"authority":"producer ordinals; not geometric order"})));
            }
        }
    }
    let mut limits=vec!["Producer text/layout facts are screenshot-bound assertions, not independently recaptured behaviour or successful interaction.".into(),"Exact text is compared without case, price, punctuation or Unicode normalization; raster reflow and semantic order are separate findings.".into(),"Localized raw FLIP/native samples, intended region, boundary and protected complement retain independent authority; no semantic success or approval is inferred.".into()];
    if !before.complete || !after.complete {
        limits.push(
            "At least one source scope is incomplete: absent nodes are not proof of removal."
                .into(),
        );
    }
    if !source {
        limits.push("OCR word-slot matching can shift on resegmentation; confidence never proves exact text or application defects. Reading/keyboard order and disclosures are unavailable.".into());
    }
    Ok(Report {
        schema: "saccade-ui-review.v1".into(),
        source_identities: [
            canonical::digest(before).map_err(|e| invalid(&e.to_string()))?,
            canonical::digest(after).map_err(|e| invalid(&e.to_string()))?,
        ],
        sources: [before.clone(), after.clone()],
        findings,
        localized: measurement,
        limits,
    })
}
/// Parse Tesseract word-level TSV from the one optional pinned backend.
/// TSV recognition remains uncertain; words receive geometric slot IDs only.
pub fn tesseract_tsv(
    tsv: &str,
    hash: String,
    dimensions: [u32; 2],
    producer: serde_json::Value,
) -> Result<Source> {
    let mut lines = tsv.lines();
    if lines.next()
        != Some(
            "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext",
        )
    {
        return Err(invalid("unexpected Tesseract TSV header"));
    }
    let mut nodes = Vec::new();
    for line in lines {
        let values: Vec<_> = line.splitn(12, '\t').collect();
        if values.len() != 12 {
            return Err(invalid("malformed Tesseract TSV row"));
        }
        if values[0] != "5" {
            continue;
        }
        let number = |i: usize| {
            values[i]
                .parse::<f64>()
                .map_err(|_| invalid("invalid Tesseract TSV number"))
        };
        let confidence = number(10)?;
        if values[11].is_empty() {
            continue;
        }
        let bounds = [number(6)?, number(7)?, number(8)?, number(9)?];
        if bounds[0] < 0.0
            || bounds[1] < 0.0
            || bounds[0] + bounds[2] > f64::from(dimensions[0])
            || bounds[1] + bounds[3] > f64::from(dimensions[1])
        {
            return Err(invalid("OCR box outside screenshot"));
        }
        nodes.push(Node {
            id: format!("ocr-slot:{}", values[1..6].join("/")),
            text: values[11].into(),
            role: String::new(),
            bounds: Some(bounds),
            reading_order: None,
            keyboard_order: None,
            disclosure: false,
            ocr_confidence: Some(confidence),
        });
    }
    let source = Source {
        schema: "saccade-ui-source.v1".into(),
        capture_sha256: hash,
        dimensions,
        kind: "tesseract_tsv".into(),
        producer,
        nodes,
        complete: false,
    };
    source.validate(&source.capture_sha256, dimensions)?;
    Ok(source)
}
