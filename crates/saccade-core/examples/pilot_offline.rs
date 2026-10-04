//! Offline pilot preparation and scoring, using the production catalog and encoders.
//! This executable has no transport, credential lookup, or dispatch path.
mod pilot_conformance;
mod pilot_gemini;
mod pilot_labels;
use saccade_core::evidence::{Artifact, Document, canonical, request::DecisionRequest};
use saccade_core::judge_evidence::vision::{
    self, DisplayImage, DisplayTransform, VisionRoi, VisionTask,
};
use saccade_core::judge_stats::evaluation::{self, Observation, Outcome};
use saccade_core::{judge_evidence, questions};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;

fn write(path: &Path, value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(path, canonical::bytes(value)?)?;
    Ok(())
}
fn compact_performance(
    case: &mut saccade_core::evidence::case::EvidenceCase,
) -> Result<(), Box<dyn std::error::Error>> {
    use saccade_core::evidence::case::{Availability, FactValue};
    let mut projected = false;
    for fact in &mut case.facts {
        if !matches!(fact.name.as_str(), "performance" | "attribution") {
            continue;
        }
        let Availability::Available {
            value: FactValue::Text(text),
        } = &mut fact.value
        else {
            continue;
        };
        projected = true;
        let mut value: Value = serde_json::from_str(text)?;
        if let Some(object) = value.as_object_mut() {
            for key in ["context_before", "context_after", "terms"] {
                if let Some(omitted) = object.remove(key) {
                    object.insert(
                        format!("{key}_sha256"),
                        serde_json::to_value(canonical::digest(&omitted)?)?,
                    );
                    object.insert(format!("{key}_omission"), json!("Full source is retained in the hash-bound report; omitted from compact pilot packet."));
                    if let Some(terms) = omitted.as_array() {
                        object.insert("term_count".into(), json!(terms.len()));
                    }
                }
            }
        }
        *text = serde_json::to_string(&value)?;
    }
    if projected {
        case.limits.push("pilot-feature-projection/1 omits full term inventories and performance source contexts from packets; their hashes and authoritative report remain bound. Missing attribution is never invented.".into());
        case.refresh_id()?;
    }
    Ok(())
}

fn prepare(
    report_file: &Path,
    out: &Path,
    context: Option<&Path>,
) -> Result<Value, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    let report: saccade_core::Report = serde_json::from_slice(&std::fs::read(report_file)?)?;
    let case_file = report_file.with_file_name("evidence.json");
    let doc = Document::read(&case_file)?;
    let Artifact::Case(c) = doc.artifact else {
        return Err("expected measured case".into());
    };
    let mut c = *c;
    c.measurement.report.verify(&case_file)?;
    for i in &c.inputs {
        i.content.verify(&case_file)?;
        for s in &i.sidecars {
            s.verify(&case_file)?;
        }
    }
    judge_evidence::report_features(&mut c, &report)?;
    compact_performance(&mut c)?;
    if let Some(context) = context {
        use saccade_core::evidence::case::{
            ArtifactRef, Availability, Fact, FactSource, FactValue,
        };
        let value: Value = serde_json::from_slice(&std::fs::read(context)?)?;
        if let Some(validity) = value.get("validity") {
            c.validity = serde_json::from_value(validity.clone())?;
        }
        if let Some(intent) = value.get("intent") {
            c.intent = serde_json::from_value(intent.clone())?;
        }
        let artifact = ArtifactRef::from_file(context, &case_file, false)?;
        let identity = canonical::Digest::of_bytes(&std::fs::read(context)?);
        for (name, supplied) in value["features"]
            .as_object()
            .ok_or("missing context features")?
        {
            c.facts.retain(|f| f.name != *name);
            let availability: Availability<FactValue> =
                serde_json::from_value(supplied["value"].clone())?;
            let source: FactSource = serde_json::from_value(supplied["source"].clone())?;
            if !matches!(source, FactSource::Measured | FactSource::Declared) {
                return Err("construction context cannot contain model output".into());
            }
            c.facts.push(Fact {
                id: format!("encoded-{name}"),
                name: name.clone(),
                units: "construction-evidence/1".into(),
                scope: c.scope.clone(),
                source,
                artifact: artifact.clone(),
                source_identity: identity.clone(),
                value: availability,
                depends_on_model_observation: false,
                observation_refs: vec![],
            });
        }
        c.refresh_id()?;
    }
    judge_evidence::prepare_context(&mut c)?;
    let mut requests = Vec::new();
    let mut missing = Vec::new();
    for q in questions::CATALOG {
        match judge_evidence::encode(&c, q.id, Default::default()) {
            Ok(r) => {
                let path = out.join(format!("{}.json", q.id));
                write(&path, &r)?;
                let payload =
                    saccade_core::judge_provider::observations::jev_payload(&r, "jev-latest")?;
                std::fs::write(out.join(format!("{}.payload.json", q.id)), &payload)?;
                requests.push(json!({"question":q.id,"request_id":r.request_id,"request_sha256":canonical::Digest::of_bytes(&canonical::bytes(&r)?),"payload_sha256":canonical::Digest::of_bytes(&payload)}));
            }
            Err(e) => missing.push(json!({"question":q.id,"reason":e.to_string()})),
        }
    }
    let mut presentations = Vec::new();
    for (n, entry) in report.entries.iter().enumerate() {
        let load = |role: &str| -> Result<DisplayImage, Box<dyn std::error::Error>> {
            let id = format!("{role}:{}", entry.name);
            let input = c.inputs.iter().find(|i| i.id == id).ok_or("missing pair")?;
            let file = saccade_core::paths::resolve(&input.content.path, &case_file);
            Ok(DisplayImage::load(
                &id,
                &file,
                if saccade_core::hdr::is_hdr_path(&file) {
                    DisplayTransform::Hdr {
                        version: "hdr-display/1".into(),
                        tonemapper: "reinhard".into(),
                        exposure_stops: 0.0,
                    }
                } else {
                    DisplayTransform::Srgb { background: 128 }
                },
            )?)
        };
        let a = load("baseline")?;
        let b = load("capture")?;
        let rois: Vec<_> = entry
            .hotspots
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, h)| VisionRoi {
                region_id: format!("r{}", i + 1),
                rect: h.rect_px,
                enhance: true,
            })
            .collect();
        for swap in [false, true] {
            let p = vision::prepare_visual(
                &c,
                &entry.name,
                [&a, &b],
                &rois,
                VisionTask::Observations,
                swap,
            )?;
            let order = if swap { "ba" } else { "ab" };
            let dir = out.join(format!("view-{n}-{order}"));
            std::fs::create_dir_all(&dir)?;
            for v in &p.payload.views {
                std::fs::write(dir.join(format!("{}.png", v.id)), &v.png)?;
            }
            write(&dir.join("presentation.json"), &p.payload)?;
            write(&dir.join("mapping.json"), &p.mapping)?;
            presentations.push(json!({"view":n,"order":order,"identity":p.identity()?,"views":p.payload.views.iter().map(|v|json!({"id":v.id,"kind":v.kind,"sha256":v.png_sha256})).collect::<Vec<_>>()}));
        }
    }
    write(
        &out.join("case.json"),
        &Document::new(Artifact::Case(Box::new(c))),
    )?;
    Ok(
        json!({"requests":requests,"missing":missing,"presentations":presentations,"catalog":questions::CATALOG,"encoder_version":judge_evidence::ENCODER_VERSION}),
    )
}
fn relocate(report: &Path, out: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    use saccade_core::evidence::case::ArtifactRef;
    let from = report.with_file_name("evidence.json");
    let to = out.join("case.json");
    let mut doc = Document::read(&to)?;
    let Artifact::Case(c) = &mut doc.artifact else {
        return Err("missing case".into());
    };
    let repair = |r: &mut ArtifactRef| -> Result<(), Box<dyn std::error::Error>> {
        let file = saccade_core::paths::resolve(&r.path, &from);
        r.verify(&from)?;
        *r = ArtifactRef::from_file(&file, &to, false)?;
        r.verify(&to)?;
        Ok(())
    };
    repair(&mut c.measurement.report)?;
    for i in &mut c.inputs {
        repair(&mut i.content)?;
        for r in &mut i.sidecars {
            repair(r)?;
        }
    }
    for f in &mut c.facts {
        repair(&mut f.artifact)?;
    }
    c.validate()?;
    let mut requests = Vec::new();
    for q in questions::CATALOG {
        let file = out.join(format!("{}.json", q.id));
        if !file.is_file() {
            continue;
        }
        let mut r: DecisionRequest = serde_json::from_slice(&std::fs::read(&file)?)?;
        for f in r
            .evidence
            .facts
            .iter_mut()
            .chain(&mut r.evidence.observations)
        {
            repair(&mut f.artifact)?;
        }
        r.validate_for(c)?;
        write(&file, &r)?;
        let payload = saccade_core::judge_provider::observations::jev_payload(&r, "jev-latest")?;
        std::fs::write(out.join(format!("{}.payload.json", q.id)), &payload)?;
        requests.push(json!({"question":q.id,"request_id":r.request_id,"request_sha256":canonical::Digest::of_bytes(&canonical::bytes(&r)?),"payload_sha256":canonical::Digest::of_bytes(&payload)}));
    }
    write(&to, &doc)?;
    Ok(json!({"requests":requests,"references_verified":true}))
}

fn enrich(case_file: &Path, input: &Path, out: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    use saccade_core::evidence::case::{ArtifactRef, Availability, Fact, FactSource, FactValue};
    use saccade_core::evidence::request::ObservationContext;
    let doc = Document::read(case_file)?;
    let Artifact::Case(c) = doc.artifact else {
        return Err("expected case".into());
    };
    let value: Value = serde_json::from_slice(&std::fs::read(input)?)?;
    let response = Path::new(value["response_file"].as_str().ok_or("missing response")?);
    let hash = canonical::Digest::of_bytes(&std::fs::read(response)?);
    if serde_json::to_value(&hash)? != value["response_sha256"] {
        return Err("observation response changed".into());
    }
    std::fs::create_dir_all(out)?;
    let artifact = ArtifactRef::from_file(response, &out.join("requests.json"), false)?;
    let observations: Vec<Fact> = value["observations"]
        .as_array()
        .ok_or("missing observations")?
        .iter()
        .enumerate()
        .map(|(i, v)| Fact {
            id: format!("corpus-visual-{i}"),
            name: "visual_observation".into(),
            units: "corpus-visual-batch/1".into(),
            scope: c.scope.clone(),
            source: FactSource::ModelObservation,
            artifact: artifact.clone(),
            source_identity: hash.clone(),
            value: Availability::Available {
                value: FactValue::Text(v.to_string()),
            },
            depends_on_model_observation: false,
            observation_refs: vec![],
        })
        .collect();
    if observations.len() > 16 {
        return Err("unbounded observations".into());
    }
    let context: ObservationContext = serde_json::from_value(value["context"].clone())?;
    let mut requests = Vec::new();
    for q in questions::CATALOG {
        let options = judge_evidence::EncodingOptions {
            presentation_identity: context.transform_identity.clone(),
            observations: observations.clone(),
            observation_context: if observations.is_empty() {
                None
            } else {
                Some(context.clone())
            },
            policy: serde_json::from_value(value["policy"].clone())?,
        };
        if let Ok(r) = judge_evidence::encode(&c, q.id, options) {
            write(&out.join(format!("{}.json", q.id)), &r)?;
            requests.push(r);
        }
    }
    Ok(json!({"requests":requests}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    observation: Observation,
    request: DecisionRequest,
    #[serde(default)]
    synthetic: bool,
}
fn score(input: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let mut rows: Vec<Row> = serde_json::from_slice(&std::fs::read(input)?)?;
    let mut real = Vec::new();
    let mut controls = Vec::new();
    let mut task_scores = Vec::new();
    for row in &mut rows {
        questions::validate_request(&row.request)?;
        let o = &mut row.observation;
        if o.question != row.request.question.id {
            return Err("question mismatch".into());
        }
        if let Some(t) = &o.truth {
            // A concealed constructed reference can contain knowledge absent from
            // the model packet. Validate its closed class; deterministic limits
            // still apply to every model answer below.
            if !row.request.question.answers.contains(t) {
                return Err("truth outside catalog".into());
            }
        }
        if matches!(o.outcome, Outcome::Answered | Outcome::Abstained) {
            if let Some(a) = &o.answer {
                if questions::validate_answer(&row.request, a).is_err() {
                    o.outcome = Outcome::Invalid;
                    o.answer = None;
                    o.probability = None;
                    o.critical_error = true;
                } else if (a == "abstain") != (o.outcome == Outcome::Abstained) {
                    return Err("answer/outcome mismatch".into());
                }
            } else {
                return Err("valid outcome lacks answer".into());
            }
        } else if o.answer.is_some() {
            return Err("failure carries answer".into());
        }
        if let Some(t) = &o.truth {
            let invalid = match questions::feature(&row.request, "validity") {
                Some(saccade_core::evidence::case::FactValue::Text(v)) if v != "unknown" => {
                    Some(v == "invalid")
                }
                _ => None,
            };
            let s = questions::score(
                &o.question,
                t,
                o.answer.as_deref(),
                questions::ScoringContext {
                    capture_invalid: invalid,
                    unsupported_performance_claim: false,
                },
            )?;
            o.critical_error |= s.harmful_miss;
            task_scores.push(json!({"case_id":o.case_id,"question":o.question,"synthetic":row.synthetic,"score":s}));
        }
        if row.synthetic {
            controls.push(o.clone());
        } else {
            real.push(o.clone());
        }
    }
    Ok(
        json!({"strata":evaluation::strata(&real),"controls":evaluation::strata(&controls),"task_scores":task_scores,
            "observations":real,"fit":evaluation::fit(&real),"qualified":false}),
    )
}
fn write_exr(
    width: u32,
    height: u32,
    input: &Path,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(input)?;
    let expected = u64::from(width) * u64::from(height) * 3 * 4;
    if width == 0 || height == 0 || bytes.len() as u64 != expected {
        return Err("float image dimensions or byte count invalid".into());
    }
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    if samples.iter().any(|v| !v.is_finite()) {
        return Err("float samples must be finite".into());
    }
    let image =
        image::Rgb32FImage::from_raw(width, height, samples).ok_or("invalid float image")?;
    image::DynamicImage::ImageRgb32F(image).save_with_format(out, image::ImageFormat::OpenExr)?;
    let decoded = image::open(out)?.to_rgb32f();
    let max_sample = decoded
        .as_raw()
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    Ok(
        json!({"sha256":canonical::Digest::of_bytes(&std::fs::read(out)?),"width":width,"height":height,"max_sample":max_sample,"format":"rgb-f32-exr"}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let value = match a.get(1).map(String::as_str) {
        Some("prepare") if a.len() == 4 => prepare(Path::new(&a[2]), Path::new(&a[3]), None)?,
        Some("prepare") if a.len() == 5 => {
            prepare(Path::new(&a[2]), Path::new(&a[3]), Some(Path::new(&a[4])))?
        }
        Some("score") if a.len() == 3 => score(Path::new(&a[2]))?,
        Some("relocate") if a.len() == 4 => relocate(Path::new(&a[2]), Path::new(&a[3]))?,
        Some("enrich") if a.len() == 5 => {
            enrich(Path::new(&a[2]), Path::new(&a[3]), Path::new(&a[4]))?
        }
        Some("conformance") if a.len() == 3 => pilot_conformance::run(Path::new(&a[2]))?,
        Some("label") if a.len() == 4 => pilot_labels::build(Path::new(&a[2]), Path::new(&a[3]))?,
        Some("exr") if a.len() == 6 => write_exr(
            a[2].parse()?,
            a[3].parse()?,
            Path::new(&a[4]),
            Path::new(&a[5]),
        )?,
        _ => return Err("usage: pilot_offline prepare REPORT OUT | score ROWS".into()),
    };
    println!("{}", serde_json::to_string(&value)?);
    Ok(())
}

#[cfg(test)]
mod hdr_tests {
    #[test]
    fn constructed_exr_preserves_native_float_samples() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let raw = temp.path().join("samples.f32");
        let out = temp.path().join("samples.exr");
        let samples: [f32; 12] = [0.0, 0.5, 2.0, 8.0, 1.0, 0.25, 0.1, 3.0, 0.8, 1.5, 2.5, 4.5];
        std::fs::write(
            &raw,
            samples
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )?;
        let metadata = super::write_exr(2, 2, &raw, &out)?;
        assert_eq!(metadata["max_sample"], 8.0);
        assert_eq!(image::open(&out)?.to_rgb32f().as_raw(), &samples);
        assert!(super::write_exr(3, 2, &raw, &out).is_err());
        Ok(())
    }
}
