//! The five supported decision questions. Historical artifact readers remain
//! independent of this catalog; new requests must use its exact closed schemas.
use crate::evidence::case::{Availability, FactSource, FactValue};
use crate::evidence::request::{Constraints, DecisionRequest, Question, Role};
use crate::evidence::{Result, require};
use serde::Serialize;
use serde_json::{Value, json};

/// One required feature and whether its explicit absence is part of the question.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Requirement {
    /// Encoder feature name, also used to record missing evidence.
    pub name: &'static str,
    /// Absence can be interpreted, subject to deterministic answer restrictions.
    pub missing_is_evidence: bool,
}
const fn needed(name: &'static str) -> Requirement {
    Requirement {
        name,
        missing_is_evidence: false,
    }
}
const fn nullable(name: &'static str) -> Requirement {
    Requirement {
        name,
        missing_is_evidence: true,
    }
}

/// Versioned, closed question schema and its task-specific evaluation contract.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct QuestionSchema {
    /// ID includes the rubric version.
    pub id: &'static str,
    /// Fixed question, never project-supplied instructions.
    pub text: &'static str,
    /// Closed choices, including abstention.
    pub answers: &'static [&'static str],
    /// Evidence checked before dispatch.
    pub required_evidence: &'static [Requirement],
    /// Additional question-selected optional features.
    pub optional_evidence: &'static [&'static str],
    /// When to abstain rather than invent evidence.
    pub abstention: &'static str,
    /// Deterministic constraints in addition to the proposal-only shared contract.
    pub deterministic_constraints: &'static [&'static str],
    /// Task-specific measurements; availability is scored separately by evaluation.
    pub scoring: &'static [&'static str],
}

/// Exactly five supported schemas. Queue/retry/approval/bisect are not questions.
pub const CATALOG: [QuestionSchema; 5] = [
    QuestionSchema {
        id: "triage.route.v1",
        text: "Which route best describes this changed capture given validity, deltas, noise, intent and region facts?",
        answers: &[
            "likely_noise",
            "likely_intended",
            "suspected_regression",
            "needs_eyes",
            "abstain",
        ],
        required_evidence: &[
            needed("validity"),
            needed("deltas"),
            nullable("noise"),
            nullable("intent"),
            needed("region_facts"),
        ],
        optional_evidence: &[
            "capture_checks",
            "spatial_summary",
            "color_tone",
            "structure",
            "temporal",
            "semantics",
        ],
        abstention: "Abstain when the available evidence cannot support a route. Missing noise or intent cannot support likely_noise or likely_intended respectively.",
        deterministic_constraints: &[
            "invalid_capture_cannot_be_noise_or_intended",
            "noise_route_requires_repeats",
            "intended_route_requires_intent",
            "measured_violations_remain_visible",
            "no_equality_or_approval_claim",
        ],
        scoring: &[
            "per_class_precision_recall",
            "regression_routed_to_noise_or_intended",
        ],
    },
    QuestionSchema {
        id: "vision.route.v1",
        text: "What minimum visual inspection is needed to resolve the task's uncertainty in this exact structured packet?",
        answers: &[
            "text_sufficient",
            "inspect_regions",
            "inspect_full_frame",
            "human_directly",
            "abstain",
        ],
        required_evidence: &[
            needed("structured_evidence"),
            nullable("semantic_uncertainty"),
            needed("affected_regions"),
            nullable("missing_features"),
        ],
        optional_evidence: &[
            "spatial_summary",
            "color_tone",
            "structure",
            "temporal",
            "semantics",
        ],
        abstention: "Abstain when the necessary visual fact or useful inspection scope cannot be determined; human_directly records a separate need for human judgment.",
        deterministic_constraints: &[
            "pixel_need_is_human_labeled",
            "pixel_need_independent_of_gemini_outcome",
            "human_authority_is_separate_from_pixel_need",
            "no_equality_or_approval_claim",
        ],
        scoring: &[
            "pixel_requirement_recall",
            "unnecessary_visual_requests",
            "region_full_frame_route_agreement",
        ],
    },
    QuestionSchema {
        id: "perf.interpret.v1",
        text: "Which disposition is supported by the qualified deterministic performance findings and declared intervention?",
        answers: &[
            "consistent_with_intent",
            "unexplained_change",
            "collect_more_evidence",
            "needs_human",
            "abstain",
        ],
        required_evidence: &[
            needed("deterministic_findings"),
            needed("qualification"),
            nullable("repeats"),
            nullable("attribution"),
            nullable("counters"),
            nullable("declared_intervention"),
        ],
        optional_evidence: &["performance", "temporal"],
        abstention: "Abstain when evidence cannot support a disposition. Explicit gaps can support collect_more_evidence, never a qualified speedup or causal claim.",
        deterministic_constraints: &[
            "qualification_is_computed",
            "no_invented_timing_or_attribution",
            "correlation_is_not_causality",
            "unexplained_terms_remain_visible",
            "no_equality_or_approval_claim",
        ],
        scoring: &[
            "evidence_backed_disposition_agreement",
            "unsupported_speedup_or_causality_claims",
        ],
    },
    QuestionSchema {
        id: "capture.disposition.v1",
        text: "What action is supported by the validity checks, convergence, expected content and capture context?",
        answers: &[
            "continue_review",
            "recapture",
            "inspect_configuration",
            "needs_human",
            "abstain",
        ],
        required_evidence: &[
            needed("validity"),
            nullable("convergence"),
            nullable("expected_content"),
            nullable("capture_context"),
        ],
        optional_evidence: &[
            "capture_checks",
            "deltas",
            "region_facts",
            "temporal",
            "structure",
        ],
        abstention: "Abstain when no disposition can be supported. Missing or invalid deterministic evidence cannot be overridden by continue_review.",
        deterministic_constraints: &[
            "continue_requires_valid_capture",
            "recapture_is_advice_not_execution",
            "configuration_checks_are_not_model_authority",
            "no_equality_or_approval_claim",
        ],
        scoring: &[
            "invalid_capture_allowed_to_continue",
            "unnecessary_recaptures",
        ],
    },
    QuestionSchema {
        id: "intent.match.v1",
        text: "Which answer best describes the observed changes against the declared intent?",
        answers: &[
            "consistent",
            "partially_consistent",
            "contradicts",
            "insufficient_intent",
            "abstain",
        ],
        required_evidence: &[
            nullable("structured_intent"),
            needed("changed_regions_passes"),
            nullable("attributed_visual_observations"),
        ],
        optional_evidence: &[
            "deltas",
            "color_tone",
            "structure",
            "semantics",
            "performance",
        ],
        abstention: "Abstain when intent or attributed observations cannot settle the relationship. Missing or lower-assurance intent may support insufficient_intent.",
        deterministic_constraints: &[
            "consistency_requires_structured_intent_and_observations",
            "measured_violations_remain_visible",
            "no_equality_or_approval_claim",
        ],
        scoring: &["missed_contradictions", "insufficient_intent_detection"],
    },
];

/// Rejects unknown, removed, unversioned and deferred questions.
pub fn lookup(id: &str) -> Result<&'static QuestionSchema> {
    CATALOG.iter().find(|q| q.id == id).ok_or_else(|| {
        crate::evidence::ContractError::Invalid(format!("unsupported decision question {id:?}"))
    })
}

impl QuestionSchema {
    /// Canonical R4 question instance.
    pub fn question(&self) -> Question {
        Question {
            id: self.id.into(),
            text: self.text.into(),
            answers: self.answers.iter().map(|s| (*s).into()).collect(),
        }
    }
    /// Shared fixed proposal and citation limits.
    pub fn constraints(&self) -> Constraints {
        Constraints {
            role: Role::Proposal,
            required_citations: true,
            max_reason_chars: 600,
        }
    }
    /// JSON Schema for this exact question, including its evidence/scoring rubric.
    pub fn json_schema(&self) -> Value {
        json!({"$schema":"https://json-schema.org/draft/2020-12/schema",
            "title":self.id, "type":"object", "const":self.question(),
            "x-saccade-question":self, "x-saccade-constraints":self.constraints()})
    }
}

/// Available named feature. Missing features never masquerade as a zero.
pub fn feature<'a>(request: &'a DecisionRequest, name: &str) -> Option<&'a FactValue> {
    request
        .evidence
        .facts
        .iter()
        .find(|f| f.name == name)
        .and_then(|f| f.value.value())
}

/// R9 dispatch validation, stricter than historical R4 artifact reads.
pub fn validate_request(request: &DecisionRequest) -> Result<()> {
    request.validate()?;
    let schema = lookup(&request.question.id)?;
    require(
        request.question == schema.question(),
        "question differs from the versioned catalog",
    )?;
    require(
        request.constraints == schema.constraints(),
        "question constraints differ from the catalog",
    )?;
    require(
        request.evidence.encoder_version == "structured-evidence/1",
        "unsupported structured evidence encoder",
    )?;
    for required in schema.required_evidence {
        if required.name == "attributed_visual_observations"
            && !request.evidence.observations.is_empty()
        {
            continue;
        }
        let facts: Vec<_> = request
            .evidence
            .facts
            .iter()
            .filter(|f| f.name == required.name)
            .collect();
        require(
            facts.len() == 1,
            &format!("required feature {} must occur exactly once", required.name),
        )?;
        if !matches!(facts[0].value, Availability::Available { .. }) {
            require(
                required.missing_is_evidence
                    && request.evidence.missing.iter().any(|m| m == required.name),
                &format!("required evidence {} is absent", required.name),
            )?;
        } else if matches!(
            required.name,
            "validity"
                | "qualification"
                | "deltas"
                | "noise"
                | "region_facts"
                | "affected_regions"
                | "changed_regions_passes"
                | "deterministic_findings"
                | "repeats"
                | "attribution"
                | "counters"
                | "convergence"
                | "capture_context"
        ) {
            require(
                facts[0].source == FactSource::Measured && !facts[0].depends_on_model_observation,
                "deterministic evidence must be measured, not declared or model-inferred",
            )?;
        }
    }
    if let Some(value) = feature(request, "validity") {
        require(
            matches!(value, FactValue::Text(v) if ["valid", "invalid", "unknown"].contains(&v.as_str())),
            "invalid deterministic validity category",
        )?;
    }
    if let Some(value) = feature(request, "qualification") {
        require(
            matches!(value, FactValue::Boolean(_)),
            "qualification must be a deterministic boolean",
        )?;
    }
    // Enrichment identities must resolve before dispatch and calibration.
    if let Some(context) = &request.evidence.observation_context {
        require(
            !request.evidence.observations.is_empty(),
            "observation identity without observations",
        )?;
        require(
            context.transform_identity == request.evidence.presentation_identity,
            "observation transforms differ from the presentation",
        )?;
    }
    Ok(())
}

/// Deterministic evidence limits apply to human scoring and model proposals alike.
pub fn validate_answer(request: &DecisionRequest, answer: &str) -> Result<()> {
    validate_request(request)?;
    require(
        request.question.answers.iter().any(|a| a == answer),
        "answer outside question choices",
    )?;
    let valid = feature(request, "validity") == Some(&FactValue::Text("valid".into()));
    let qualified = feature(request, "qualification") == Some(&FactValue::Boolean(true));
    let allowed = match (request.question.id.as_str(), answer) {
        ("triage.route.v1", "likely_noise") => valid && feature(request, "noise").is_some(),
        ("triage.route.v1", "likely_intended") => valid && feature(request, "intent").is_some(),
        ("capture.disposition.v1", "continue_review") => valid,
        ("perf.interpret.v1", "consistent_with_intent" | "unexplained_change") => {
            qualified
                && ["repeats", "attribution"]
                    .iter()
                    .all(|n| feature(request, n).is_some())
                && (answer != "consistent_with_intent"
                    || feature(request, "declared_intervention").is_some())
        }
        ("intent.match.v1", "consistent" | "partially_consistent" | "contradicts") => {
            feature(request, "structured_intent").is_some()
                && !request.evidence.observations.is_empty()
        }
        _ => true,
    };
    require(
        allowed,
        "answer overrides missing or invalid deterministic evidence",
    )
}

/// One fixture-level scoring result; corpus denominators and calibration belong to evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskScore {
    /// Committed agreement, absent for abstention or missing provider response.
    pub agreement: Option<bool>,
    /// An actual abstention, distinct from provider unavailability.
    pub abstained: bool,
    /// Task-specific dangerous miss.
    pub harmful_miss: bool,
    /// Separate insufficient-intent detection.
    pub insufficient_intent_detected: Option<bool>,
    /// Recapture advice on a human-labeled valid continue-review case.
    pub unnecessary_recapture: Option<bool>,
}

/// Independent evidence/rubric inputs to task scoring, never model self-grading.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScoringContext {
    /// Deterministic capture invalidity, unknown when not supplied.
    pub capture_invalid: Option<bool>,
    /// Evidence-backed audit flags an unsupported speedup or causality claim.
    pub unsupported_performance_claim: bool,
}

/// Scores one applicable closed-answer fixture, preserving missingness.
pub fn score(
    id: &str,
    human: &str,
    proposed: Option<&str>,
    context: ScoringContext,
) -> Result<TaskScore> {
    let q = lookup(id)?;
    require(
        q.answers.contains(&human),
        "human answer outside question choices",
    )?;
    require(
        proposed.is_none_or(|a| q.answers.contains(&a)),
        "proposed answer outside question choices",
    )?;
    let committed = proposed.filter(|a| *a != "abstain");
    let harmful_miss = match id {
        "triage.route.v1" => {
            human == "suspected_regression"
                && committed.is_some_and(|a| matches!(a, "likely_noise" | "likely_intended"))
        }
        "perf.interpret.v1" => committed.is_some() && context.unsupported_performance_claim,
        "capture.disposition.v1" => {
            context.capture_invalid == Some(true) && committed == Some("continue_review")
        }
        "intent.match.v1" => {
            human == "contradicts"
                && committed.is_some_and(|a| matches!(a, "consistent" | "partially_consistent"))
        }
        _ => false,
    };
    Ok(TaskScore {
        agreement: committed.filter(|_| human != "abstain").map(|a| a == human),
        abstained: proposed == Some("abstain"),
        harmful_miss,
        insufficient_intent_detected: if human == "insufficient_intent" {
            committed.map(|a| a == "insufficient_intent")
        } else {
            None
        },
        unnecessary_recapture: if human == "continue_review"
            && context.capture_invalid == Some(false)
        {
            committed.map(|a| a == "recapture")
        } else {
            None
        },
    })
}
