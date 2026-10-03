//! Human final label collection with immutable evidence identities and provenance.
#[cfg(feature = "evaluation")]
use crate::judge::{EvidenceOptions, JudgeItem, JudgeQuestion, PixelSource, report_items};
use crate::report::Hotspot;
#[cfg(feature = "evaluation")]
use crate::report::Report;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
#[cfg(feature = "evaluation")]
use std::path::PathBuf;

/// Labels schema identifier.
pub const SCHEMA: &str = "saccade-labels.v1";

/// Current canonical label writer schema; v1 above remains a historical reader.
pub const QUESTION_LABELS_SCHEMA: &str = "saccade-labels.v2";
/// Experimental evaluation family holds rubric/exposure/re-label audit records;
/// exported labels keep the exact R4 labels.v2 shape.
pub const LABEL_RECORDS_SCHEMA: &str = "saccade-evaluation.v1";
/// Human routing rubric, independently of vision-provider outcomes.
pub const VISION_RUBRIC_VERSION: &str = "vision-route-human/1";

/// A three-valued human assessment; uncertainty is never forced to yes/no.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PixelNeed {
    /// A necessary visual fact was absent from the structured packet.
    Yes,
    /// The structured packet sufficed without visual judgment.
    No,
    /// Available evidence did not establish pixel need.
    Undetermined,
}
/// Minimum useful visual context, independent of a need for human authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VisualScope {
    /// No visual inspection was necessary.
    None,
    /// Specified regions with recorded context sufficed.
    Regions,
    /// Context outside the available regions was necessary.
    FullFrame,
}
/// Assessment recorded before pixels or model proposals are exposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StructuredAssessment {
    /// Hash of the exact structured packet shown to the routing model.
    pub packet_sha256: crate::evidence::canonical::Digest,
    /// Whether that packet supports the task disposition.
    pub supports_disposition: PixelNeed,
    /// The unresolved visual fact, if any.
    pub unresolved_visual_fact: Option<String>,
    /// Provisional catalog route, retained even when final assessment differs.
    pub provisional_route: String,
    /// Human uncertainty before pixels.
    pub uncertainty: String,
    /// Audit time establishing structured-first order.
    pub recorded_unix_ms: u64,
}
/// Assessment after anonymous regions and, when necessary, full-frame context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VisualAssessment {
    /// Human ground truth, independent of Gemini success or agreement.
    pub requires_pixels: PixelNeed,
    /// Minimum useful visual context.
    pub minimum_scope: VisualScope,
    /// Authority/ambiguity/rubric requires a human, separately from pixel need.
    pub requires_human: bool,
    /// Concrete fact that was necessary or remained unavailable.
    pub necessary_visual_fact: Option<String>,
    /// Anonymous region/context references actually inspected.
    pub region_refs: Vec<String>,
    /// Whether the reviewer then saw anonymous full-frame context.
    pub full_frame_seen: bool,
    /// Corresponding final route label.
    pub route: String,
    /// Audit time after initial assessment.
    pub recorded_unix_ms: u64,
}
/// Structured-first human rubric. No provider answer or outcome enters it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VisionRoutingRubric {
    /// Always vision-route-human/1.
    pub version: String,
    /// Initial judgment retained verbatim.
    pub initial: StructuredAssessment,
    /// Final judgment, including unresolved pixel need.
    pub final_assessment: VisualAssessment,
}
impl VisionRoutingRubric {
    /// Checks the staged rubric against the exact model packet and route label.
    pub fn validate_for(
        &self,
        request: &crate::evidence::request::DecisionRequest,
        answer: &str,
    ) -> crate::evidence::Result<()> {
        use crate::evidence::require;
        crate::questions::validate_request(request)?;
        require(
            request.question.id == "vision.route.v1" && self.version == VISION_RUBRIC_VERSION,
            "vision rubric/question version mismatch",
        )?;
        require(
            self.initial.packet_sha256 == crate::evidence::canonical::digest(&request.evidence)?,
            "human saw a different structured packet",
        )?;
        require(
            request
                .question
                .answers
                .contains(&self.initial.provisional_route)
                && self.final_assessment.route == answer
                && request.question.answers.iter().any(|a| a == answer),
            "vision rubric route differs from its label",
        )?;
        let final_view = &self.final_assessment;
        require(
            self.initial.recorded_unix_ms < final_view.recorded_unix_ms,
            "record structured assessment before showing pixels",
        )?;
        let concrete = final_view
            .necessary_visual_fact
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());
        require(
            final_view.requires_pixels != PixelNeed::Yes
                || (concrete && final_view.minimum_scope != VisualScope::None),
            "pixel requirement needs a concrete visual fact and useful scope",
        )?;
        require(
            final_view.requires_pixels != PixelNeed::No
                || final_view.minimum_scope == VisualScope::None,
            "no pixel need must have scope none",
        )?;
        require(
            final_view.minimum_scope != VisualScope::Regions || !final_view.region_refs.is_empty(),
            "region judgment needs inspected region references",
        )?;
        require(
            final_view.minimum_scope != VisualScope::FullFrame || final_view.full_frame_seen,
            "full-frame judgment needs inspected full-frame context",
        )?;
        let route_matches = match answer {
            "text_sufficient" => {
                final_view.requires_pixels == PixelNeed::No
                    && !final_view.requires_human
                    && self.initial.supports_disposition == PixelNeed::Yes
            }
            "inspect_regions" => {
                final_view.requires_pixels == PixelNeed::Yes
                    && final_view.minimum_scope == VisualScope::Regions
                    && !final_view.requires_human
            }
            "inspect_full_frame" => {
                final_view.requires_pixels == PixelNeed::Yes
                    && final_view.minimum_scope == VisualScope::FullFrame
                    && !final_view.requires_human
            }
            "human_directly" => final_view.requires_human,
            "abstain" => final_view.requires_pixels == PixelNeed::Undetermined,
            _ => false,
        };
        require(
            route_matches,
            "route disagrees with separate pixel/scope/human assessments",
        )
    }
}

/// One auditable human label and its exposure/test-retest history. Canonical
/// bindings are reused; a baseline decision never fabricates question answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QuestionLabel {
    /// Identity of this exact assessment, exposure and revision history.
    pub label_id: crate::evidence::canonical::Digest,
    /// R4 case/request/question/human-decision binding.
    pub label: crate::evidence::human::Label,
    /// Whether the human had seen model proposals before recording the label.
    pub saw_model_proposals: crate::evidence::case::Availability<bool>,
    /// Required only for vision.route; includes both initial and final assessments.
    pub vision_rubric: Option<VisionRoutingRubric>,
    /// Earlier label being superseded; earlier records remain in the collection.
    pub supersedes: Option<crate::evidence::canonical::Digest>,
    /// Earlier assessment independently re-labeled for test-retest.
    pub test_retest_of: Option<crate::evidence::canonical::Digest>,
}
impl QuestionLabel {
    /// Content identity preserves exposures, disagreement and revisions.
    pub fn identity(&self) -> crate::evidence::Result<crate::evidence::canonical::Digest> {
        crate::evidence::canonical::digest(
            &json!({"label":self.label,"saw_model_proposals":self.saw_model_proposals,
            "vision_rubric":self.vision_rubric,"supersedes":self.supersedes,"test_retest_of":self.test_retest_of}),
        )
    }
    /// Binds explicit human input to an exact question; never accepts a proposal.
    pub fn validate_for(
        &self,
        request: &crate::evidence::request::DecisionRequest,
        decision: &crate::evidence::human::HumanDecision,
    ) -> crate::evidence::Result<()> {
        use crate::evidence::require;
        self.label.validate_for(request, decision)?;
        crate::questions::validate_request(request)?;
        require(self.label_id == self.identity()?, "stale question label")?;
        self.saw_model_proposals.validate()?;
        require(
            self.supersedes.as_ref() != Some(&self.label_id)
                && self.test_retest_of.as_ref() != Some(&self.label_id),
            "label cannot supersede or re-label itself",
        )?;
        match (&self.vision_rubric, request.question.id.as_str()) {
            (Some(rubric), "vision.route.v1") => rubric.validate_for(request, &self.label.answer),
            (None, "vision.route.v1") => Err(crate::evidence::ContractError::Invalid(
                "vision.route labels require the human pixel-need rubric".into(),
            )),
            (None, _) => Ok(()),
            (Some(_), _) => Err(crate::evidence::ContractError::Invalid(
                "vision rubric on a different question".into(),
            )),
        }
    }
    /// Eligibility for unexposed human-ground-truth scoring; unknown exposure
    /// remains recorded but does not silently qualify a label.
    pub fn eligible(&self) -> bool {
        self.saw_model_proposals.value() == Some(&false)
            && self.label.answer != "abstain"
            && (self.label.question_id != "vision.route.v1"
                || (self.label.exposure.mapping_access.value() == Some(&false)
                    && self.label.exposure.implementation_context.value() == Some(&false)))
    }
}

/// Current label collection retains disagreeing/superseded/repeated assessments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QuestionLabels {
    /// Always saccade-evaluation.v1; this is a label audit collection.
    pub schema: String,
    /// Always human_label_records, reserving other evaluation job types.
    pub kind: String,
    /// Explicit question-level human records.
    pub items: Vec<QuestionLabel>,
}
impl QuestionLabels {
    /// Validates exact request and human bindings before exporting eligible labels.
    pub fn validate_for(
        &self,
        requests: &[crate::evidence::request::DecisionRequest],
        decisions: &[crate::evidence::human::HumanDecision],
    ) -> crate::evidence::Result<()> {
        use crate::evidence::require;
        require(
            self.schema == LABEL_RECORDS_SCHEMA && self.kind == "human_label_records",
            "unsupported human label record schema/kind",
        )?;
        let mut seen = std::collections::BTreeSet::new();
        for item in &self.items {
            require(
                seen.insert(&item.label_id),
                "duplicate question label identity",
            )?;
            let request = requests
                .iter()
                .find(|r| r.request_id == item.label.request_id)
                .ok_or_else(|| {
                    crate::evidence::ContractError::Invalid("unknown labeled request".into())
                })?;
            let decision = decisions
                .iter()
                .find(|d| d.decision_id == item.label.human_decision_id)
                .ok_or_else(|| {
                    crate::evidence::ContractError::Invalid("unknown human label decision".into())
                })?;
            item.validate_for(request, decision)?;
            for previous in [&item.supersedes, &item.test_retest_of]
                .into_iter()
                .flatten()
            {
                let earlier = self
                    .items
                    .iter()
                    .find(|i| &i.label_id == previous)
                    .ok_or_else(|| {
                        crate::evidence::ContractError::Invalid(
                            "missing prior human assessment".into(),
                        )
                    })?;
                require(
                    seen.contains(previous)
                        && earlier.label_id != item.label_id
                        && earlier.label.case_id == item.label.case_id
                        && earlier.label.request_id == item.label.request_id,
                    "revision/test-retest must reference an earlier assessment of the same question",
                )?;
            }
        }
        Ok(())
    }
    /// Exports canonical eligible labels locally, leaving all historical assessments
    /// in this collection. Superseded records are excluded from current scoring.
    pub fn eligible_labels(
        &self,
        requests: &[crate::evidence::request::DecisionRequest],
        decisions: &[crate::evidence::human::HumanDecision],
    ) -> crate::evidence::Result<crate::evidence::human::Labels> {
        self.validate_for(requests, decisions)?;
        let current: Vec<_> = self
            .items
            .iter()
            .filter(|i| {
                i.eligible()
                    && !self
                        .items
                        .iter()
                        .any(|later| later.supersedes.as_ref() == Some(&i.label_id))
            })
            .collect();
        let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for item in current {
            groups
                .entry((&item.label.case_id, &item.label.request_id))
                .or_default()
                .push(item);
        }
        Ok(crate::evidence::human::Labels {
            schema: QUESTION_LABELS_SCHEMA.into(),
            items: groups
                .into_values()
                .filter_map(|group| {
                    group
                        .first()
                        .filter(|first| group.iter().all(|i| i.label.answer == first.label.answer))
                        .map(|i| i.label.clone())
                })
                .collect(),
        })
    }
    /// Reads exact offline rubric records and validates every human/question binding.
    pub fn read(
        path: &Path,
        requests: &[crate::evidence::request::DecisionRequest],
        decisions: &[crate::evidence::human::HumanDecision],
    ) -> crate::evidence::Result<Self> {
        let records: Self =
            crate::evidence::canonical::decode(&std::fs::read(crate::paths::native(path))?)?;
        records.validate_for(requests, decisions)?;
        Ok(records)
    }
    /// Persists all rubric/history records locally after validation.
    pub fn write(
        &self,
        path: &Path,
        requests: &[crate::evidence::request::DecisionRequest],
        decisions: &[crate::evidence::human::HumanDecision],
    ) -> crate::evidence::Result<()> {
        self.validate_for(requests, decisions)?;
        std::fs::write(
            crate::paths::native(path),
            crate::evidence::canonical::bytes(self)?,
        )?;
        Ok(())
    }
    /// Writes only canonical labels.v2; model proposals and automatic approval
    /// inference are never sources for these labels.
    pub fn export(
        &self,
        path: &Path,
        requests: &[crate::evidence::request::DecisionRequest],
        decisions: &[crate::evidence::human::HumanDecision],
    ) -> crate::evidence::Result<()> {
        let labels = self.eligible_labels(requests, decisions)?;
        std::fs::write(
            crate::paths::native(path),
            crate::evidence::canonical::bytes(&labels)?,
        )?;
        Ok(())
    }
}

/// Separate pixel-need and route measurements; undetermined labels have no
/// binary pixel ground truth, and unavailable provider responses are not errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisionScore {
    /// Route agreement, separate from pixel requirement.
    pub route_agreement: Option<bool>,
    /// Whether a human-required pixel case was detected.
    pub pixel_requirement_detected: Option<bool>,
    /// Unnecessary visual request on a human no-pixels case.
    pub unnecessary_visual_request: Option<bool>,
}
/// Scores human pixel need independently of the provider's success or agreement.
pub fn score_vision(
    rubric: &VisionRoutingRubric,
    proposed_route: Option<&str>,
    proposed_pixel_need: Option<PixelNeed>,
) -> crate::evidence::Result<VisionScore> {
    let q = crate::questions::lookup("vision.route.v1")?;
    crate::evidence::require(
        proposed_route.is_none_or(|a| q.answers.contains(&a)),
        "invalid proposed vision route",
    )?;
    let route = proposed_route.filter(|a| *a != "abstain");
    let human = &rubric.final_assessment;
    Ok(VisionScore {
        route_agreement: route.map(|r| r == human.route),
        pixel_requirement_detected: if human.requires_pixels == PixelNeed::Yes {
            proposed_pixel_need
                .filter(|n| *n != PixelNeed::Undetermined)
                .map(|n| n == PixelNeed::Yes)
        } else {
            None
        },
        unnecessary_visual_request: if human.requires_pixels == PixelNeed::No {
            route.map(|r| matches!(r, "inspect_regions" | "inspect_full_frame"))
        } else {
            None
        },
    })
}

/// One human final label, usable without a live server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Label {
    /// Entry name.
    pub entry: String,
    /// Bounded question name.
    pub question: String,
    /// Human final answer in stable order (a/b for preference).
    pub answer: String,
    /// Hash of the exact evidence this final is about.
    pub evidence_hash: String,
    /// Encoded evidence in stable order.
    pub state: Value,
    /// Intent or rubric.
    pub intent: Option<String>,
    /// Reference and candidate paths, relative to the labels document.
    pub images: Vec<String>,
    /// SHA-256 of image file bytes in the same order.
    pub sha256: Vec<Option<String>>,
    /// Crop selection.
    pub hotspots: Vec<Hotspot>,
    /// Report path, relative to the labels document, when available.
    pub report: Option<String>,
    /// Human identity, timestamp and source file(s).
    pub provenance: Vec<Value>,
}
/// Deduplicated label set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Labels {
    /// Always saccade-labels.v1.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-labels.v1")))]
    pub schema: String,
    /// Evidence-bound human finals.
    pub items: Vec<Label>,
}

/// Canonical identity shared with review; paths and provenance do not affect it.
pub fn identity(label: &Label) -> String {
    let states = if label.state.get("ab").is_some() && label.state.get("ba").is_some() {
        [label.state["ab"].clone(), label.state["ba"].clone()]
    } else {
        [label.state.clone(), label.state.clone()]
    };
    hash(&json!([
        label.entry,
        label.question,
        states,
        label.intent,
        label.sha256,
        label.hotspots
    ]))
}

impl Labels {
    /// Read, validate closed answers, hashes and duplicate consistency.
    pub fn read(path: &Path) -> Result<Self> {
        let set: Self = serde_json::from_slice(
            &std::fs::read(path).map_err(crate::run::io_err("reading labels".into()))?,
        )?;
        if set.schema != SCHEMA {
            return Err(Error::Config("unsupported labels schema".into()));
        }
        let mut seen = BTreeMap::new();
        for i in &set.items {
            let valid = if i.question == "preference" {
                ["a", "b", "tie"].contains(&i.answer.as_str())
            } else {
                let q = crate::decision::Question::parse(&i.question)
                    .ok_or_else(|| Error::Config("unknown label question".into()))?;
                q.allowed_answers().contains(&i.answer.as_str())
                    && !crate::judge_stats::is_abstain(&i.answer)
            };
            if i.evidence_hash != identity(i) {
                return Err(Error::Config(format!(
                    "label evidence identity mismatch: {}",
                    i.entry
                )));
            }
            if !valid || i.provenance.is_empty() || i.images.len() != i.sha256.len() {
                return Err(Error::Config(
                    "labels require final answers, evidence, hashes and provenance".into(),
                ));
            }
            if let Some(previous) = seen.insert((&i.evidence_hash, &i.question), &i.answer)
                && previous != &i.answer
            {
                return Err(Error::Config(
                    "conflicting human finals for the same evidence".into(),
                ));
            }
        }
        Ok(set)
    }
}

/// Create a replay item, verifying stored image identities before any upload.
#[cfg(feature = "evaluation")]
pub fn replay(label: &Label, document: &Path) -> Result<JudgeItem> {
    if label.evidence_hash != identity(label) {
        return Err(Error::Config("label evidence identity mismatch".into()));
    }
    let mut source = None;
    if label.images.len() == 2 {
        let paths: Vec<_> = label
            .images
            .iter()
            .map(|p| crate::paths::resolve(p, document))
            .collect();
        for (p, h) in paths.iter().zip(&label.sha256) {
            let actual = crate::run::sha256_file(p)?;
            if h.as_ref() != Some(&actual) {
                return Err(Error::Config(format!(
                    "label evidence changed: {}",
                    p.display()
                )));
            }
        }
        source = Some(PixelSource::Files(paths[0].clone(), paths[1].clone()));
    }
    Ok(JudgeItem {
        id: format!("{}:{}", label.entry, label.question),
        entry: label.entry.clone(),
        hotspot: None,
        question: JudgeQuestion::parse(&label.question)
            .ok_or_else(|| Error::Config("unknown label question".into()))?,
        request_hash: label.evidence_hash.clone(),
        states: if label.state.get("ab").is_some() && label.state.get("ba").is_some() {
            [label.state["ab"].clone(), label.state["ba"].clone()]
        } else {
            [label.state.clone(), label.state.clone()]
        },
        source,
        hotspots: label.hotspots.clone(),
        canary: None,
        pair: None,
        intent: label.intent.clone(),
    })
}

#[cfg(feature = "evaluation")]
fn add(out: &mut BTreeMap<(String, String), Label>, label: Label) -> Result<()> {
    let key = (label.evidence_hash.clone(), label.question.clone());
    if let Some(old) = out.get_mut(&key) {
        if old.answer != label.answer {
            return Err(Error::Config(format!(
                "conflicting human finals for {}",
                label.entry
            )));
        }
        for p in label.provenance {
            if !old.provenance.contains(&p) {
                old.provenance.push(p);
            }
        }
        if old.images.is_empty() && !label.images.is_empty() {
            old.images = label.images;
            old.sha256 = label.sha256;
            old.state = label.state;
            old.hotspots = label.hotspots;
        }
    } else {
        out.insert(key, label);
    }
    Ok(())
}

#[cfg(feature = "evaluation")]
fn from_report(
    path: &Path,
    entry: &str,
    answer: &str,
    question: &str,
    intent: Option<String>,
    out: &Path,
    provenance: Value,
) -> Result<Label> {
    let report: Report = serde_json::from_slice(
        &std::fs::read(path).map_err(crate::run::io_err("reading labelled report".into()))?,
    )?;
    let e = report
        .entries
        .iter()
        .find(|e| e.name == entry)
        .ok_or_else(|| Error::Config("label entry absent from report".into()))?;
    let q = JudgeQuestion::parse(question)
        .ok_or_else(|| Error::Config("unknown label question".into()))?;
    let item = if crate::decision::deterministic_failure(&report, e).is_none() {
        report_items(
            &report,
            path.parent().unwrap_or(Path::new(".")),
            q,
            intent.as_deref(),
            &[entry.into()],
            &EvidenceOptions::default(),
        )
        .map_err(|e| Error::Config(e.to_string()))?
        .0
        .into_iter()
        .next()
    } else {
        None
    };
    let state = item
        .as_ref()
        .map(|i| json!({"ab":i.states[0],"ba":i.states[1]}))
        .unwrap_or_else(|| crate::decision::entry_state(&report, e, intent.as_deref()));
    let images: Vec<_> = [&e.paths.baseline, &e.paths.capture]
        .into_iter()
        .filter_map(|p| p.as_ref())
        .map(|p| {
            crate::paths::record(
                &crate::paths::resolve(p, path),
                out.parent().unwrap_or(Path::new(".")),
                false,
            )
        })
        .collect();
    let sha256 = [
        (&e.paths.baseline, &e.baseline_sha256),
        (&e.paths.capture, &e.capture_sha256),
    ]
    .into_iter()
    .filter(|(p, _)| p.is_some())
    .map(|(_, h)| h.clone())
    .collect::<Vec<_>>();
    let evidence_hash = item
        .as_ref()
        .map(|i| crate::review::evidence_hash(i, &report))
        .unwrap_or_else(|| hash(&json!([state, sha256])));
    let mut label = Label {
        entry: entry.into(),
        question: question.into(),
        answer: answer.into(),
        evidence_hash,
        state,
        intent,
        images,
        sha256,
        hotspots: e.hotspots.clone(),
        report: Some(crate::paths::record(
            path,
            out.parent().unwrap_or(Path::new(".")),
            false,
        )),
        provenance: vec![provenance],
    };
    label.evidence_hash = identity(&label);
    Ok(label)
}

/// Harvest report decisions, serve sessions, answered review inbox items and
/// consistent human votes in both orders. Model proposals and promoted gates are excluded.
#[cfg(feature = "evaluation")]
pub fn collect(decisions_dir: Option<&Path>, reports: &[PathBuf], out: &Path) -> Result<Labels> {
    let mut labels = BTreeMap::new();
    let mut files = Vec::new();
    let mut report_map = BTreeMap::new();
    for report in reports {
        let canon = crate::paths::canonicalize(report)
            .map_err(crate::run::io_err("resolving label report".into()))?;
        let d = canon
            .parent()
            .unwrap_or(Path::new("."))
            .join(crate::decision::DECISIONS_FILE_NAME);
        report_map.insert(d.clone(), canon);
        if d.is_file() {
            files.push(d);
        }
    }
    if let Some(dir) = decisions_dir.filter(|p| p.is_dir()) {
        for f in walkdir::WalkDir::new(dir).follow_links(false) {
            let f = f.map_err(|e| Error::Config(format!("scanning decisions: {e}")))?;
            if !f.file_type().is_file() {
                continue;
            }
            let p = f.path();
            let name = f.file_name().to_string_lossy();
            if name.ends_with("saccade-decisions.v1.json") {
                let p = crate::paths::canonicalize(p)
                    .map_err(crate::run::io_err("resolving decisions".into()))?;
                files.push(p);
            } else if p.parent().is_some_and(|p| p.ends_with("inbox")) && name.ends_with(".json") {
                let it: crate::inbox::Item = serde_json::from_slice(
                    &std::fs::read(p).map_err(crate::run::io_err("reading inbox labels".into()))?,
                )?;
                if it.status != "answered"
                    || !it
                        .answer
                        .as_deref()
                        .is_some_and(|a| matches!(a, "accept" | "reject"))
                {
                    continue;
                }
                let ctx: Value = it
                    .context
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(Value::Null);
                let (Some(report), Some(entry), Some(hash)) = (
                    ctx["report"].as_str(),
                    ctx["entry"].as_str(),
                    ctx["evidence_hash"].as_str(),
                ) else {
                    continue;
                };
                let intent = ctx["intent"].as_str().map(str::to_owned);
                let mut label = from_report(
                    Path::new(report),
                    entry,
                    it.answer.as_deref().unwrap_or(""),
                    "accept",
                    intent,
                    out,
                    json!({"source":crate::paths::portable(p),"human":"inbox","timestamp":it.answered_unix}),
                )?;
                label.state = ctx["state"].clone();
                label.evidence_hash = identity(&label);
                if label.evidence_hash != hash {
                    return Err(Error::Config(
                        "inbox final belongs to changed evidence".into(),
                    ));
                }
                add(&mut labels, label)?;
            } else if name == "human-label-items.json"
                && !p
                    .parent()
                    .is_some_and(|d| d.join("review-labels.json").is_file())
            {
                let dir = p.parent().unwrap_or(Path::new("."));
                let metadata: Vec<Label> = serde_json::from_slice(
                    &std::fs::read(p)
                        .map_err(crate::run::io_err("reading human vote evidence".into()))?,
                )?;
                let run = crate::judge_vote::read_run(dir).map_err(Error::Config)?;
                type Settled = BTreeMap<(String, String), Vec<(String, String, u64)>>;
                let mut paired: Settled = BTreeMap::new();
                for v in crate::judge_vote::votes(dir) {
                    let Some(it) = run.items.iter().find(|i| i.id == v.item) else {
                        continue;
                    };
                    let stable = if it.question == "preference" {
                        match (v.answer.as_str(), it.order.as_str()) {
                            ("P1", "ab") | ("P2", "ba") => "a",
                            ("P2", "ab") | ("P1", "ba") => "b",
                            ("tie", _) => "tie",
                            _ => continue,
                        }
                        .to_owned()
                    } else {
                        if crate::judge_stats::is_abstain(&v.answer) {
                            continue;
                        }
                        v.answer
                    };
                    paired.entry((v.voter, it.item.clone())).or_default().push((
                        it.order.clone(),
                        stable,
                        v.ts,
                    ));
                }
                for ((voter, item), v) in paired {
                    if v.len() != 2 || v[0].0 == v[1].0 || v[0].1 != v[1].1 {
                        continue;
                    }
                    let Some(mut label) = metadata.iter().find(|l| l.entry == item).cloned() else {
                        continue;
                    };
                    label.answer = v[0].1.clone();
                    label.provenance = vec![
                        json!({"human":voter,"source":crate::paths::portable(p),"timestamp_ms":v.iter().map(|v|v.2).max()}),
                    ];
                    label.images = label
                        .images
                        .iter()
                        .map(|i| {
                            crate::paths::record(
                                &crate::paths::resolve(i, p),
                                out.parent().unwrap_or(Path::new(".")),
                                false,
                            )
                        })
                        .collect();
                    if label.evidence_hash != identity(&label) {
                        return Err(Error::Config("changed human vote evidence".into()));
                    }
                    add(&mut labels, label)?;
                }
            } else if name == "review-labels.json" {
                let dir = p.parent().unwrap_or(Path::new("."));
                let metadata: Vec<Value> = serde_json::from_slice(
                    &std::fs::read(p)
                        .map_err(crate::run::io_err("reading vote metadata".into()))?,
                )?;
                let run = crate::judge_vote::read_run(dir).map_err(Error::Config)?;
                type PairedVotes = BTreeMap<(String, String), Vec<(String, String, u64)>>;
                let mut paired: PairedVotes = BTreeMap::new();
                for vote in crate::judge_vote::votes(dir) {
                    let Some(item) = run.items.iter().find(|i| i.id == vote.item) else {
                        continue;
                    };
                    let stable = match (vote.answer.as_str(), item.order.as_str()) {
                        ("P1", "ab") | ("P2", "ba") => "a",
                        ("P2", "ab") | ("P1", "ba") => "b",
                        ("tie", _) => "tie",
                        _ => continue,
                    };
                    paired
                        .entry((vote.voter, item.item.clone()))
                        .or_default()
                        .push((item.order.clone(), stable.into(), vote.ts));
                }
                for ((voter, id), v) in paired {
                    if v.len() != 2 || v[0].0 == v[1].0 || v[0].1 != v[1].1 {
                        continue;
                    }
                    if v[0].1 == "tie" {
                        continue;
                    }
                    let Some(m) = metadata.iter().find(|m| m["item"] == id) else {
                        continue;
                    };
                    let (Some(report), Some(entry)) = (m["report"].as_str(), m["entry"].as_str())
                    else {
                        continue;
                    };
                    let mut label = from_report(
                        Path::new(report),
                        entry,
                        crate::review::candidate_answer(&v[0].1),
                        "accept",
                        m["intent"].as_str().map(str::to_owned),
                        out,
                        json!({"source":crate::paths::portable(p),"human":voter,"timestamp_ms":v.iter().map(|v|v.2).max()}),
                    )?;
                    label.state = m["state"].clone();
                    label.evidence_hash = identity(&label);
                    if label.evidence_hash != m["evidence_hash"] {
                        return Err(Error::Config(
                            "vote final belongs to changed evidence".into(),
                        ));
                    }
                    add(&mut labels, label)?;
                }
            }
        }
    }
    files.sort();
    files.dedup();
    for file in files {
        let d = crate::view::read_decisions(&file)?;
        if d.blind {
            continue;
        }
        for set in &d.sets {
            let Some(verdict) = set.decision else {
                continue;
            };
            let human = set
                .proposals
                .iter()
                .filter(|p| {
                    crate::judge_stats::is_human_source(&p.source)
                        && !p.proposed
                        && !p.promoted
                        && p.question == "accept"
                })
                .max_by_key(|p| p.timestamp_ms);
            if human.is_none() && set.proposals.iter().any(|p| p.promoted) {
                continue;
            }
            let answer = serde_json::to_value(verdict)?
                .as_str()
                .unwrap_or("")
                .to_owned();
            if human.is_some_and(|p| p.answer != answer) {
                continue;
            }
            let provenance = json!({"source":crate::paths::portable(&file),"human":human.map_or("viewer",|p|p.source.as_str()),"timestamp_ms":set.timestamp_ms});
            if let Some(report) = report_map.get(&file) {
                let label =
                    from_report(report, &set.name, &answer, "accept", None, out, provenance)?;
                let current = &label.sha256;
                if !set.sha256.is_empty() && current != &set.sha256 {
                    return Err(Error::Config(
                        "human decision belongs to changed image hashes".into(),
                    ));
                }
                add(&mut labels, label)?;
            } else {
                if set.sha256.is_empty() || set.sha256.iter().any(Option::is_none) {
                    continue;
                }
                let base = out.parent().unwrap_or(Path::new("."));
                let images = d
                    .dirs
                    .iter()
                    .map(|p| {
                        crate::paths::record(
                            &crate::paths::resolve(p, &file).join(&set.name),
                            base,
                            false,
                        )
                    })
                    .collect();
                let mut label = Label {
                    entry: set.name.clone(),
                    question: "accept".into(),
                    answer,
                    evidence_hash: String::new(),
                    state: json!({"human_evidence":set.sha256}),
                    intent: None,
                    images,
                    sha256: set.sha256.clone(),
                    hotspots: Vec::new(),
                    report: None,
                    provenance: vec![provenance],
                };
                label.evidence_hash = identity(&label);
                add(&mut labels, label)?;
            }
        }
    }
    let set = Labels {
        schema: SCHEMA.into(),
        items: labels.into_values().collect(),
    };
    for label in &set.items {
        let _ = replay(label, out)?;
    }
    crate::review::write_json(out, &serde_json::to_value(&set)?)?;
    Ok(set)
}

pub(crate) fn hash(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(value.to_string().as_bytes());
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
