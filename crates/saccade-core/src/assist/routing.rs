//! Optional text-only evidence-need component, qualified separately from rules.
use super::{
    Result,
    catalog::{Catalog, Condition},
    decode,
    execution::{CacheKey, DATA_RULE, ENCODER},
    require,
    schema::{Identity, JEV},
    workflow::{self, Need},
};
use crate::evidence::canonical::Digest;
use serde_json::{Value, json};
/// A router may request pixels or abstain; it cannot supply a successful check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Continue to the complete visual path.
    Vision,
    /// Evidence is insufficient; withhold advice rather than invent a source fact.
    Insufficient,
}
/// Prepare only after deterministic validity, missingness and exact-source rules.
/// No pixels, oracle outcomes, mutations or acting-agent claims enter this request.
pub fn prepare(
    catalog: &Catalog,
    identity: &Identity,
    condition: Option<&Condition>,
    revision: &str,
    api: Digest,
) -> Result<(CacheKey, Vec<u8>)> {
    require(
        matches!(
            workflow::evidence_need(catalog, condition, identity.task)?,
            Need::Vision
        ),
        "routing cannot replace a deterministic stop or source fact",
    )?;
    require(
        *identity == catalog.identity(identity.task, identity.report_hash.clone(), condition)?,
        "routing request identity",
    )?;
    let payload=crate::evidence::canonical::bytes(&json!({"model":JEV,"state":{"request_hash":identity.request_hash,"task":identity.task,"condition":condition,"images":catalog.images.iter().map(|i|json!({"dimensions":i.dimensions,"capture_scope":i.capture_scope,"complete":i.complete,"original_pixels":i.original_pixels})).collect::<Vec<_>>(),"source_facts":catalog.source_evidence,"measurements":catalog.measurements},"questions":{"q":{"type":"choice","instructions":format!("{DATA_RULE} Decide the evidence need only. Choose vision when pixels could establish the bounded visible condition or describe changes; choose insufficient when supplied evidence cannot establish it. Never return a semantic outcome or suppress a deterministic violation."),"criteria":{"vision":"additional original pixels are needed","insufficient":"withhold advice for insufficient evidence"}}}})).map_err(|_|super::Error::Invalid("routing payload"))?;
    let key = CacheKey {
        evidence_hash: identity.request_hash.clone(),
        payload_hash: Digest::of_bytes(&payload),
        prompt_hash: Digest::of_bytes(DATA_RULE.as_bytes()),
        encoder_version: ENCODER.into(),
        provider: "jev".into(),
        model: JEV.into(),
        revision: revision.into(),
        settings: json!({"choice":"assist.evidence_need.v2"}),
        api_config_hash: api,
        order: "route".into(),
    };
    key.validate()?;
    Ok((key, payload))
}
/// Fixed choices only; neither text nor an injected approval can become authority.
pub fn answer(body: &[u8], revision: &str) -> Result<Decision> {
    let value: Value = decode(body)?;
    require(
        value["model"] == JEV && value["modelVersion"].as_str().unwrap_or(JEV) == revision,
        "Jev routing revision drift",
    )?;
    let answer = &value["answers"]["q"];
    require(
        answer.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["choice", "probabilities"].contains(&k.as_str()))
        }),
        "routing authority or unknown fields",
    )?;
    match answer["choice"].as_str() {
        Some("vision") => Ok(Decision::Vision),
        Some("insufficient") => Ok(Decision::Insufficient),
        _ => Err(super::Error::Invalid("routing fixed choice")),
    }
}
