//! Immutable assist workflows, neutral two-order observations and mechanical validation.
use super::{
    Error, Result,
    catalog::{Catalog, Condition},
    decode, digest,
    execution::{CacheKey, DATA_RULE, ENCODER},
    geometry::Geometry,
    require,
    schema::*,
};
use crate::evidence::canonical::Digest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Explicit routing selection; deterministic stop/unavailability rules always win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Deterministic evidence-need rules only; unresolved visual questions remain unavailable.
    Rules,
    /// Deterministic rules followed by vision and optional dependent support.
    Cascade,
    /// Evaluate full visual path, including cases routed to exact source facts.
    AllVision,
}
/// Evidence need is a local fact about scope, never an approval instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    /// Exact hash-bound producer geometry can answer this condition.
    Structured(Outcome),
    /// A visual question remains.
    Vision,
    /// Missing original pixels or incomplete required scope.
    Unavailable(&'static str),
}
/// Deterministic evidence-need rules cannot be suppressed by a provider.
pub fn evidence_need(catalog: &Catalog, condition: Option<&Condition>, task: Task) -> Result<Need> {
    catalog.validate()?;
    if let Some(condition) = condition {
        catalog.validate_condition(condition)?;
    }
    if catalog.images.iter().any(|i| !i.original_pixels) {
        return Ok(Need::Unavailable(
            "original pixels unavailable; pre-masked content cannot be assessed",
        ));
    }
    if task == Task::CheckUi && catalog.images.iter().any(|i| !i.complete) {
        return Ok(Need::Unavailable("requested capture scope is incomplete"));
    }
    if task == Task::AuditMask && catalog.exclusions.iter().any(|m| !m.original_pixels) {
        return Ok(Need::Unavailable("individual mask lacks original pixels"));
    }
    // Only exact geometric non-overlap can be answered by the Wave 3 packet.
    // Source text/DOM presence alone cannot prove visible text or absence.
    if let Some(Condition::NonOverlap { first, second }) = condition {
        let a = catalog
            .regions
            .iter()
            .find(|r| &r.id == first)
            .ok_or(Error::Invalid("first region"))?;
        let b = catalog
            .regions
            .iter()
            .find(|r| &r.id == second)
            .ok_or(Error::Invalid("second region"))?;
        let image = catalog.image(a.image_role)?;
        if !contains(&Geometry::Box(a.rect.map(f64::from)), image.capture_scope)
            || !contains(&Geometry::Box(b.rect.map(f64::from)), image.capture_scope)
        {
            return Ok(Need::Unavailable(
                "known targets outside requested capture scope",
            ));
        }
        let exact = catalog.source_evidence.iter().any(|s| {
            s.kind != "tesseract_tsv"
                && s.complete
                && s.capture_sha256 == image.sha256.as_str()[7..]
                && s.nodes
                    .iter()
                    .any(|n| n.id == *first && n.bounds == Some(a.rect.map(f64::from)))
                && s.nodes
                    .iter()
                    .any(|n| n.id == *second && n.bounds == Some(b.rect.map(f64::from)))
        });
        if exact {
            return Ok(Need::Structured(
                if intersects(&Geometry::Box(a.rect.map(f64::from)), b.rect) {
                    Outcome::NotObserved
                } else {
                    Outcome::Observed
                },
            ));
        }
    }
    Ok(Need::Vision)
}
/// Neutral provider observation. No provider may set verification or provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireObservation {
    /// P1/P2; single-image tasks use P1.
    pub slot: String,
    /// Closed visual category.
    pub kind: Kind,
    /// Descriptive untrusted literal data.
    pub statement: String,
    /// Normalized [0,1] coordinates within the recorded view.
    pub geometry: Geometry,
    /// Visible scope.
    pub visibility: Visibility,
    /// Neutral request-local region references, e.g. P1:R0.
    pub evidence_refs: Vec<String>,
    /// Reported uncertainty, not calibrated confidence.
    pub uncertainty: f64,
}
/// Closed Gemini response; hash binds the response to one exact evidence task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireAnswer {
    /// Exact catalog/condition/policy identity shown to the provider.
    pub request_hash: Digest,
    /// Advisory semantic result.
    pub outcome: Outcome,
    /// Bounded visible observations only.
    pub observations: Vec<WireObservation>,
}
/// Private mapping and exact payload; roles/mutation labels are never sent to Gemini.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// Bound local request identity.
    pub identity: Identity,
    /// Exact body for dispatch/replay.
    pub payload: Vec<u8>,
    /// Revision-separated cache key.
    pub key: CacheKey,
    /// Locally bound condition; incidental observations cannot establish it.
    pub condition: Option<Condition>,
    roles: Vec<Role>,
    references: Vec<(String, String, Role, [u32; 4])>,
}
fn base64(data: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        out.push(char::from(ABC[(a >> 2) as usize]));
        out.push(char::from(ABC[(((a & 3) << 4) | (b >> 4)) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(ABC[(((b & 15) << 2) | (c >> 6)) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ABC[(c & 63) as usize])
        } else {
            '='
        });
    }
    out
}
/// Build a neutral Gemini payload from verified original screenshots, encoded PNGs
/// and explicit transforms. No filenames, before/after labels or evaluation truth leak.
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    catalog: &Catalog,
    identity: Identity,
    condition: Option<&Condition>,
    pngs: &[(Role, Vec<u8>)],
    reverse: bool,
    revision: &str,
    api_config_hash: Digest,
) -> Result<Prepared> {
    require(
        identity == catalog.identity(identity.task, identity.report_hash.clone(), condition)?,
        "request/catalog identity mismatch",
    )?;
    require(
        pngs.len() == catalog.images.len(),
        "missing encoded screenshot",
    )?;
    let mut roles: Vec<_> = catalog.images.iter().map(|i| i.role).collect();
    if reverse {
        roles.reverse();
    }
    let mut references = Vec::new();
    let mut views = Vec::new();
    let mut parts = Vec::new();
    for (index, role) in roles.iter().enumerate() {
        let image = catalog.image(*role)?;
        let png = pngs
            .iter()
            .find(|(r, _)| r == role)
            .ok_or(Error::Invalid("missing PNG role"))?;
        let reader = image::ImageReader::new(std::io::Cursor::new(&png.1))
            .with_guessed_format()
            .map_err(|_| Error::Invalid("encoded PNG"))?;
        require(
            reader.format() == Some(image::ImageFormat::Png)
                && reader
                    .into_dimensions()
                    .map_err(|_| Error::Invalid("encoded dimensions"))?
                    == (image.transform.encoded[0], image.transform.encoded[1]),
            "encoded view mismatch",
        )?;
        require(
            Digest::of_bytes(&png.1) == image.encoded_sha256,
            "encoded pixel identity mismatch",
        )?;
        require(png.1.len() <= 16 * 1024 * 1024, "encoded image byte limit")?;
        let slot = format!("P{}", index + 1);
        let regions: Vec<_> = catalog
            .regions
            .iter()
            .filter(|r| r.image_role == *role)
            .enumerate()
            .map(|(n, r)| {
                let id = format!("{slot}:R{n}");
                references.push((id.clone(), r.id.clone(), *role, r.rect));
                json!({"id":id,"rect_px":r.rect})
            })
            .collect();
        views.push(json!({"slot":slot,"dimensions":image.dimensions,"capture_scope":image.capture_scope,"complete":image.complete,"original_pixels":image.original_pixels,"transform":image.transform,"regions":regions}));
        parts.push(json!({"text":slot}));
        parts.push(json!({"inline_data":{"mime_type":"image/png","data":base64(&png.1)}}));
    }
    // The exact condition is a request, never an acting agent's success claim.
    // Source facts and numerical measurements stay typed in the local sidecar;
    // blind extraction sees pixels and neutral regions, avoiding outcome leakage.
    let exclusions:Vec<_> = catalog.exclusions.iter().enumerate().map(|(i,m)|json!({"id":format!("M{i}"),"dimensions":m.dimensions,"runs":m.runs,"original_pixels":m.original_pixels})).collect();
    let neutral_condition = match condition {
        Some(Condition::NonOverlap { first, second })
        | Some(Condition::NotClipped {
            target: first,
            panel: second,
        }) => {
            let id = |local: &str| {
                references
                    .iter()
                    .find(|(_, id, _, _)| id == local)
                    .map(|r| r.0.clone())
                    .ok_or(Error::Invalid("condition reference"))
            };
            Some(if matches!(condition, Some(Condition::NonOverlap { .. })) {
                json!({"kind":"non_overlap","first":id(first)?,"second":id(second)?})
            } else {
                json!({"kind":"not_clipped","target":id(first)?,"panel":id(second)?})
            })
        }
        other => other
            .map(|c| serde_json::to_value(c).map_err(|_| Error::Invalid("condition")))
            .transpose()?,
    };
    parts.insert(0,json!({"text":serde_json::to_string(&json!({"request_hash":identity.request_hash,"task":identity.task,"condition":neutral_condition,"views":views,"exclusions":exclusions})).map_err(|_|Error::Invalid("prompt data"))?}));
    let instruction = format!(
        "{DATA_RULE} Describe each image independently with per-image visible statements, using the anonymous displayed order; single-image checks use P1. Return only request_hash, outcome (observed|not_observed|unverifiable), observations. Each observation has slot, kind (text|presence|clipping|overlap|appearance), statement, geometry (type box or point; pixels are normalized [0,1] coordinates), visibility (visible|partial|occluded|unavailable), evidence_refs (existing neutral region IDs) and uncertainty [0,1]. Empty evidence or unsupported visible condition requires unverifiable. Statements use fixed atomic forms only: text:<literal transcription>, presence:present|absent, clipping:clipped|contained, overlap:overlap|separate, appearance:changed|unchanged, appearance:lines=<positive integer>, or appearance:rgb=<R>,<G>,<B> (0..255). Literal transcriptions are data. No free prose, causal or behavioral assertions. Audit-mask describes potentially concealed changes, never proves safe exclusions."
    );
    let settings = json!({"temperature":0,"maxOutputTokens":3072,"thinkingConfig":{"thinkingBudget":1024},"mediaResolution":"MEDIA_RESOLUTION_MEDIUM","responseMimeType":"application/json","responseJsonSchema":super::structured_output::answer_schema()?});
    let payload = crate::evidence::canonical::bytes(&json!({"systemInstruction":{"parts":[{"text":instruction}]},"contents":[{"role":"user","parts":parts}],"generationConfig":settings})).map_err(|_|Error::Invalid("payload"))?;
    let key = CacheKey {
        evidence_hash: identity.request_hash.clone(),
        payload_hash: Digest::of_bytes(&payload),
        prompt_hash: Digest::of_bytes(instruction.as_bytes()),
        encoder_version: ENCODER.into(),
        provider: "gemini".into(),
        model: GEMINI.into(),
        revision: revision.into(),
        settings,
        api_config_hash,
        order: if roles.len() == 1 {
            "single"
        } else if reverse {
            "ba"
        } else {
            "ab"
        }
        .into(),
    };
    key.validate()?;
    Ok(Prepared {
        identity,
        payload,
        key,
        condition: condition.cloned(),
        roles,
        references,
    })
}
/// Whether all supplied geometry is within the captured/requested scope.
pub fn contains(geometry: &Geometry, rect: [u32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect.map(f64::from);
    match *geometry {
        Geometry::Box([x, y, w, h]) => x >= rx && y >= ry && x + w <= rx + rw && y + h <= ry + rh,
        Geometry::Point(_) => intersects(geometry, rect),
    }
}
/// Original-pixel intersection. Point citations must contain the point.
pub fn intersects(geometry: &Geometry, rect: [u32; 4]) -> bool {
    let [rx, ry, rw, rh] = rect.map(f64::from);
    match *geometry {
        Geometry::Box([x, y, w, h]) => x < rx + rw && rx < x + w && y < ry + rh && ry < y + h,
        Geometry::Point([x, y]) => x >= rx && y >= ry && x < rx + rw && y < ry + rh,
    }
}
/// Atomic visible statements exclude behavioral and causal claims by construction.
/// Text after `text:` is a literal transcription, never an asserted instruction.
pub fn validate_statement(kind: Kind, statement: &str) -> Result<()> {
    let valid = match kind {
        Kind::Text => statement
            .strip_prefix("text:")
            .is_some_and(|s| !s.trim().is_empty()),
        Kind::Presence => ["presence:present", "presence:absent"].contains(&statement),
        Kind::Clipping => ["clipping:clipped", "clipping:contained"].contains(&statement),
        Kind::Overlap => ["overlap:overlap", "overlap:separate"].contains(&statement),
        Kind::Appearance => {
            ["appearance:changed", "appearance:unchanged"].contains(&statement)
                || statement
                    .strip_prefix("appearance:lines=")
                    .is_some_and(|s| s.parse::<u32>().is_ok_and(|n| n > 0 && n <= 1024))
                || statement.strip_prefix("appearance:rgb=").is_some_and(|s| {
                    let values: Vec<_> = s.split(',').collect();
                    values.len() == 3 && values.iter().all(|v| v.parse::<u8>().is_ok())
                })
        }
    };
    require(valid, "nonvisual or unsupported statement grammar")
}
/// Decode and validate a recorded or live Gemini envelope, retaining local provenance.
pub fn decode_answer(
    catalog: &Catalog,
    prepared: &Prepared,
    body: &[u8],
    provenance: &Provenance,
) -> Result<(Outcome, Vec<Observation>)> {
    require(
        body.len() <= 256 * 1024
            && provenance.provider == "gemini"
            && provenance.requested_model == GEMINI
            && provenance.returned_model == GEMINI
            && provenance.returned_revision == prepared.key.revision
            && provenance.request_hash == prepared.key.payload_hash
            && provenance.response_hash == Digest::of_bytes(body)
            && provenance.order == prepared.key.order
            && provenance.prompt_hash == prepared.key.prompt_hash
            && provenance.encoder_version == ENCODER
            && provenance.sampling_settings == prepared.key.settings,
        "response identity or revision mismatch",
    )?;
    let envelope: Value = decode(body)?;
    require(
        envelope["modelVersion"].as_str() == Some(&prepared.key.revision),
        "response revision drift",
    )?;
    let parts = envelope["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or(Error::Invalid("missing candidate parts"))?;
    require(
        envelope["candidates"]
            .as_array()
            .is_some_and(|c| c.len() == 1)
            && envelope["candidates"][0]["finishReason"] == "STOP",
        "incomplete or multiple candidates",
    )?;
    let text = parts
        .iter()
        .filter(|p| p["thought"] != true)
        .filter_map(|p| p["text"].as_str())
        .collect::<String>();
    super::structured_output::validate_answer(text.as_bytes())?;
    let answer: WireAnswer = decode(text.as_bytes())?;
    require(
        answer.request_hash == prepared.identity.request_hash && answer.observations.len() <= 64,
        "observation request identity or count",
    )?;
    require(
        answer.outcome == Outcome::Unverifiable || !answer.observations.is_empty(),
        "committed result without cited observations",
    )?;
    let mut observations = Vec::new();
    for wire in answer.observations {
        require(
            wire.statement.len() <= 1024
                && !wire.statement.trim().is_empty()
                && wire.evidence_refs.len() <= 16
                && !wire.evidence_refs.is_empty()
                && wire.uncertainty.is_finite()
                && (0.0..=1.0).contains(&wire.uncertainty),
            "unbounded or unsupported observation",
        )?;
        validate_statement(wire.kind, &wire.statement)?;
        let index = match wire.slot.as_str() {
            "P1" => 0,
            "P2" => 1,
            _ => return Err(Error::Invalid("invented image slot")),
        };
        let role = *prepared
            .roles
            .get(index)
            .ok_or(Error::Invalid("absent image slot"))?;
        let image = catalog.image(role)?;
        let geometry = image
            .transform
            .from_normalized(&wire.geometry, image.dimensions)?;
        require(
            contains(&geometry, image.capture_scope),
            "observation outside captured scope",
        )?;
        let mut refs = Vec::new();
        for reference in &wire.evidence_refs {
            let (_, local, ref_role, rect) = prepared
                .references
                .iter()
                .find(|r| &r.0 == reference)
                .ok_or(Error::Invalid("invented region citation"))?;
            require(
                *ref_role == role && intersects(&geometry, *rect),
                "citation role or intersection mismatch",
            )?;
            require(!refs.contains(local), "duplicate citation")?;
            refs.push(local.clone());
        }
        refs.sort();
        let id = digest(&(role, wire.kind, &wire.statement, &geometry, &refs))?;
        require(
            !observations
                .iter()
                .any(|o: &Observation| o.observation_id == id.as_str()),
            "duplicate observation",
        )?;
        observations.push(Observation {
            observation_id: id.as_str().into(),
            image_role: role,
            kind: wire.kind,
            statement: wire.statement,
            geometry,
            visibility: wire.visibility,
            evidence_refs: refs,
            uncertainty: wire.uncertainty,
        });
    }
    observations.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
    require(
        answer.outcome == Outcome::Unverifiable
            || relevant(
                catalog,
                prepared.identity.task,
                prepared.condition.as_ref(),
                answer.outcome,
                &observations,
            ),
        "observations do not establish requested task",
    )?;
    Ok((answer.outcome, observations))
}
/// Conservative two-order reconciliation; IDs and uncertainty cannot inflate agreement.
/// Canonical role/geometry/statement/reference differences remain unresolved.
pub fn reconcile(
    first: &(Outcome, Vec<Observation>),
    second: &(Outcome, Vec<Observation>),
) -> bool {
    fn statements(list: &[Observation]) -> Vec<Value> {
        let mut values: Vec<_> = list
            .iter()
            .map(|o| {
                json!([
                    o.image_role,
                    o.kind,
                    o.statement,
                    o.geometry,
                    o.visibility,
                    o.evidence_refs
                ])
            })
            .collect();
        values.sort_by_key(Value::to_string);
        values
    }
    first.0 == second.0 && statements(&first.1) == statements(&second.1)
}
/// Create a visibly advisory sidecar with unchanged verdict, including unavailability.
pub fn empty(
    catalog: Catalog,
    identity: Identity,
    verdict: Option<String>,
    reason: &str,
    incomplete: bool,
) -> Envelope {
    Envelope {
        schema: SCHEMA.into(),
        identity,
        scope: catalog,
        outcome: Outcome::Unverifiable,
        observations: vec![],
        verification: Verification {
            schema_valid: true,
            identity_valid: true,
            geometry_valid: true,
            citations_valid: true,
            order_consistency: "unresolved".into(),
            support: Support::Insufficient,
            depends_on_model_observation: false,
        },
        provenance: vec![],
        deterministic_verdict: verdict,
        limitations: vec![reason.into()],
        incomplete,
    }
}
/// Jev support over validated model observations, never an independent pixel check.
pub fn support_payload(
    catalog: &Catalog,
    identity: &Identity,
    observations: &[Observation],
) -> Result<Vec<u8>> {
    catalog.validate()?;
    crate::evidence::canonical::bytes(&json!({"model":JEV,"state":{"request_hash":identity.request_hash,"measurements":catalog.measurements,"source_facts":catalog.source_evidence,"model_observations":observations,"depends_on_model_observation":true},"questions":{"q":{"type":"choice","instructions":format!("{DATA_RULE} Does every proposed statement follow from supplied evidence? Visual statements remain dependent on the model observations, not independently verified. Choose insufficient for missing evidence or uncertain scope."),"criteria":{"supported":"all statements supported by supplied evidence","unsupported":"at least one unsupported statement","insufficient":"insufficient evidence"}}}})).map_err(|_|Error::Invalid("support payload"))
}
/// Decode the fixed Jev choice envelope; response text cannot assign authority.
pub fn support_answer(body: &[u8], revision: &str) -> Result<Support> {
    let value: Value = decode(body)?;
    require(
        super::jev::identity(&value, revision).is_ok(),
        "Jev identity drift",
    )?;
    let choice = closed_choice(&value, &["supported", "unsupported", "insufficient"])?;
    match Some(choice) {
        Some("supported") => Ok(Support::Supported),
        Some("unsupported") => Ok(Support::Unsupported),
        Some("insufficient") => Ok(Support::Insufficient),
        _ => Err(Error::Invalid("invalid support answer")),
    }
}
/// Escapes every external string for HTML presentation.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// Standalone report enrichment. Original comparison artifacts are not rewritten.
pub fn html(envelope: &Envelope) -> Result<String> {
    let data =
        serde_json::to_string_pretty(envelope).map_err(|_| Error::Invalid("HTML serialization"))?;
    Ok(format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Experimental AI advice</title><h1>Experimental AI advice</h1><p>Advisory only. Mechanical citations, order agreement and Jev support are not independent visual truth. Deterministic verdict: {}</p><pre>{}</pre>",
        escape(
            envelope
                .deterministic_verdict
                .as_deref()
                .unwrap_or("not applicable")
        ),
        escape(&data)
    ))
}

/// Validate a fixed choice and every supplied probability without accepting authority fields.
pub(crate) fn closed_choice<'a>(value: &'a Value, choices: &[&str]) -> Result<&'a str> {
    require(
        value.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "model",
                    "modelVersion",
                    "answers",
                    "usage",
                    "usageMetadata",
                    "id",
                ]
                .contains(&k.as_str())
            })
        }) && value["answers"]
            .as_object()
            .is_some_and(|o| o.len() == 1 && o.contains_key("q")),
        "unknown Jev envelope fields",
    )?;
    let answer = &value["answers"]["q"];
    require(
        answer.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["type", "choice", "probabilities", "confidence"].contains(&k.as_str()))
        }),
        "unknown Jev answer fields",
    )?;
    require(
        answer.get("type").is_none_or(|v| v == "choice"),
        "Jev answer type",
    )?;
    require(
        answer.get("confidence").is_none_or(|v| {
            v.as_f64()
                .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
        }),
        "Jev confidence range",
    )?;
    let choice = answer["choice"]
        .as_str()
        .ok_or(Error::Invalid("missing Jev choice"))?;
    require(choices.contains(&choice), "invalid Jev choice")?;
    if let Some(probabilities) = answer.get("probabilities") {
        let p = probabilities
            .as_object()
            .ok_or(Error::Invalid("Jev probabilities object"))?;
        require(
            p.len() == choices.len() && choices.iter().all(|k| p.contains_key(*k)),
            "Jev probability keys",
        )?;
        let mut sum = 0.;
        let mut selected = 0.;
        for (key, value) in p {
            let n = value
                .as_f64()
                .ok_or(Error::Invalid("Jev probability number"))?;
            require(
                n.is_finite() && (0.0..=1.0).contains(&n),
                "Jev probability range",
            )?;
            sum += n;
            if key == choice {
                selected = n;
            }
        }
        require(
            (sum - 1.).abs() <= 1e-6
                && p.values()
                    .all(|v| v.as_f64().is_some_and(|n| n <= selected)),
            "Jev probability distribution conflicts with choice",
        )?;
    }
    Ok(choice)
}

/// Require task-specific evidence; this is structural support, never independent visual truth.
pub fn relevant(
    catalog: &Catalog,
    task: Task,
    condition: Option<&Condition>,
    outcome: Outcome,
    observations: &[Observation],
) -> bool {
    let cited = |o: &Observation, a: &str, b: &str| {
        o.evidence_refs.iter().any(|s| s == a) && o.evidence_refs.iter().any(|s| s == b)
    };
    match task {
        Task::Explain => observations.iter().any(|o| {
            o.kind == Kind::Appearance
                && ["appearance:changed", "appearance:unchanged"].contains(&o.statement.as_str())
        }),
        Task::CheckUi => match condition {
            Some(Condition::LabelVisible { label }) => observations.iter().any(|o| {
                o.image_role != Role::Before
                    && if outcome == Outcome::Observed {
                        o.kind == Kind::Text && o.statement == format!("text:{label}")
                    } else {
                        ["presence:absent", "clipping:clipped"].contains(&o.statement.as_str())
                    }
            }),
            Some(Condition::BannerAbsent { label }) => observations.iter().any(|o| {
                o.image_role != Role::Before
                    && if outcome == Outcome::Observed {
                        o.statement == "presence:absent"
                    } else {
                        o.kind == Kind::Text && o.statement == format!("text:{label}")
                    }
            }),
            Some(Condition::NotClipped { target, panel }) => observations
                .iter()
                .any(|o| o.kind == Kind::Clipping && cited(o, target, panel)),
            Some(Condition::NonOverlap { first, second }) => observations
                .iter()
                .any(|o| o.kind == Kind::Overlap && cited(o, first, second)),
            None => false,
        },
        Task::AuditMask => {
            !catalog.exclusions.is_empty()
                && catalog.exclusions.iter().all(|m| {
                    m.original_pixels
                        && observations.iter().any(|o| {
                            matches!(o.kind, Kind::Appearance | Kind::Clipping | Kind::Text)
                                && m.runs.iter().any(|[start, length]| {
                                    let width = u64::from(m.dimensions[0]);
                                    // Membership runs can cross rows; inspect each bounded row segment.
                                    let mut cursor = *start;
                                    while cursor < start + length {
                                        let count =
                                            (width - cursor % width).min(start + length - cursor);
                                        if intersects(
                                            &o.geometry,
                                            [
                                                (cursor % width) as u32,
                                                (cursor / width) as u32,
                                                count as u32,
                                                1,
                                            ],
                                        ) {
                                            return true;
                                        }
                                        cursor += count;
                                    }
                                    false
                                })
                        })
                })
        }
    }
}
