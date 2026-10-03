//! Structured evidence for the five decision questions, with explicit missing
//! features and references to authoritative artifacts. Legacy grid, color, OCR
//! and strip helpers remain available to historical review consumers.

use std::path::Path;
use std::process::{Command, Stdio};

use image::{Rgb, RgbImage, imageops};
use serde::Serialize;

use crate::report::Hotspot;

/// Anonymous visual evidence, with a private source and presentation binding.
pub mod vision;

use crate::evidence::canonical::{self, Digest};
use crate::evidence::case::{
    Availability, EvidenceCase, Fact, FactSource, FactValue, IntentAssurance,
};
use crate::evidence::request::{DecisionRequest, ObservationContext, RequestEvidence};
use crate::evidence::{Result as EvidenceResult, require};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Version of the unrounded, question-selected structured encoder.
pub const ENCODER_VERSION: &str = "structured-evidence/1";
/// Maximum encoded size; unlimited text, logs and source trees are excluded.
pub const MAX_EVIDENCE_BYTES: usize = 64 * 1024;

/// Exact presentation and completed observations, supplied by the coordinator.
pub struct EncodingOptions {
    /// Display/crop/transform identity, even for a direct request.
    pub presentation_identity: Digest,
    /// Already attributed model observations from a completed extraction.
    pub observations: Vec<Fact>,
    /// Actual model/rubric/transform/fallback identities, never guessed.
    pub observation_context: Option<ObservationContext>,
    /// Effective authorized policy, without endpoints or credentials.
    pub policy: BTreeMap<String, Value>,
}
impl Default for EncodingOptions {
    fn default() -> Self {
        Self {
            presentation_identity: Digest::of_bytes(b"structured-only/1"),
            observations: Vec::new(),
            observation_context: None,
            policy: BTreeMap::new(),
        }
    }
}

fn encoded_fact(
    case: &EvidenceCase,
    name: &str,
    units: &str,
    value: Availability<FactValue>,
    source: FactSource,
) -> Fact {
    Fact {
        id: format!("encoded-{name}"),
        name: name.into(),
        units: units.into(),
        scope: case.scope.clone(),
        source,
        artifact: case.measurement.report.clone(),
        source_identity: case.measurement.semantic_sha256.clone(),
        value,
        depends_on_model_observation: false,
        observation_refs: Vec::new(),
    }
}
fn available(value: FactValue) -> Availability<FactValue> {
    Availability::Available { value }
}
fn text(value: &impl serde::Serialize) -> EvidenceResult<Availability<FactValue>> {
    Ok(available(FactValue::Text(
        String::from_utf8(canonical::bytes(value)?)
            .map_err(|_| crate::evidence::ContractError::Invalid("non-UTF8 evidence".into()))?,
    )))
}
fn intent_projection(
    intent: &crate::evidence::case::Intent,
) -> EvidenceResult<Availability<FactValue>> {
    let mut value = serde_json::to_value(intent)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("provenance");
        if let Some(source) = &intent.source {
            object.insert("source".into(), json!({"sha256":source.sha256}));
        }
    }
    text(&value)
}
fn replace_feature(case: &mut EvidenceCase, fact: Fact) {
    if let Some(existing) = case.facts.iter_mut().find(|f| f.id == fact.id) {
        *existing = fact;
    } else {
        case.facts.push(fact);
    }
}

/// Adds compact projections of canonical context before computing case identity.
/// Existing workflow records must be rebuilt if this changes their underlying case.
pub fn prepare_context(case: &mut EvidenceCase) -> EvidenceResult<()> {
    case.validate()?;
    let old_id = case.case_id.clone();
    let validity = match case.validity.status {
        crate::evidence::case::ValidityStatus::Valid => "valid",
        crate::evidence::case::ValidityStatus::Invalid => "invalid",
        crate::evidence::case::ValidityStatus::Unknown => "unknown",
    };
    let intent = case.intent.value();
    let source_roots: std::collections::BTreeSet<_> = case
        .provenance
        .source_roots
        .iter()
        .chain(case.inputs.iter().flat_map(|i| &i.provenance.source_roots))
        .collect();
    let projections = [
        encoded_fact(
            case,
            "validity",
            "category",
            available(FactValue::Text(validity.into())),
            FactSource::Measured,
        ),
        encoded_fact(
            case,
            "intent",
            "structured_declaration",
            match intent {
                Some(i) => intent_projection(i)?,
                None => Availability::missing("declared intent absent"),
            },
            FactSource::Declared,
        ),
        encoded_fact(
            case,
            "structured_intent",
            "structured_declaration",
            match intent.filter(|i| i.assurance == IntentAssurance::Structured) {
                Some(i) => intent_projection(i)?,
                None => {
                    Availability::missing("structured intent absent; free text has lower assurance")
                }
            },
            FactSource::Declared,
        ),
        encoded_fact(
            case,
            "declared_intervention",
            "structured_declaration",
            match intent.filter(|i| !i.expected_changes.is_empty()) {
                Some(i) => text(&i.expected_changes)?,
                None => Availability::missing("predeclared intervention absent"),
            },
            FactSource::Declared,
        ),
        encoded_fact(
            case,
            "capture_context",
            "capture_identity",
            if case.inputs.iter().all(|i| i.capture.value().is_some()) {
                text(
                    &case
                        .inputs
                        .iter()
                        .map(|i| (&i.id, &i.capture))
                        .collect::<Vec<_>>(),
                )?
            } else {
                Availability::missing("capture context is incomplete")
            },
            FactSource::Measured,
        ),
        encoded_fact(
            case,
            "provenance",
            "content_identity",
            text(
                &json!({"inputs":case.inputs.iter().map(|i| json!({"id":i.id,"content":i.content.sha256,"sidecars":i.sidecars.iter().map(|s| &s.sha256).collect::<Vec<_>>(),"build":i.build})).collect::<Vec<_>>(),
            "report":case.measurement.report.sha256,"semantic_report":case.measurement.semantic_sha256,
            "config":canonical::digest(&case.effective_config)?,"producer":case.measurement.metric_version,
            "source_roots":source_roots}),
            )?,
            FactSource::Measured,
        ),
        encoded_fact(
            case,
            "structured_evidence",
            "content_identity",
            text(
                &json!({"report":case.measurement.semantic_sha256,"scope":case.scope,"validity":case.validity}),
            )?,
            FactSource::Measured,
        ),
    ];
    // Apply transactionally so an error cannot leave the caller with stale records.
    let mut next = case.clone();
    for fact in projections {
        replace_feature(&mut next, fact);
    }
    next.refresh_id()?;
    require(
        next.case_id == old_id
            || (case.requests.is_empty()
                && case.proposals.is_empty()
                && case.human_decisions.is_empty()
                && case.next_actions.is_empty()),
        "context encoding changed a case with existing workflow records; rebuild the records",
    )?;
    next.validate()?;
    *case = next;
    Ok(())
}

/// Selects bounded features per question, retaining provenance, units and omissions.
/// Unknown features are omitted and referenced through the immutable report.
pub fn encode(
    case: &EvidenceCase,
    question_id: &str,
    options: EncodingOptions,
) -> EvidenceResult<DecisionRequest> {
    case.validate()?;
    let schema = crate::questions::lookup(question_id)?;
    let mut names: Vec<_> = schema.required_evidence.iter().map(|r| r.name).collect();
    names.extend(schema.optional_evidence);
    names.push("provenance");
    let mut facts = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        if name == "attributed_visual_observations" && !options.observations.is_empty() {
            continue;
        }
        let matches: Vec<_> = case
            .facts
            .iter()
            .filter(|f| f.name == name && f.source != FactSource::ModelObservation)
            .collect();
        require(matches.len() <= 1, "duplicate structured feature names")?;
        if let Some(fact) = matches.first() {
            if fact.value.value().is_none() {
                missing.push(name.into());
            }
            facts.push((*fact).clone());
        } else {
            missing.push(name.into());
            if schema.required_evidence.iter().any(|r| r.name == name) {
                // Missing projections must be present in the case before request creation.
                return Err(crate::evidence::ContractError::Invalid(format!(
                    "case lacks feature {name}; encode its availability first"
                )));
            }
        }
    }
    let expected_validity = match case.validity.status {
        crate::evidence::case::ValidityStatus::Valid => "valid",
        crate::evidence::case::ValidityStatus::Invalid => "invalid",
        crate::evidence::case::ValidityStatus::Unknown => "unknown",
    };
    if let Some(validity) = facts.iter().find(|f| f.name == "validity") {
        require(
            validity.value.value() == Some(&FactValue::Text(expected_validity.into()))
                && validity.source == FactSource::Measured,
            "encoded validity differs from deterministic case validity",
        )?;
    }
    // Include constituents of selected aggregates without flattening additivity.
    let mut index = 0;
    while index < facts.len() {
        if let Some(FactValue::Aggregate(group)) = facts[index].value.value() {
            for id in group.members.clone() {
                if !facts.iter().any(|f| f.id == id) {
                    let member = case.facts.iter().find(|f| f.id == id).ok_or_else(|| {
                        crate::evidence::ContractError::Invalid(
                            "unknown encoded aggregate member".into(),
                        )
                    })?;
                    require(
                        member.source != FactSource::ModelObservation,
                        "observations must use typed observation references",
                    )?;
                    facts.push(member.clone());
                }
            }
        }
        index += 1;
    }
    for fact in &case.facts {
        if !facts.iter().any(|f| f.id == fact.id) {
            missing.push(format!(
                "omitted:{}; full artifact: {}",
                fact.id,
                case.measurement.report.sha256.as_str()
            ));
        }
    }
    let mut request = DecisionRequest {
        request_id: Digest::of_bytes(b""),
        case_id: case.case_id.clone(),
        question: schema.question(),
        evidence: RequestEvidence {
            encoder_version: ENCODER_VERSION.into(),
            facts,
            observations: options.observations,
            intent_ref: case.intent.value().map(|i| i.id.clone()),
            missing,
            observation_context: options.observation_context,
            presentation_identity: options.presentation_identity,
        },
        constraints: schema.constraints(),
        policy: options.policy,
    };
    request.refresh_id()?;
    crate::questions::validate_request(&request)?;
    request.validate_for(case)?;
    require(
        canonical::bytes(&request)?.len() <= MAX_EVIDENCE_BYTES,
        "structured evidence exceeds 64 KiB; select a smaller case scope",
    )?;
    Ok(request)
}

/// Reuses an authoritative report's diagnostics; no timing qualification or
/// semantic meaning is inferred from labels, filenames or unqualified timing.
pub fn report_features(case: &mut EvidenceCase, report: &crate::Report) -> EvidenceResult<()> {
    case.validate()?;
    require(
        crate::evidence::case::Measurement::report_identity(report)?
            == case.measurement.semantic_sha256,
        "report differs from the case measurement",
    )?;
    let mut next = case.clone();
    let entries: Vec<_> = report
        .entries
        .iter()
        .filter(|e| case.scope.entries.contains(&e.name))
        .collect();
    require(
        entries.len() == case.scope.entries.len(),
        "report scope is incomplete",
    )?;
    let values = [
        ("deltas", "flip_or_buffer_units", if entries.iter().all(|e| e.metrics.is_some() || e.buffer.is_some()) { text(&entries.iter().map(|e| json!({"entry":e.name,"metrics":e.metrics,"metric":e.metric_used,"threshold":e.threshold,"value":e.value,"buffer":e.buffer})).collect::<Vec<_>>())? } else { Availability::missing("some inputs lack decoded difference measurements") }),
        ("region_facts", "normalized_geometry", text(&entries.iter().map(|e| json!({"entry":e.name,"regions":e.regions,"hotspots":e.hotspots,"mask_fraction":e.masked_fraction,
            "box_geometry":e.hotspots.iter().map(|h| json!({"box_centroid":[h.rect_frac[0]+h.rect_frac[2]/2.0,h.rect_frac[1]+h.rect_frac[3]/2.0],
                "border_contact":h.rect_frac[0]<=0.0 || h.rect_frac[1]<=0.0 || h.rect_frac[0]+h.rect_frac[2]>=1.0 || h.rect_frac[1]+h.rect_frac[3]>=1.0})).collect::<Vec<_>>()})).collect::<Vec<_>>())?),
        ("affected_regions", "normalized_geometry", text(&entries.iter().map(|e| json!({"entry":e.name,"hotspots":e.hotspots,"regions":e.regions})).collect::<Vec<_>>())?),
        ("changed_regions_passes", "normalized_geometry", text(&entries.iter().map(|e| json!({"entry":e.name,"hotspots":e.hotspots,"regions":e.regions,"metadata_disagreements":e.meta_diff})).collect::<Vec<_>>())?),
        ("capture_checks", "validity_checks", text(&entries.iter().map(|e| json!({"entry":e.name,"status":e.status,"error":e.error,"validity":e.capture_validity,"metadata_disagreements":e.meta_diff,"warnings":e.warnings,"baseline_properties":e.baseline_properties,"properties":e.properties})).collect::<Vec<_>>())?),
        ("color_tone", "diagnostic", if entries.iter().any(|e| e.diagnostics.is_some()) { text(&entries.iter().map(|e| json!({"entry":e.name,"tone":e.diagnostics.as_ref().and_then(|d| d.tone.as_ref()),"signed":e.diagnostics.as_ref().and_then(|d| d.signed.as_ref()),"residual":e.diagnostics.as_ref().and_then(|d| d.residual.as_ref()),"hdr_assumptions":e.hdr})).collect::<Vec<_>>())? } else { Availability::missing("color diagnostics were not computed") }),
        ("structure", "pixels", if entries.iter().any(|e| e.diagnostics.is_some() || e.buffer.is_some()) { text(&entries.iter().map(|e| json!({"entry":e.name,"alignment":e.diagnostics.as_ref().and_then(|d| d.shift.as_ref()),"buffer":e.buffer,"properties":e.properties,"nonfinite":e.diagnostics.as_ref().and_then(|d| d.nonfinite.as_ref())})).collect::<Vec<_>>())? } else { Availability::missing("structure diagnostics were not computed") }),
    ];
    for (name, units, value) in values {
        replace_feature(
            &mut next,
            encoded_fact(case, name, units, value, FactSource::Measured),
        );
    }
    if let Some(perf) = &report.perf_diff {
        let qualified = perf.comparability == crate::perf::Comparability::Qualified;
        let repeats = match (&perf.context_before, &perf.context_after) {
            (Some(before), Some(after))
                if !before.raw_samples.is_empty() && !after.raw_samples.is_empty() =>
            {
                text(
                    &json!({"before":before.raw_samples,"after":after.raw_samples,"noise_comparability":perf.noise_comparability,"noise_floor_ms":perf.frame.noise_floor}),
                )?
            }
            _ => Availability::missing("qualified repeat sources were not supplied"),
        };
        let counters: Vec<_> = perf
            .terms
            .iter()
            .filter(|t| !t.counters.is_empty())
            .map(|t| json!({"term":t.id,"counters":t.counters}))
            .collect();
        let features = [
            (
                "qualification",
                "boolean",
                available(FactValue::Boolean(qualified)),
            ),
            (
                "deterministic_findings",
                "performance_disposition",
                text(
                    &json!({"frame_change":perf.frame_change,"attribution":perf.attribution,
                "materiality":perf.materiality,"qualification_reasons":perf.qualification_reasons,"warnings":perf.warnings}),
                )?,
            ),
            ("repeats", "source_hashes", repeats),
            (
                "attribution",
                "ms",
                text(
                    &json!({"status":perf.attribution,"terms":perf.terms,"unattributed_before":perf.unattributed_before,
                "unattributed_after":perf.unattributed_after,"materiality":perf.materiality}),
                )?,
            ),
            (
                "counters",
                "producer_units",
                if counters.is_empty() {
                    Availability::missing("performance counters absent")
                } else {
                    text(&counters)?
                },
            ),
            ("performance", "ms", text(perf)?),
        ];
        for (name, units, value) in features {
            replace_feature(
                &mut next,
                encoded_fact(case, name, units, value, FactSource::Measured),
            );
        }
    }
    for name in [
        "noise",
        "repeats",
        "attribution",
        "counters",
        "qualification",
        "deterministic_findings",
        "convergence",
        "expected_content",
        "semantic_uncertainty",
        "missing_features",
        "attributed_visual_observations",
        "temporal",
        "spatial_summary",
        "semantics",
        "performance",
    ] {
        if !next.facts.iter().any(|f| f.name == name) {
            let fact = encoded_fact(
                case,
                name,
                "feature",
                Availability::missing("not supplied by the authoritative producer"),
                FactSource::Measured,
            );
            next.facts.push(fact);
        }
    }
    next.refresh_id()?;
    require(
        next.case_id == case.case_id
            || (case.requests.is_empty()
                && case.proposals.is_empty()
                && case.human_decisions.is_empty()
                && case.next_actions.is_empty()),
        "report encoding changed a case with existing workflow records",
    )?;
    next.validate()?;
    *case = next;
    Ok(())
}

/// Grid means and maxima retain finite f32 precision rather than rounding to
/// two decimals; empty/non-finite cells stay unavailable with explicit counts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpatialSummary {
    /// Cell means, rows top to bottom; absent when no finite sample exists.
    pub mean: Vec<Vec<Option<f64>>>,
    /// Cell maxima, same layout.
    pub maximum: Vec<Vec<Option<f64>>>,
    /// Explicit numerical representation.
    pub precision: &'static str,
    /// Dimensionless FLIP units.
    pub units: &'static str,
    /// Finite input sample count.
    pub finite_count: usize,
    /// Non-finite input sample count.
    pub nonfinite_count: usize,
}

/// Computes the spatial feature only from a complete, dimensioned error map.
pub fn spatial_summary(map: &[f32], width: u32, height: u32) -> EvidenceResult<SpatialSummary> {
    let (w, h) = (width as usize, height as usize);
    require(
        w > 0 && h > 0 && w.checked_mul(h) == Some(map.len()),
        "error map dimensions differ",
    )?;
    let mut mean = vec![vec![None; GRID]; GRID];
    let mut maximum = mean.clone();
    for gy in 0..GRID {
        for gx in 0..GRID {
            let mut sum = 0.0;
            let mut n = 0;
            let mut max = f64::NEG_INFINITY;
            for y in gy * h / GRID..(gy + 1) * h / GRID {
                for &v in &map[y * w + gx * w / GRID..y * w + (gx + 1) * w / GRID] {
                    if v.is_finite() {
                        let v = f64::from(v);
                        sum += v;
                        n += 1;
                        max = max.max(v);
                    }
                }
            }
            if n > 0 {
                mean[gy][gx] = Some(sum / n as f64);
                maximum[gy][gx] = Some(max);
            }
        }
    }
    let finite_count = map.iter().filter(|v| v.is_finite()).count();
    Ok(SpatialSummary {
        mean,
        maximum,
        precision: "f32 samples; f64 aggregation; no decimal rounding",
        units: "flip",
        finite_count,
        nonfinite_count: map.len() - finite_count,
    })
}

/// Cells per side of the FLIP grid.
pub const GRID: usize = 8;

/// Longest OCR line kept, in characters.
const OCR_LINE: usize = 120;
/// Most OCR lines kept per side of the diff.
const OCR_LINES: usize = 20;
/// Largest OCR output read, in bytes.
const OCR_BYTES: usize = 64 * 1024;

/// A named reference colour (sRGB).
const NAMES: &[(&str, [u8; 3])] = &[
    ("black", [0, 0, 0]),
    ("near-black", [20, 20, 24]),
    ("very dark grey", [45, 45, 45]),
    ("dark grey", [80, 80, 80]),
    ("grey", [128, 128, 128]),
    ("light grey", [180, 180, 180]),
    ("very light grey", [215, 215, 215]),
    ("near-white", [242, 242, 242]),
    ("white", [255, 255, 255]),
    ("red", [220, 30, 30]),
    ("dark red", [120, 20, 20]),
    ("orange", [240, 140, 20]),
    ("brown", [110, 70, 30]),
    ("beige", [220, 200, 160]),
    ("yellow", [240, 220, 30]),
    ("olive", [120, 120, 30]),
    ("lime", [150, 220, 40]),
    ("green", [40, 160, 50]),
    ("dark green", [20, 80, 35]),
    ("teal", [20, 140, 130]),
    ("cyan", [40, 210, 230]),
    ("sky blue", [120, 180, 235]),
    ("blue", [35, 70, 220]),
    ("navy", [20, 30, 100]),
    ("purple", [120, 50, 160]),
    ("magenta", [210, 40, 170]),
    ("pink", [245, 150, 185]),
];

fn lin(c: f64) -> f64 {
    let c = c / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB (components 0 to 255) to CIELAB (D65).
pub fn rgb_to_lab(rgb: [f64; 3]) -> [f64; 3] {
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// The English name of the nearest table colour in CIELAB.
pub fn colour_name(rgb: [f64; 3]) -> &'static str {
    let lab = rgb_to_lab(rgb);
    let dist = |c: &[u8; 3]| {
        let t = rgb_to_lab([f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]);
        (0..3).map(|i| (lab[i] - t[i]).powi(2)).sum::<f64>()
    };
    NAMES
        .iter()
        .min_by(|a, b| dist(&a.1).total_cmp(&dist(&b.1)))
        .map_or("grey", |(n, _)| n)
}

/// How the tint moved from `from` to `to`: a hue word with a strength
/// (`slightly`, none, `strongly`), or `None` when the chroma barely changed.
fn tint(from: [f64; 3], to: [f64; 3]) -> Option<String> {
    let (a, b) = (to[1] - from[1], to[2] - from[2]);
    let c = a.hypot(b);
    if c < 3.0 {
        return None;
    }
    let angle = b.atan2(a).to_degrees().rem_euclid(360.0);
    // CIELAB hue angles are not equally spaced sRGB hue sectors. In
    // particular a blue displacement is near 295 degrees, not 270.
    let hue = match angle {
        x if !(50.0..340.0).contains(&x) => "red",
        x if x < 85.0 => "orange",
        x if x < 120.0 => "yellow",
        x if x < 175.0 => "green",
        x if x < 240.0 => "teal",
        x if x < 310.0 => "blue",
        _ => "purple",
    };
    let strength = if c < 8.0 {
        "slightly "
    } else if c < 20.0 {
        ""
    } else {
        "strongly "
    };
    Some(format!("{strength}{hue}"))
}

/// A colour change in English: `dark grey -> near-black, slightly blue`.
pub fn describe_shift(from: [f64; 3], to: [f64; 3]) -> String {
    let (nf, nt) = (colour_name(from), colour_name(to));
    let base = format!("{nf} \u{2192} {nt}");
    match tint(rgb_to_lab(from), rgb_to_lab(to)) {
        Some(t) => format!("{base}, {t}"),
        None => base,
    }
}

/// The colour change inside one changed region.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColourShift {
    /// 1-based hotspot the region is.
    pub hotspot: u32,
    /// Mean colour of the reference over the region's changed pixels, as a name.
    pub reference: String,
    /// The same for the candidate.
    pub candidate: String,
    /// The change in one phrase, for example `dark grey -> near-black, slightly blue`.
    pub change: String,
}

/// An OCR text difference between the two images.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OcrDiff {
    /// Lines only the reference has.
    pub removed: Vec<String>,
    /// Lines only the candidate has.
    pub added: Vec<String>,
    /// Lines both have.
    pub unchanged_lines: usize,
}

/// What the encoder adds to a decision-request state. Numbers and words only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Extras {
    /// Mean FLIP error of an 8x8 grid of cells, rows top to bottom, rounded to 2 decimals.
    pub flip_grid_8x8: Vec<Vec<f64>>,
    /// Colour change inside the largest hotspots.
    pub colour_shifts: Vec<ColourShift>,
    /// OCR text difference, when an OCR command is configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_diff: Option<OcrDiff>,
}

/// Mean of the error map per grid cell, rounded to 2 decimals. Non-finite
/// values count as 0 (they are reported elsewhere).
pub fn flip_grid(error_map: &[f32], width: u32, height: u32) -> Vec<Vec<f64>> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![vec![0.0; GRID]; GRID];
    if w == 0 || h == 0 || error_map.len() != w * h {
        return out;
    }
    for (gy, row) in out.iter_mut().enumerate() {
        for (gx, cell) in row.iter_mut().enumerate() {
            let (x0, x1) = (gx * w / GRID, ((gx + 1) * w / GRID).max(gx * w / GRID + 1));
            let (y0, y1) = (gy * h / GRID, ((gy + 1) * h / GRID).max(gy * h / GRID + 1));
            let (mut sum, mut n) = (0.0f64, 0u64);
            for y in y0..y1.min(h) {
                for &v in &error_map[y * w + x0..y * w + x1.min(w)] {
                    if v.is_finite() {
                        sum += f64::from(v);
                    }
                    n += 1;
                }
            }
            *cell = if n == 0 {
                0.0
            } else {
                (sum / n as f64 * 100.0).round() / 100.0
            };
        }
    }
    out
}

/// Mean colour of `img` over the changed pixels (error above 0.1) of `rect`,
/// or over the whole rectangle when none exceeds it.
fn region_mean(img: &RgbImage, error_map: &[f32], rect: [u32; 4], changed_only: bool) -> [f64; 3] {
    let (w, h) = img.dimensions();
    let mut acc = [0.0f64; 3];
    let mut n = 0u64;
    for y in rect[1]..rect[1].saturating_add(rect[3]).min(h) {
        for x in rect[0]..rect[0].saturating_add(rect[2]).min(w) {
            let e = error_map.get(y as usize * w as usize + x as usize);
            if changed_only && !e.is_some_and(|e| *e > 0.1) {
                continue;
            }
            let p = img.get_pixel(x, y).0;
            for (a, c) in acc.iter_mut().zip(p) {
                *a += f64::from(c);
            }
            n += 1;
        }
    }
    if n == 0 {
        return if changed_only {
            region_mean(img, error_map, rect, false)
        } else {
            [0.0; 3]
        };
    }
    acc.map(|a| a / n as f64)
}

/// The colour shifts of the first `top` hotspots, reference to candidate.
pub fn colour_shifts(
    base: &RgbImage,
    cap: &RgbImage,
    error_map: &[f32],
    hotspots: &[Hotspot],
    top: usize,
) -> Vec<ColourShift> {
    hotspots
        .iter()
        .take(top)
        .enumerate()
        .map(|(i, h)| {
            let from = region_mean(base, error_map, h.rect_px, true);
            let to = region_mean(cap, error_map, h.rect_px, true);
            ColourShift {
                hotspot: (i + 1) as u32,
                reference: colour_name(from).to_owned(),
                candidate: colour_name(to).to_owned(),
                change: describe_shift(from, to),
            }
        })
        .collect()
}

/// Runs `cmd` on `image` and returns its standard output, at most 64 KiB. The
/// command is split on whitespace (no shell); `{}` is replaced by the image
/// path (also accepts `{image}`), or appended when neither placeholder exists.
fn run_ocr(cmd: &str, image: &Path) -> Result<String, String> {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().ok_or("the OCR command is empty")?;
    let path = image.to_string_lossy();
    let mut args: Vec<String> = parts
        .map(|p| p.replace("{image}", &path).replace("{}", &path))
        .collect();
    if !cmd.contains("{}") && !cmd.contains("{image}") {
        args.push(path.into_owned());
    }
    let out = Command::new(program)
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("running the OCR command {program:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the OCR command {program:?} exited with {}",
            out.status
        ));
    }
    let take = &out.stdout[..out.stdout.len().min(OCR_BYTES)];
    Ok(String::from_utf8_lossy(take).into_owned())
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(OCR_LINE).collect())
        .collect()
}

/// The line-level text difference of two OCR outputs (multiset semantics).
pub fn ocr_diff_of(reference: &str, candidate: &str) -> OcrDiff {
    let (a, mut b) = (lines_of(reference), lines_of(candidate));
    let mut removed = Vec::new();
    let mut unchanged = 0;
    for line in a {
        match b.iter().position(|l| *l == line) {
            Some(i) => {
                b.remove(i);
                unchanged += 1;
            }
            None => removed.push(line),
        }
    }
    removed.truncate(OCR_LINES);
    b.truncate(OCR_LINES);
    OcrDiff {
        removed,
        added: b,
        unchanged_lines: unchanged,
    }
}

/// Runs the OCR command on both image files and diffs the text.
///
/// The text of a private screenshot goes to the judges in the evidence; point
/// this only at images you may share (see the README's privacy note).
pub fn ocr_diff(cmd: &str, reference: &Path, candidate: &Path) -> Result<OcrDiff, String> {
    Ok(ocr_diff_of(
        &run_ocr(cmd, reference)?,
        &run_ocr(cmd, candidate)?,
    ))
}

/// Padding around a hotspot crop, in pixels.
const STRIP_PAD: u32 = 12;
/// Tallest panel of a strip, in pixels.
const STRIP_H: u32 = 384;
/// Panels smaller than this are enlarged (nearest-neighbour).
const STRIP_MIN: u32 = 160;
/// Widest whole-frame panel, in pixels (the fallback when there is no hotspot).
/// Height of the label bar.
const BAR: u32 = 18;

fn stretch_gain(a: &RgbImage, b: &RgbImage, r: [u32; 4]) -> f32 {
    let mut max = 0u8;
    for img in [a, b] {
        for y in r[1]..r[1] + r[3] {
            for x in r[0]..r[0] + r[2] {
                let p = img.get_pixel(x, y).0;
                max = max.max(p[0]).max(p[1]).max(p[2]);
            }
        }
    }
    if max == 0 || max >= 200 {
        1.0
    } else {
        255.0 / f32::from(max)
    }
}

fn panel(img: &RgbImage, r: [u32; 4], gain: f32) -> RgbImage {
    let mut c = imageops::crop_imm(img, r[0], r[1], r[2], r[3]).to_image();
    if gain != 1.0 {
        for p in c.pixels_mut() {
            for v in &mut p.0 {
                *v = (f32::from(*v) * gain).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let big = c.width().max(c.height());
    if big < STRIP_MIN {
        let s = STRIP_MIN.div_ceil(big);
        imageops::resize(
            &c,
            c.width() * s,
            c.height() * s,
            imageops::FilterType::Nearest,
        )
    } else if c.height() > STRIP_H || c.width() > 768 {
        let f = (f64::from(STRIP_H) / f64::from(c.height())).min(768.0 / f64::from(c.width()));
        let w = ((f64::from(c.width()) * f).round() as u32).max(1);
        let h = ((f64::from(c.height()) * f).round() as u32).max(1);
        imageops::resize(&c, w, h, imageops::FilterType::Triangle)
    } else {
        c
    }
}

fn join_two(first: &RgbImage, second: &RgbImage) -> RgbImage {
    let ph = first.height().max(second.height());
    let mut out = RgbImage::from_pixel(first.width() + second.width() + 1, BAR + ph, Rgb([24; 3]));
    imageops::replace(&mut out, first, 0, i64::from(BAR));
    imageops::replace(
        &mut out,
        second,
        i64::from(first.width()) + 1,
        i64::from(BAR),
    );
    crate::explain::fill(&mut out, first.width(), 0, 1, BAR + ph, Rgb([200; 3]));
    crate::explain::draw_text(&mut out, 4, 2, "1", 2, Rgb([235; 3]));
    crate::explain::draw_text(&mut out, first.width() + 5, 2, "2", 2, Rgb([235; 3]));
    out
}

/// The padded, clamped crop rectangle of a hotspot.
fn crop_rect(rect: [u32; 4], w: u32, h: u32) -> [u32; 4] {
    let x0 = rect[0].saturating_sub(STRIP_PAD).min(w.saturating_sub(1));
    let y0 = rect[1].saturating_sub(STRIP_PAD).min(h.saturating_sub(1));
    let x1 = rect[0]
        .saturating_add(rect[2])
        .saturating_add(STRIP_PAD)
        .min(w);
    let y1 = rect[1]
        .saturating_add(rect[3])
        .saturating_add(STRIP_PAD)
        .min(h);
    [x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)]
}

/// Blind strips for a vision judge: one per hotspot (at most `top`), the two
/// crops side by side, contrast-stretched when dark, labelled `1` and `2`
/// only. `swap` puts the second image first. Without hotspots there are no
/// strips; full frames are never attached as a fallback.
pub fn strips(
    first: &RgbImage,
    second: &RgbImage,
    hotspots: &[Hotspot],
    top: usize,
    swap: bool,
) -> Vec<RgbImage> {
    let (a, b) = if swap {
        (second, first)
    } else {
        (first, second)
    };
    let (w, h) = first.dimensions();
    if (w, h) != second.dimensions() || w == 0 || h == 0 {
        return Vec::new();
    }
    let rects: Vec<[u32; 4]> = hotspots
        .iter()
        .take(top)
        .map(|hs| crop_rect(hs.rect_px, w, h))
        .collect();
    rects
        .into_iter()
        .map(|r| {
            let gain = stretch_gain(a, b, r);
            let (pa, pb) = (panel(a, r, gain), panel(b, r, gain));
            join_two(&pa, &pb)
        })
        .collect()
}

/// Encodes an image as PNG bytes.
pub fn png_bytes(img: &RgbImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let dynimg = image::DynamicImage::ImageRgb8(img.clone());
    match dynimg.write_to(&mut out, image::ImageFormat::Png) {
        Ok(()) => out.into_inner(),
        Err(_) => Vec::new(),
    }
}
